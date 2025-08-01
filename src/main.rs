mod cli;
mod error;
mod logger;
mod ssh;

use crate::{cli::CliArgs, error::AppError};
use clap::Parser;
use git2::{
  build::{CheckoutBuilder, RepoBuilder},
  Cred, FetchOptions, FileFavor, IndexAddOption, MergeOptions, RemoteCallbacks,
  Repository,
};
use log::*;
use logger::init_logger;
use ssh::{add_key_to_agent, start_agent};
use std::fs;
use std::path::PathBuf;
use std::{env, path::Path};

fn setup_git_callbacks(
  key_path_maybe: Option<&Path>,
) -> RemoteCallbacks<'static> {
  let mut callbacks = RemoteCallbacks::new();
  if let Some(key_path) = key_path_maybe {
    // This is the Path equivalent of .clone().
    let key_path_copy = key_path.to_path_buf();
    callbacks.credentials(move |_url, username_from_url, _allowed_types| {
      let username_rust_made_me = whoami::username();
      let username = username_from_url.unwrap_or(&username_rust_made_me);
      // Alas, this would be great if it weren't for our sparse options for
      // dealing with SSH agents.  The most "complete" solution is
      // ssh-agent-client-rs, but that crate systemically uses `unwrap` instead of
      // proper Rust error handling.  This makes debugging and clean error
      // propagation impossible.
      // Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
      Cred::ssh_key(
        username,
        None, // Let git2 find the public key automatically.
        &key_path_copy,
        None, // passphrase.
      )
    });
  }
  // TODO: GitHub's keys are both documented and finite, so this shouldn't be
  // needed.  I'll have to add them to my local configuration, and document how
  // that was done.
  // callbacks.certificate_check(|_cert, _valid| {
  //   // Trust all certificates (like StrictHostKeyChecking=no)
  //   true
  // });
  callbacks
}

fn open_or_clone_repo(
  ssh_identity: Option<&Path>,
  git_url: &str,
  sync_dir: &PathBuf,
) -> Result<Repository, AppError> {
  if sync_dir.join(".git").exists() {
    Ok(Repository::open(sync_dir)?)
  } else {
    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(setup_git_callbacks(ssh_identity));

    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_opts);
    builder.clone(git_url, sync_dir).map_err(AppError::from)
  }
}

fn main() -> Result<(), AppError> {
  let args = CliArgs::parse();
  init_logger();
  info!(
    "Syncing {} to {}...",
    &args.git_url,
    args.sync_dir.display()
  );

  fs::create_dir_all(&args.sync_dir)?;
  let repo = open_or_clone_repo(
    args.ssh_identity.as_deref(),
    &args.git_url,
    &args.sync_dir,
  )?;
  env::set_current_dir(&args.sync_dir)?;

  let statuses = repo.statuses(None)?;
  if !statuses.is_empty() {
    let user = env::var("USER").unwrap_or("unknown".to_string());
    let host = env::var("HOSTNAME").unwrap_or("localhost".to_string());
    let time =
      chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);

    let mut config = repo.config()?;
    config.set_str("user.email", &format!("{}@{}", user, host))?;
    config.set_str("user.name", &user)?;

    let mut index = repo.index()?;
    index.add_all(["."], git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let head_ref = repo.head()?;
    let head = repo.head()?.peel_to_commit()?;

    repo.commit(
      Some("HEAD"),
      &repo.signature()?,
      &repo.signature()?,
      &format!("Update from WebDAV changes on {}", time),
      &tree,
      &[&head],
    )?;

    let mut remote = repo.find_remote("origin")?;
    remote.fetch(
      &["refs/heads/*:refs/remotes/origin/*"],
      Some(
        &mut FetchOptions::new()
          .remote_callbacks(setup_git_callbacks(args.ssh_identity.as_deref())),
      ),
      None,
    )?;

    let head_annotated = repo.reference_to_annotated_commit(&head_ref)?;
    // Determine upstream (remote's HEAD)
    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let upstream = repo.reference_to_annotated_commit(&fetch_head)?;

    let mut rebase =
      repo.rebase(Some(&head_annotated), Some(&upstream), None, None)?;
    while let Some(_op) = rebase.next() {
      let sig = repo.signature()?;
      // Try to auto-resolve conflicts favoring theirs
      if repo.index()?.has_conflicts() {
        let mut merge_opts = MergeOptions::new();
        merge_opts.file_favor(FileFavor::Theirs);

        let mut index = repo.index()?;
        // Add all with "theirs" resolution
        index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
        if index.has_conflicts() {
          panic!("Conflict detected during rebase — aborting! Repo left in dirty state for debugging.");
        }
      }

      // Commit the operation
      let tree_oid = repo.index()?.write_tree()?;
      let _tree = repo.find_tree(tree_oid)?;
      rebase.commit(Some(&sig), &sig, None)?;
      repo.checkout_index(
        Some(&mut repo.index()?),
        Some(&mut CheckoutBuilder::new()),
      )?;
    }

    rebase.finish(None)?;
    info!("Rebased HEAD onto upstream with theirs auto-resolution");

    repo
      .find_remote("origin")?
      .push(&["refs/heads/*:refs/heads/*"], None)?;
  }

  Ok(())
}

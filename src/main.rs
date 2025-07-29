mod cli;
mod error;

use crate::{cli::CliArgs, error::AppError};
use clap::Parser;
use git2::{
  build::{CheckoutBuilder, RepoBuilder},
  Cred, FetchOptions, FileFavor, IndexAddOption, MergeOptions, RemoteCallbacks,
  Repository,
};
use std::env;
use std::fs;
use std::path::PathBuf;

fn setup_git_callbacks() -> RemoteCallbacks<'static> {
  let mut callbacks = RemoteCallbacks::new();

  callbacks.credentials(|_url, username_from_url, _allowed_types| {
    Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
  });

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
  git_url: &str,
  sync_dir: &PathBuf,
) -> Result<Repository, AppError> {
  if sync_dir.join(".git").exists() {
    Ok(Repository::open(sync_dir)?)
  } else {
    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(setup_git_callbacks());

    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_opts);
    builder.clone(git_url, sync_dir).map_err(AppError::from)
  }
}

fn main() -> Result<(), AppError> {
  let args = CliArgs::parse();

  fs::create_dir_all(&args.sync_dir)?;
  let repo = open_or_clone_repo(&args.git_url, &args.sync_dir)?;
  env::set_current_dir(&args.sync_dir)?;

  // Check for uncommitted changes.
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
      Some(&mut FetchOptions::new().remote_callbacks(setup_git_callbacks())),
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
      let tree = repo.find_tree(tree_oid)?;
      rebase.commit(Some(&sig), &sig, None)?;
      repo.checkout_index(
        Some(&mut repo.index()?),
        Some(&mut CheckoutBuilder::new()),
      )?;
    }

    rebase.finish(None)?;
    println!("Rebased HEAD onto upstream with theirs auto-resolution");

    // Merge commit approach.  On hold.
    // let origin_head = repo.find_reference("FETCH_HEAD")?;
    // let origin_commit = repo.reference_to_annotated_commit(&origin_head)?;
    // let (analysis, _) = repo.merge_analysis(&[&origin_commit])?;

    // if analysis.is_up_to_date() {
    //   println!("Already up-to-date");
    //   return Ok(());
    // }

    // let local_branch = repo.find_branch("main", git2::BranchType::Local)?;
    // let local_commit = local_branch.get().peel_to_commit()?;

    // if analysis.is_fast_forward() {
    //   let refname = head.name().expect("HEAD should have a name");
    //   let mut reference = repo.find_reference(refname)?;
    //   reference.set_target(origin_commit.id(), "Fast-forward")?;
    //   repo.set_head(refname)?;
    //   repo.checkout_head(Some(CheckoutBuilder::new().force()))?;
    //   println!("Fast-forwarded {}", refname);
    //   return Ok(());
    // } else if analysis.is_normal() {
    //   // Merge with "theirs" preference
    //   let mut merge_opts = MergeOptions::new();
    //   merge_opts.file_favor(FileFavor::Theirs);

    //   repo.merge(&[&origin_commit], Some(&mut merge_opts), None)?;

    //   let mut index = repo.index()?;
    //   if index.has_conflicts() {
    //     // Auto-abort to avoid leaving dirty state
    //     repo.checkout_head(Some(CheckoutBuilder::new().force()))?;
    //     repo.cleanup_state()?;
    //     return Err(git2::Error::from_str(
    //       "Merge resulted in unresolvable conflicts, aborted",
    //     ));
    //   }
    //   // Write merge commit
    //   let sig = repo.signature()?;
    //   let tree_oid = index.write_tree()?;
    //   let tree = repo.find_tree(tree_oid)?;
    //   let other_commit = repo.find_commit(origin_commit.id())?;

    //   let merge_commit = repo.commit(
    //     Some("HEAD"),
    //     &sig,
    //     &sig,
    //     "Merged origin (favoring theirs)",
    //     &tree,
    //     &[&head_commit, &other_commit],
    //   )?;

    // }

    // repo
    //   .head()?
    //   .name()
    //   .ok_or_else(|| git2::Error::from_str("Invalid HEAD ref"))?
    //   .to_string();
    // repo.merge(
    //   // TODO: Figure out the main branch.
    //   [ AnnotatedCommit::refname("origin/master") ],
    //   Some(MergeOptions.file_favor(FileFavor::Theirs)),
    // )?;
    // repo.cleanup_state()?;

    repo
      .find_remote("origin")?
      .push(&["refs/heads/*:refs/heads/*"], None)?;
  }

  Ok(())
}

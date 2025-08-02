mod cli;
mod error;
mod git;
mod logger;

use crate::{cli::CliArgs, error::AppError};
use clap::Parser;
use git::{
  commit_all, fetch_options, git_callbacks, is_local_behind_remote,
  main_branch, push_options,
};
use git2::{
  build::{CheckoutBuilder, RepoBuilder},
  Direction, FileFavor, IndexAddOption, MergeOptions, PushOptions, Repository,
};
use log::*;
use logger::init_logger;
use std::fs;
use std::path::PathBuf;
use std::{env, path::Path};
use tap::Tap;

fn open_or_clone_repo(
  ssh_identity: Option<&Path>,
  git_url: &str,
  sync_dir: &PathBuf,
) -> Result<Repository, AppError> {
  if sync_dir.join(".git").exists() {
    info!("{} is a git repo.", sync_dir.display());
    Ok(Repository::open(sync_dir)?)
  } else {
    info!(
      "{} is not a git repo.  Cloning there now...",
      sync_dir.display()
    );
    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_options(ssh_identity));
    builder.clone(git_url, sync_dir).map_err(AppError::from)
  }
}

// Partially vibe coded.  Forgive me.  I have refactored some of it, but still a
// monolith that needs smaller functions broken out.
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
  let mut remote = if let Ok(r) = repo.find_remote("origin") {
    r
  } else {
    repo.remote("origin", &args.git_url)?
  };
  info!("Connecting?");
  remote.connect_auth(
    Direction::Fetch,
    Some(git_callbacks(args.ssh_identity.as_deref())),
    None,
  )?;
  info!("Connected?");
  // This doesn't actually track the remote branch's default branch.  This just
  // asks the git configuration (system?) what the default branch is.  If the
  // repository's default branch differs from the system configuration default
  // branch, you will have a mismatch.
  // let default_branch = remote.default_branch()?;
  // let branch_name = default_branch.as_str().unwrap();
  let head_ref = repo.head()?;
  let branch_name = main_branch(&repo)?;
  info!("Main branch is: {}", branch_name);
  // Ensure we are on a local branch, not detached at origin/<branch>.
  if !repo.head()?.is_branch() {
    info!("Need to get on a local branch.");
    let branch_ref = format!("refs/remotes/origin/{}", branch_name);
    let target = repo.find_reference(&branch_ref)?.peel_to_commit()?;
    // create local branch if it doesn’t exist yet.
    if repo
      .find_branch(&branch_name, git2::BranchType::Local)
      .is_err()
    {
      repo.branch(&branch_name, &target, true)?;
    }
    repo.set_head(&format!("refs/heads/{}", branch_name))?;
    repo.checkout_head(Some(
      git2::build::CheckoutBuilder::new()
        .allow_conflicts(true)
        .force(),
    ))?;
    info!("Forcibly moved to local branch.");
  }
  let statuses = repo.statuses(None)?;
  if !statuses.is_empty() {
    info!("Local changes detected.  Committing...");
    // Refresh HEAD to avoid stale parent OID.
    commit_all(&repo)?;
  } else {
    info!("No local changes detected.");
  }
  info!("Fetching new changes from remote...");
  remote.fetch(
    &[format!(
      "+refs/heads/{}:refs/remotes/origin/{}",
      &branch_name, &branch_name,
    )],
    Some(&mut fetch_options(args.ssh_identity.as_deref())),
    None,
  )?;
  info!("Fetch complete.");
  let head_annotated = repo.reference_to_annotated_commit(&head_ref)?;
  // Determine upstream (remote's HEAD).
  let long_branch = format!("refs/remotes/origin/{}", &branch_name);
  let upstream = repo.reference_to_annotated_commit(
    &repo.find_reference(long_branch.as_str())?,
  )?;
  if is_local_behind_remote(&repo)? {
    info!("Local is behind remote.  Rebasing...");
    let mut rebase =
      repo.rebase(Some(&head_annotated), Some(&upstream), None, None)?;
    while let Some(_op) = rebase.next() {
      let sig = repo.signature()?;
      // Try to auto-resolve conflicts favoring theirs.
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
      // Commit the operation.
      let tree_oid = repo.index()?.write_tree()?;
      let _tree = repo.find_tree(tree_oid)?;
      rebase.commit(Some(&sig), &sig, None)?;
      repo.checkout_index(
        Some(&mut repo.index()?),
        Some(&mut CheckoutBuilder::new()),
      )?;
    }
    rebase.finish(None)?;
    info!("Rebased HEAD onto upstream with --theirs auto-resolution.");
  }
  info!("Pushing changes to remote...");
  remote.push(
    &[format!(
      "+refs/heads/{}:refs/remotes/origin/{}",
      &branch_name, &branch_name,
    )],
    Some(&mut push_options(args.ssh_identity.as_deref())),
  )?;
  info!("Success!");
  Ok(())
}

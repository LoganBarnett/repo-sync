use git2::{
  CertificateCheckStatus, Commit, Cred, FetchOptions, IndexAddOption, Oid,
  PushOptions, RemoteCallbacks, Repository,
};
use log::*;
use std::{env, path::Path};
use tap::Tap;

use crate::error::AppError;

pub fn commit_all(repo: &Repository) -> Result<Oid, AppError> {
  let user = env::var("USER").unwrap_or("unknown".to_string());
  let host = env::var("HOSTNAME").unwrap_or("localhost".to_string());
  let time =
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
  let mut config = repo.config()?;
  config.set_str("user.email", &format!("{}@{}", user, host))?;
  config.set_str("user.name", &user)?;
  // Stage changes.
  let mut index = repo.index()?;
  index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
  index.write()?;
  let tree_id = index.write_tree()?;
  let tree = repo.find_tree(tree_id)?;
  // Get author/committer.
  let sig = repo.signature()?;
  // Figure out parents: empty if first commit, else current HEAD.
  let parents: Vec<Commit> = if repo.is_empty()? {
    Vec::new()
  } else {
    vec![repo.head()?.peel_to_commit()?]
  };
  // Borrow for libgit2 call.
  let parent_refs: Vec<&Commit> = parents.iter().collect();
  // Create commit, advancing HEAD if possible.
  let oid = repo.commit(
    if repo.is_empty()? { None } else { Some("HEAD") },
    &sig,
    &sig,
    &format!("Update from WebDAV changes on {}", time),
    &tree,
    &parent_refs,
  )?;
  Ok(oid)
}

pub fn compare_local_and_remote(
  repo: &Repository,
) -> Result<(usize, usize), AppError> {
  let head = repo.head()?;
  let head_commit = head.peel_to_commit()?;
  let branch = head
    .shorthand()
    // TODO: Map to AppErr.
    .ok_or_else(|| git2::Error::from_str("No branch"))?;
  let remote_ref =
    repo.find_reference(&format!("refs/remotes/origin/{}", branch))?;
  let remote_commit = remote_ref.peel_to_commit()?;
  let (ahead, behind) =
    repo.graph_ahead_behind(head_commit.id(), remote_commit.id())?;
  Ok((ahead, behind))
}

pub fn fetch_options(ssh_identity: Option<&Path>) -> FetchOptions<'static> {
  // Due to how this is setup in git2, and perhaps also on Rust's side of
  // things, you can't just chain + return this value directly.  Local variables
  // FTL.
  let mut opts = FetchOptions::new();
  opts.remote_callbacks(git_callbacks(ssh_identity.as_deref()));
  opts
}

pub fn git_callbacks(
  key_path_maybe: Option<&Path>,
) -> RemoteCallbacks<'static> {
  info!("Setting up git authentication callbacks...");
  let mut callbacks = RemoteCallbacks::new();
  if let Some(key_path) = key_path_maybe {
    // This is the Path equivalent of .clone().
    let key_path_copy = key_path.to_path_buf();
    callbacks.credentials(move |_url, username_from_url, _allowed_types| {
      let username = username_from_url
        .map(|s| s.to_string())
        .unwrap_or_else(|| whoami::username())
        .tap(|x| info!("Username: {}", x));
      // Alas, this would be great if it weren't for our sparse options for
      // dealing with SSH agents.  The most "complete" solution is
      // ssh-agent-client-rs, but that crate systemically uses `unwrap` instead of
      // proper Rust error handling.  This makes debugging and clean error
      // propagation impossible.
      // Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
      Cred::ssh_key(
        &username,
        None, // Let git2 find the public key automatically.
        &key_path_copy,
        None, // passphrase.
      )
    });
  }
  // TODO: GitHub's keys are both documented and finite, so this shouldn't be
  // needed.  I'll have to add them to my local configuration, and document how
  // that was done.
  callbacks.certificate_check(|_cert, _valid| {
    // Trust all certificates (like StrictHostKeyChecking=no)
    Ok(CertificateCheckStatus::CertificateOk)
  });
  callbacks
}

pub fn is_local_behind_remote(repo: &Repository) -> Result<bool, AppError> {
  let (_, behind) = compare_local_and_remote(&repo)?;
  Ok(behind > 0)
}

pub fn main_branch(repo: &Repository) -> Result<String, AppError> {
  // This doesn't actually track the remote branch's default branch.  This just
  // asks the git configuration (system?) what the default branch is.  If the
  // repository's default branch differs from the system configuration default
  // branch, you will have a mismatch.
  // let default_branch = remote.default_branch()?;
  // let branch_name = default_branch.as_str().unwrap();
  let head_ref = repo.head()?;
  // TODO: Remove unwrap.
  let branch_name = head_ref
    .shorthand()
    .ok_or(AppError::GitBranchMissingError)?;
  Ok(branch_name.to_string())
}

pub fn push_options(ssh_identity: Option<&Path>) -> PushOptions<'static> {
  // Due to how this is setup in git2, and perhaps also on Rust's side of
  // things, you can't just chain + return this value directly.  Local variables
  // FTL.
  let mut opts = PushOptions::new();
  opts.remote_callbacks(git_callbacks(ssh_identity.as_deref()));
  opts
}

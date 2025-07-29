use clap::Parser;
use git2::{
  build::RepoBuilder, Cred, FetchOptions, RemoteCallbacks, Repository,
};
use ssh_agent::proto::RequestIdentities;
use ssh_agent::AgentClient;
use std::env;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
  #[arg(long)]
  git_url: String,
  #[arg(long)]
  ssh_identity: PathBuf,
  #[arg(long)]
  sync_dir: PathBuf,
}

#[derive(Error, Debug)]
enum SyncError {
  #[error("SSH agent socket not set in environment")]
  MissingSshAuthSock,
  #[error("Failed to connect to SSH agent: {0}")]
  SshAgentConnect(#[from] std::io::Error),
  #[error("Failed to use libgit2: {0}")]
  Git(#[from] git2::Error),
  #[error("I/O error: {0}")]
  Io(#[from] std::io::Error),
  #[error("Home directory not found")]
  NoHomeDir,
}

fn setup_git_callbacks() -> RemoteCallbacks<'static> {
  let mut callbacks = RemoteCallbacks::new();

  callbacks.credentials(|_url, username_from_url, _allowed_types| {
    Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
  });

  callbacks.certificate_check(|_cert, _valid, _host| {
    // Trust all certificates (like StrictHostKeyChecking=no)
    true
  });

  callbacks
}

fn open_or_clone_repo(
  git_url: &str,
  sync_dir: &PathBuf,
) -> Result<Repository, SyncError> {
  if sync_dir.join(".git").exists() {
    Ok(Repository::open(sync_dir)?)
  } else {
    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(setup_git_callbacks());

    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_opts);
    builder.clone(git_url, sync_dir).map_err(SyncError::from)
  }
}

fn main() -> Result<(), SyncError> {
  let args = Args::parse();

  // Check SSH agent is ready.
  let ssh_sock =
    env::var("SSH_AUTH_SOCK").map_err(|_| SyncError::MissingSshAuthSock)?;
  let mut agent = AgentClient::connect(ssh_sock)?;
  let _ids = agent.request_identities(RequestIdentities)?;

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

    let head_ref = repo
      .head()?
      .name()
      .ok_or_else(|| git2::Error::from_str("Invalid HEAD ref"))?
      .to_string();
    repo.merge_branches(
      "HEAD",
      &format!("origin/{}", head_ref.trim_start_matches("refs/heads/")),
      None,
    )?;

    repo
      .find_remote("origin")?
      .push(&["refs/heads/*:refs/heads/*"], None)?;
  }

  Ok(())
}

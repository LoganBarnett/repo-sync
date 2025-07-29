use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
  #[error("SSH agent socket not set in environment")]
  MissingSshAuthSock,
  #[error("Failed to use libgit2: {0}")]
  Git(#[from] git2::Error),
  #[error("I/O error: {0}")]
  Io(#[from] std::io::Error),
  #[error("Home directory not found")]
  NoHomeDir,
}

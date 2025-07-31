use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
  #[error("Failed to use libgit2: {0}")]
  Git(#[from] git2::Error),
  #[error("I/O error: {0}")]
  Io(#[from] std::io::Error),
  #[error("Error using SSH agent: {0}")]
  SshAgent(#[from] ssh_agent_client_rs::Error),
  #[error("SSH_AUTH_SOCK environment variable is not defined.")]
  SshAgentSocketMissing(#[from] std::env::VarError),
}

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
  #[error("Failed to use libgit2: {0}")]
  Git(#[from] git2::Error),
  #[error("Git default branch is missing.")]
  GitBranchMissingError,
  // #[error("I/O error: {0}")]
  // Io(#[from] std::io::Error),
  #[error("Error reading private key: {0}")]
  SshPrivateKeyReadError(#[from] std::io::Error),
  #[error("Error using SSH agent: {0}")]
  SshAgent(#[from] ssh_agent_client_rs::Error),
  #[error("Error starting SSH agent: {0}")]
  SshAgentStartupSpawnError(std::io::Error),
  #[error("Error starting SSH agent: {0}")]
  SshAgentStartupAgentMissingError(String),
  #[error("Error starting SSH agent: {0}")]
  SshAgentStartupInvalidOutputError(std::string::FromUtf8Error),
  #[error("Error starting SSH agent: {0}")]
  SshAgentStartupCommandFailedError(std::io::Error),
  #[error("SSH_AUTH_SOCK environment variable is not defined.")]
  SshAgentSocketMissing(#[from] std::env::VarError),
  #[error("Failed to parse SSH identity: {0}")]
  SshIdentityParseFailedError(ssh_key::Error),
}

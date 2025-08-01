use crate::AppError;
use log::*;
use ssh_agent_client_rs::Client;
use ssh_key::PrivateKey;
use std::env;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn start_agent() -> Result<(), AppError> {
  info!("Starting SSH agent...");
  let output = Command::new("ssh-agent")
    .arg("-s") // sh-style output.
    .stdout(Stdio::piped())
    .spawn()
    .map_err(AppError::SshAgentStartupSpawnError)?
    .wait_with_output()
    .map_err(AppError::SshAgentStartupCommandFailedError)?;
  let text = String::from_utf8(output.stdout)
    .map_err(AppError::SshAgentStartupInvalidOutputError)?;
  for line in text.lines() {
    if let Some(rest) = line.strip_prefix("SSH_AUTH_SOCK=") {
      let path = rest.split(';').next().ok_or_else(|| {
        AppError::SshAgentStartupAgentMissingError(line.to_string())
      })?;
      env::set_var("SSH_AUTH_SOCK", path);
      info!("SSH agent start on {}", path);
      break;
    }
  }
  Ok(())
}

pub fn add_key_to_agent(key_path: &Path) -> Result<Client, AppError> {
  let private_key =
    PrivateKey::from_openssh(std::fs::read_to_string(&key_path)?)
      // let private_key = PrivateKey::from_bytes(
      //   std::fs::read_to_string(&key_path)
      //     .map_err(AppError::SshPrivateKeyReadError)
      //     ?.as_bytes(),
      // )
      .map_err(AppError::SshIdentityParseFailedError)?;
  let sock_path = env::var("SSH_AUTH_SOCK")?;
  info!("Found SSH agent on {}", sock_path);
  let mut client = Client::connect(Path::new(&sock_path))?;
  info!("Adding identity {} to SSH agent...", key_path.display());
  client.add_identity(&private_key)?;
  let identities = client.list_all_identities()?;
  info!("Listing identities...");
  for identity in identities {
    info!("Found agent identity: {:?}", &identity);
  }
  Ok(client)
}

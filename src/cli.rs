use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct CliArgs {
  #[arg(long)]
  pub git_url: String,
  // #[arg(long)]
  // pub ssh_identity: Some(PathBuf),
  #[arg(long)]
  pub sync_dir: PathBuf,
}

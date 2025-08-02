use assert_cmd::Command;
use assert_fs::TempDir;
use git2::{Repository, Signature, Sort};
use repo_sync::git::main_branch;
use std::fs::{read_to_string, remove_file, write, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Output;

// To keep testing simple, let's just have a enum that describes changes.
// Assume that all files contain a single number, and we're just altering that
// number.  As more sophisticated test cases come up, we can expand this, but we
// just need something to get us going for now.
enum ChangeType {
  CreateFile,
  Increment,
  Decrement,
  RemoveFile,
}

struct TestCommit {
  user: String,
  message: String,
  change_type: ChangeType,
}

fn apply_change(
  base_dir: &Path,
  commit: &TestCommit,
) -> Result<(), Box<dyn std::error::Error>> {
  let test_file = base_dir.join("test-file.txt");
  match &commit.change_type {
    ChangeType::CreateFile => {
      let mut file = File::create(&test_file)?;
      file.write_all(b"0")?;
    }
    ChangeType::RemoveFile => remove_file(&test_file)?,
    ChangeType::Increment => {
      let data = read_to_string(&test_file)?;
      let number: i64 = data.parse()?;
      write(&test_file, (number + 1).to_string())?;
    }
    ChangeType::Increment => {
      let data = read_to_string(&test_file)?;
      let number: i64 = data.parse()?;
      write(&test_file, (number - 1).to_string())?;
    }
    ChangeType::Decrement => {
      todo!();
    }
  };
  Ok(())
}

fn commit_count(repo: &Repository) -> Result<usize, git2::Error> {
  let mut revwalk = repo.revwalk()?;
  // Start from HEAD.
  revwalk.push_head()?;
  // Or Sort::TIME.
  revwalk.set_sorting(Sort::TOPOLOGICAL)?;
  let mut count = 0;
  for oid_result in revwalk {
    oid_result?; // ensure we consume errors
    count += 1;
  }
  Ok(count)
}

// libgit2 doesn't support the notion of pushing to a non-bare repository.
// Allowing that would make the act of setting up and managing a test repository
// very easy, but it won't work here.  So instead we create a mirror repository
// whose purpose is to perform writes (commits) and pushes them to the original,
// bare repository.
fn host_repository(
  authoritative_directory: &Path,
  write_directory: &Path,
  commits: &Vec<TestCommit>,
) -> Result<(), Box<dyn std::error::Error>> {
  let _bare_repo = Repository::init_bare(&authoritative_directory)?;
  let write_repo = Repository::clone(
    &authoritative_directory.to_str().unwrap(),
    &write_directory,
  )?;
  for commit in commits {
    let mut index = write_repo.index()?;
    let signature = Signature::now(
      &commit.user.clone(),
      &format!("{}@email.com", commit.user.clone()),
    )?;
    apply_change(&write_directory, &commit)?;
    index.add_all(["."], git2::IndexAddOption::DEFAULT, None)?;
    let tree_oid = index.write_tree()?;
    let tree = write_repo.find_tree(tree_oid)?;
    write_repo.commit(
      "HEAD".into(),
      &signature,
      &signature,
      &commit.message,
      &tree,
      &[], // parents,
    )?;
    let mut remote = write_repo.find_remote("origin")?;
    let branch_name = main_branch(&write_repo)?;
    remote.push(
      &[format!(
        "+refs/heads/{}:refs/heads/{}",
        branch_name, branch_name
      )],
      None,
    )?;
  }
  Ok(())
}

// Use this to detect status of the repository on an error.
fn git_status(
  repo_path: &std::path::Path,
) -> Result<Output, Box<dyn std::error::Error>> {
  let output = Command::new("git")
    .arg("-C")
    .arg(repo_path)
    .arg("status")
    // We intentionally want human readability here.
    // .arg("--porcelain")
    .output()?;
  if !output.status.success() {
    return Err(
      format!(
        "git status failed: {}",
        String::from_utf8_lossy(&output.stderr)
      )
      .into(),
    );
  }
  Ok(output)
}

// Perhaps a silly test, but a grounding one.
#[test]
fn version_works() -> Result<(), Box<dyn std::error::Error>> {
  let tmp = TempDir::new()?;
  let mut cmd = Command::cargo_bin("repo-sync")?;
  cmd
    .current_dir(&tmp.path())
    .arg("--version")
    .assert()
    .success();
  Ok(())
}

#[test]
fn clones_missing_repository_fs() -> Result<(), Box<dyn std::error::Error>> {
  let host_tmp = TempDir::new()?;
  let write_tmp = TempDir::new()?;
  let commits = vec![TestCommit {
    user: "taco".into(),
    change_type: ChangeType::CreateFile,
    message: "birth the universe".into(),
  }];
  host_repository(host_tmp.path(), write_tmp.path(), &commits)?;
  let sync_tmp = TempDir::new()?;
  let mut cmd = Command::cargo_bin("repo-sync")?;
  cmd
    .current_dir(&sync_tmp.path())
    .arg("--git-url")
    .arg(host_tmp.path())
    .arg("--sync-dir")
    .arg(sync_tmp.path())
    .assert()
    .success();
  // Ensure the directory is cloned.
  assert!(sync_tmp.path().join(".git").exists());
  let local_repo = Repository::open(sync_tmp.path())?;
  // Ensure the expected change is present.
  assert!(commit_count(&local_repo)? == 1);
  Ok(())
}

#[test]
fn loads_with_ssh() -> Result<(), Box<dyn std::error::Error>> {
  let host_tmp = TempDir::new()?;
  let write_tmp = TempDir::new()?;
  let commits = vec![TestCommit {
    user: "taco".into(),
    change_type: ChangeType::CreateFile,
    message: "birth the universe".into(),
  }];
  host_repository(host_tmp.path(), write_tmp.path(), &commits)?;
  let sync_tmp = TempDir::new()?;
  let mut cmd = Command::cargo_bin("repo-sync")?;
  cmd
    .current_dir(&sync_tmp.path())
    .arg("--git-url")
    .arg(host_tmp.path())
    .arg("--sync-dir")
    .arg(sync_tmp.path())
    .arg("--ssh-identity")
    // A steep and unsafe assumption.  Perhaps we can create an account and add
    // their key in the repo?  Or declare an environment variable which defaults
    // to this.
    // This is likely not Windows friendly too, but I'll let a Windows user
    // figure that out.
    .arg(format!("{}/.ssh/id_rsa", std::env::var("HOME").unwrap()))
    .assert()
    .success();
  // Ensure the directory is cloned.
  assert!(sync_tmp.path().join(".git").exists());
  let local_repo = Repository::open(sync_tmp.path())?;
  // Ensure the expected change is present.
  assert!(commit_count(&local_repo)? == 1);
  Ok(())
}

#[test]
fn commits_changes() -> Result<(), Box<dyn std::error::Error>> {
  let host_tmp = TempDir::new()?;
  let write_tmp = TempDir::new()?;
  let commits = vec![TestCommit {
    user: "taco".into(),
    change_type: ChangeType::CreateFile,
    message: "birth the universe".into(),
  }];
  host_repository(host_tmp.path(), write_tmp.path(), &commits)?;
  let sync_tmp = TempDir::new()?;
  let mut cmd = Command::cargo_bin("repo-sync")?;
  cmd
    .current_dir(&sync_tmp.path())
    .arg("--git-url")
    .arg(host_tmp.path())
    .arg("--sync-dir")
    .arg(sync_tmp.path())
    .assert()
    .success();
  let local_repo = Repository::open(sync_tmp.path())?;
  assert!(commit_count(&local_repo)? == 1);
  write(sync_tmp.path().join("new-file"), "foo")?;
  let _ = cmd.assert().try_success().map_err(|err| {
    // This retains the pretty-print behavior from assert_cmd.
    eprintln!("assert_cmd failed:\n{err}");
    // Also show us the git status of the repo.
    match git_status(sync_tmp.path()) {
      Ok(output) => {
        eprintln!("git status success: {}", output.status);
        eprintln!(
          "git status (stdout):\n{}",
          String::from_utf8(output.stdout).unwrap(),
        );
        eprintln!(
          "git status (stderr):\n{}",
          String::from_utf8(output.stderr).unwrap(),
        );
      }
      Err(e) => eprintln!("git_status failed: {e}"),
    }
    // bubble the original error upward
    Box::new(err) as Box<dyn std::error::Error>
  });
  assert!(commit_count(&local_repo)? == 2);
  Ok(())
}

// #[test]
fn does_nothing_with_no_changes() -> Result<(), Box<dyn std::error::Error>> {
  let tmp = TempDir::new()?;
  let mut cmd = Command::cargo_bin("repo-sync")?;
  cmd.current_dir(&tmp).arg("").assert().success();
  Ok(())
}

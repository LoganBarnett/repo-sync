use assert_cmd::Command;
use assert_fs::TempDir;
use git2::{Repository, Signature};
use std::fs::{read_to_string, remove_file, write, File};
use std::io::Write;
use std::path::{Path, PathBuf};

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

fn host_repository(
  directory: &Path,
  commits: &Vec<TestCommit>,
) -> Result<(), Box<dyn std::error::Error>> {
  let repo = Repository::init(&directory)?;
  for commit in commits {
    let mut index = repo.index()?;
    let signature = Signature::now(
      &commit.user.clone(),
      &format!("{}@email.com", commit.user.clone()),
    )?;
    apply_change(&directory, &commit)?;
    index.add_all(["."], git2::IndexAddOption::DEFAULT, None)?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;
    repo.commit(
      "HEAD".into(),
      &signature,
      &signature,
      &commit.message,
      &tree,
      &[], // parents,
    );
  }
  Ok(())
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
fn clones_missing_repository() -> Result<(), Box<dyn std::error::Error>> {
  let host_tmp = TempDir::new()?;
  let commits = vec![TestCommit {
    user: "taco".into(),
    change_type: ChangeType::CreateFile,
    message: "birth the universe".into(),
  }];
  host_repository(host_tmp.path(), &commits)?;
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
  // Ensure the expected change is present.
  assert!(sync_tmp.path().join(".git").exists());
  Ok(())
}

// #[test]
fn does_nothing_with_no_changes() -> Result<(), Box<dyn std::error::Error>> {
  let tmp = TempDir::new()?;
  let mut cmd = Command::cargo_bin("repo-sync")?;
  cmd.current_dir(&tmp).arg("").assert().success();
  Ok(())
}

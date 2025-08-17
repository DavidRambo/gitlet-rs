//! Tests the rm command.

use std::error::Error;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::predicate;

#[test]
fn cannot_rm_untracked_file() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::new("touch");
    cmd.current_dir(&tmpdir).arg("a.txt");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("rm").arg("a.txt");
    cmd.assert().success().stdout(predicate::str::contains(
        "Cannot remove file. The file is not tracked.",
    ));

    Ok(())
}

#[test]
fn cannot_rm_staged_changes_file() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;
    let tmp_path = format!("{}/a.txt", tmpdir.display());

    let mut cmd = Command::new("touch");
    cmd.current_dir(&tmpdir).arg("a.txt");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg("a.txt");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("rm").arg("a.txt");
    cmd.assert().failure().stderr(predicate::str::contains(
        "Cannot remove a file with staged changes.",
    ));

    assert!(std::fs::exists(&tmp_path)?);

    Ok(())
}

#[test]
fn rm_cached() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;
    let tmp_path = format!("{}/a.txt", tmpdir.display());

    let mut cmd = Command::new("touch");
    cmd.current_dir(&tmpdir).arg("a.txt");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg("a.txt");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir)
        .arg("rm")
        .arg("--cached")
        .arg("a.txt");
    cmd.assert().success();

    assert!(std::fs::exists(&tmp_path)?);

    Ok(())
}

#[test]
fn rm_from_working_tree() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::new("touch");
    cmd.current_dir(&tmpdir).arg("a.txt");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg("a.txt");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("commit").arg("test commit");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("rm").arg("a.txt");
    cmd.assert().success();

    let tmp_path = format!("{}/a.txt", tmpdir.display());
    assert!(!std::fs::exists(&tmp_path)?);

    Ok(())
}

#[test]
fn rm_already_removed() -> Result<(), Box<dyn Error>> {
    let tmpdir = assert_fs::TempDir::new()?;

    let mut cmd = Command::new("touch");
    cmd.current_dir(&tmpdir).arg("a.txt");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("add").arg("a.txt");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("commit").arg("test commit");
    cmd.assert().success();

    // Delete file from working tree and add it to 'removals' in staging area.
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("rm").arg("a.txt");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("rm 'a.txt'"));
    assert!(!std::fs::exists(tmpdir.join("a.txt"))?);

    // Will not show up as already staged for removal because the file has been deleted.
    let mut cmd = Command::cargo_bin("gitlet")?;
    cmd.current_dir(&tmpdir).arg("rm").arg("a.txt");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("File already staged for removal."));

    Ok(())
}

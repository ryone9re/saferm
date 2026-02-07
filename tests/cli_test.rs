use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn saferm() -> assert_cmd::Command {
    cargo_bin_cmd!("saferm")
}

#[test]
fn test_help() {
    saferm()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("safe rm replacement"));
}

#[test]
fn test_version() {
    saferm()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("saferm"));
}

#[test]
fn test_no_args() {
    saferm()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn test_nonexistent_file() {
    saferm()
        .args(["-f", "/nonexistent/path/to/file.txt"])
        .assert()
        .success();
}

#[test]
fn test_nonexistent_file_without_force() {
    saferm()
        .arg("/nonexistent/path/to/file.txt")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such file or directory").or(
            predicate::str::contains("そのようなファイルやディレクトリはありません"),
        ));
}

#[test]
fn test_force_trash_file() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("test.txt");
    fs::write(&file, "hello world").unwrap();

    saferm()
        .args(["-f", file.to_str().unwrap()])
        .assert()
        .success();

    assert!(!file.exists(), "File should have been moved to trash");
}

#[test]
fn test_force_trash_multiple_files() {
    let tmp = TempDir::new().unwrap();
    let file1 = tmp.path().join("a.txt");
    let file2 = tmp.path().join("b.txt");
    fs::write(&file1, "aaa").unwrap();
    fs::write(&file2, "bbb").unwrap();

    saferm()
        .args(["-f", file1.to_str().unwrap(), file2.to_str().unwrap()])
        .assert()
        .success();

    assert!(!file1.exists());
    assert!(!file2.exists());
}

#[test]
fn test_directory_without_recursive_flag() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("mydir");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("file.txt"), "content").unwrap();

    saferm()
        .args(["-f", dir.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Is a directory")
                .or(predicate::str::contains("ディレクトリです")),
        );

    assert!(dir.exists(), "Directory should still exist");
}

#[test]
fn test_directory_with_recursive_flag() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("mydir");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("file.txt"), "content").unwrap();

    saferm()
        .args(["-rf", dir.to_str().unwrap()])
        .assert()
        .success();

    assert!(!dir.exists(), "Directory should have been moved to trash");
}

#[test]
fn test_verbose_output() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("verbose_test.txt");
    fs::write(&file, "data").unwrap();

    saferm()
        .args(["-fv", file.to_str().unwrap()])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("verbose_test.txt")
                .and(predicate::str::contains("trash").or(predicate::str::contains("ゴミ箱"))),
        );
}

#[test]
fn test_symlink() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("target.txt");
    let link = tmp.path().join("link.txt");
    fs::write(&target, "target content").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    saferm()
        .args(["-f", link.to_str().unwrap()])
        .assert()
        .success();

    assert!(
        link.symlink_metadata().is_err(),
        "Symlink should have been removed"
    );
    assert!(target.exists(), "Target should still exist");
}

#[test]
fn test_unicode_filename() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("日本語ファイル.txt");
    fs::write(&file, "こんにちは").unwrap();

    saferm()
        .args(["-f", file.to_str().unwrap()])
        .assert()
        .success();

    assert!(!file.exists());
}

#[test]
fn test_spaces_in_path() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("my folder");
    fs::create_dir(&dir).unwrap();
    let file = dir.join("my file.txt");
    fs::write(&file, "content").unwrap();

    saferm()
        .args(["-f", file.to_str().unwrap()])
        .assert()
        .success();

    assert!(!file.exists());
}

#[test]
fn test_double_dash_end_of_options() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("-weird-name.txt");
    fs::write(&file, "data").unwrap();

    saferm()
        .args(["-f", "--", file.to_str().unwrap()])
        .assert()
        .success();

    assert!(!file.exists());
}

#[test]
fn test_partial_failure_exits_nonzero() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("exists.txt");
    fs::write(&file, "data").unwrap();

    // Without -f, nonexistent files cause an error
    saferm()
        .args([file.to_str().unwrap(), "/nonexistent/should_fail.txt"])
        // Pipe stdin to /dev/null to auto-deny prompts, but use -f for the
        // existing file and omit it for the nonexistent one won't work since
        // -f is global. Instead, we test that running without -f against a
        // nonexistent file fails (the existing file prompt will be denied
        // due to non-interactive stdin, but that's OK for this test).
        .assert()
        .failure();
}

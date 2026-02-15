use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn saferm() -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("saferm");
    // Use managed trash backend in tests for portability across environments
    cmd.env("SAFERM_TRASH_BACKEND", "managed");
    cmd
}

/// Create a saferm command with an isolated managed trash directory.
/// Returns (Command, TempDir) — the TempDir must be kept alive for the test's duration.
fn saferm_isolated() -> (assert_cmd::Command, TempDir) {
    let trash_dir = TempDir::new().unwrap();
    let mut cmd = cargo_bin_cmd!("saferm");
    cmd.env("SAFERM_TRASH_BACKEND", "managed");
    cmd.env("SAFERM_MANAGED_TRASH_DIR", trash_dir.path());
    (cmd, trash_dir)
}

/// Create a saferm command using the same isolated trash directory.
fn saferm_with_trash(trash_dir: &TempDir) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("saferm");
    cmd.env("SAFERM_TRASH_BACKEND", "managed");
    cmd.env("SAFERM_MANAGED_TRASH_DIR", trash_dir.path());
    cmd
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

    // Without -f in non-TTY, both targets fail:
    // - existing file: refused (no TTY for confirmation)
    // - nonexistent file: not found error
    saferm()
        .args([file.to_str().unwrap(), "/nonexistent/should_fail.txt"])
        .assert()
        .failure();
}

#[test]
fn test_symlink_to_dir_without_recursive() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("realdir");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("inner.txt"), "content").unwrap();
    let link = tmp.path().join("linkdir");
    std::os::unix::fs::symlink(&dir, &link).unwrap();

    // Symlink to directory should NOT require -r (matches rm behavior)
    saferm()
        .args(["-f", link.to_str().unwrap()])
        .assert()
        .success();

    assert!(
        link.symlink_metadata().is_err(),
        "Symlink should have been removed"
    );
    assert!(dir.exists(), "Real directory should still exist");
}

#[test]
fn test_non_tty_without_force_gives_clear_error() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("test.txt");
    fs::write(&file, "data").unwrap();

    // assert_cmd runs without a TTY — should get clear error, not IO crash
    saferm()
        .arg(file.to_str().unwrap())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no TTY").or(predicate::str::contains("TTYがありません")));

    assert!(file.exists(), "File should NOT have been deleted");
}

// ===== Restore feature tests =====

#[test]
fn test_restore_flag_help() {
    saferm()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--restore"));
}

#[test]
fn test_restore_empty_trash() {
    // --restore with -f on empty managed trash should show "no items" message
    let (mut cmd, _trash_dir) = saferm_isolated();
    cmd.args(["--restore", "-f"]).assert().success().stdout(
        predicate::str::contains("No restorable items").or(predicate::str::contains(
            "復元可能なアイテムが見つかりません",
        )),
    );
}

#[test]
fn test_trash_and_restore() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("restore_me.txt");
    fs::write(&file, "important data").unwrap();
    let (mut trash_cmd, trash_dir) = saferm_isolated();

    // Trash the file
    trash_cmd
        .args(["-f", file.to_str().unwrap()])
        .assert()
        .success();
    assert!(!file.exists());

    // Restore with -f (non-interactive, selects all matching)
    saferm_with_trash(&trash_dir)
        .args(["--restore", "-f", "restore_me"])
        .assert()
        .success();

    // File should be back at original location
    assert!(file.exists(), "File should have been restored");
    assert_eq!(fs::read_to_string(&file).unwrap(), "important data");
}

#[test]
fn test_restore_with_pattern_filter() {
    let tmp = TempDir::new().unwrap();
    let file_a = tmp.path().join("alpha.txt");
    let file_b = tmp.path().join("beta.txt");
    fs::write(&file_a, "aaa").unwrap();
    fs::write(&file_b, "bbb").unwrap();
    let (mut trash_cmd, trash_dir) = saferm_isolated();

    // Trash both files
    trash_cmd
        .args(["-f", file_a.to_str().unwrap(), file_b.to_str().unwrap()])
        .assert()
        .success();
    assert!(!file_a.exists());
    assert!(!file_b.exists());

    // Restore only "alpha" pattern
    saferm_with_trash(&trash_dir)
        .args(["--restore", "-f", "alpha"])
        .assert()
        .success();

    // alpha should be restored, beta should still be in trash
    assert!(file_a.exists(), "alpha.txt should have been restored");
    assert!(!file_b.exists(), "beta.txt should still be in trash");
}

#[test]
fn test_restore_nonexistent_filter() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("exists.txt");
    fs::write(&file, "data").unwrap();
    let (mut trash_cmd, trash_dir) = saferm_isolated();

    // Trash the file
    trash_cmd
        .args(["-f", file.to_str().unwrap()])
        .assert()
        .success();

    // Restore with non-matching pattern
    saferm_with_trash(&trash_dir)
        .args(["--restore", "-f", "nonexistent_pattern"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("No restorable items").or(predicate::str::contains(
                "復元可能なアイテムが見つかりません",
            )),
        );
}

#[test]
fn test_restore_verbose_output() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("verbose_restore.txt");
    fs::write(&file, "data").unwrap();
    let (mut trash_cmd, trash_dir) = saferm_isolated();

    // Trash the file
    trash_cmd
        .args(["-f", file.to_str().unwrap()])
        .assert()
        .success();

    // Restore with verbose
    saferm_with_trash(&trash_dir)
        .args(["--restore", "-fv", "verbose_restore"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Restored").or(predicate::str::contains("復元しました")));

    assert!(file.exists());
}

#[test]
fn test_restore_cleanup_conflict() {
    // --cleanup and --restore should conflict
    saferm()
        .args(["--cleanup", "--restore"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_restore_directory() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("mydir");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("inner.txt"), "inside").unwrap();
    let (mut trash_cmd, trash_dir) = saferm_isolated();

    // Trash the directory
    trash_cmd
        .args(["-rf", dir.to_str().unwrap()])
        .assert()
        .success();
    assert!(!dir.exists());

    // Restore
    saferm_with_trash(&trash_dir)
        .args(["--restore", "-f", "mydir"])
        .assert()
        .success();

    assert!(dir.exists(), "Directory should have been restored");
    assert_eq!(fs::read_to_string(dir.join("inner.txt")).unwrap(), "inside");
}

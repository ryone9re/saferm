use anyhow::Result;
use rust_i18n::t;
use std::path::Path;

use crate::cli::Cli;
use crate::prompt::Prompter;
use crate::trash::TrashHandler;

pub fn run(cli: &Cli, handler: &dyn TrashHandler, prompter: &dyn Prompter) -> Result<bool> {
    if cli.cleanup {
        handler.cleanup(prompter)?;
        return Ok(true);
    }

    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdin());
    let mut all_ok = true;

    for target in &cli.targets {
        if let Err(e) = process_target(target, cli, handler, prompter, is_tty) {
            eprintln!("saferm: {}", e);
            all_ok = false;
        }
    }

    Ok(all_ok)
}

fn process_target(
    target: &Path,
    cli: &Cli,
    handler: &dyn TrashHandler,
    prompter: &dyn Prompter,
    is_tty: bool,
) -> Result<()> {
    // Check existence
    if !target.exists() && !target.is_symlink() {
        if cli.force {
            return Ok(());
        }
        anyhow::bail!(t!("error_not_found", name = target.display().to_string()));
    }

    // Directory check — symlinks to directories are treated as symlinks, not directories.
    // Real rm removes symlinks without -r regardless of what they point to.
    if target.is_dir() && !target.is_symlink() {
        if !cli.recursive && !cli.dir {
            anyhow::bail!(t!("error_is_dir", name = target.display().to_string()));
        }
        // -d flag only works for empty directories
        if cli.dir && !cli.recursive && target.read_dir()?.next().is_some() {
            anyhow::bail!(t!("error_is_dir", name = target.display().to_string()));
        }
    }

    // Non-TTY without -f: refuse with a clear error (never attempt interactive prompt)
    if !is_tty && !cli.force {
        anyhow::bail!(t!(
            "error_non_interactive",
            name = target.display().to_string()
        ));
    }

    // TTY: always prompt (even with -f — saferm's core safety feature)
    if is_tty {
        let msg = if target.is_dir() && !target.is_symlink() {
            t!("confirm_trash_dir", name = target.display().to_string())
        } else {
            t!("confirm_trash", name = target.display().to_string())
        };

        if !prompter.confirm(&msg)? {
            if cli.verbose {
                eprintln!("{}", t!("cancelled", name = target.display().to_string()));
            }
            return Ok(());
        }
    }
    // Non-TTY with -f: skip prompt (script/CI usage)

    // Move to trash
    handler.trash(target)?;

    if cli.verbose {
        println!(
            "{}",
            t!(
                "verbose_trashed_with_backend",
                name = target.display().to_string(),
                backend = handler.backend_name()
            )
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::AutoConfirmPrompter;
    use std::cell::RefCell;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    struct MockTrash {
        trashed: RefCell<Vec<PathBuf>>,
    }

    impl MockTrash {
        fn new() -> Self {
            Self {
                trashed: RefCell::new(Vec::new()),
            }
        }

        fn trashed_paths(&self) -> Vec<PathBuf> {
            self.trashed.borrow().clone()
        }
    }

    impl TrashHandler for MockTrash {
        fn trash(&self, path: &Path) -> Result<()> {
            self.trashed.borrow_mut().push(path.to_path_buf());
            Ok(())
        }

        fn cleanup(&self, _prompter: &dyn Prompter) -> Result<()> {
            Ok(())
        }

        fn backend_name(&self) -> &'static str {
            "mock"
        }
    }

    struct DenyPrompter;

    impl Prompter for DenyPrompter {
        fn confirm(&self, _message: &str) -> Result<bool> {
            Ok(false)
        }
    }

    fn make_cli(targets: Vec<PathBuf>, force: bool, recursive: bool, verbose: bool) -> Cli {
        Cli {
            targets,
            recursive,
            force,
            interactive: false,
            dir: false,
            verbose,
            cleanup: false,
        }
    }

    #[test]
    fn test_trash_file() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("test.txt");
        fs::write(&file, "hello").unwrap();

        let handler = MockTrash::new();
        let cli = make_cli(vec![file.clone()], true, false, false);
        let result = run(&cli, &handler, &AutoConfirmPrompter).unwrap();

        assert!(result);
        assert_eq!(handler.trashed_paths(), vec![file]);
    }

    #[test]
    fn test_nonexistent_file_without_force() {
        let handler = MockTrash::new();
        let cli = make_cli(
            vec![PathBuf::from("/nonexistent/file.txt")],
            false,
            false,
            false,
        );
        let result = run(&cli, &handler, &AutoConfirmPrompter).unwrap();

        assert!(!result);
        assert!(handler.trashed_paths().is_empty());
    }

    #[test]
    fn test_nonexistent_file_with_force() {
        let handler = MockTrash::new();
        let cli = make_cli(
            vec![PathBuf::from("/nonexistent/file.txt")],
            true,
            false,
            false,
        );
        let result = run(&cli, &handler, &AutoConfirmPrompter).unwrap();

        assert!(result);
        assert!(handler.trashed_paths().is_empty());
    }

    #[test]
    fn test_directory_without_recursive() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("mydir");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"), "hello").unwrap();

        let handler = MockTrash::new();
        let cli = make_cli(vec![dir], false, false, false);
        let result = run(&cli, &handler, &AutoConfirmPrompter).unwrap();

        assert!(!result);
        assert!(handler.trashed_paths().is_empty());
    }

    #[test]
    fn test_directory_with_recursive() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("mydir");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"), "hello").unwrap();

        let handler = MockTrash::new();
        let cli = make_cli(vec![dir.clone()], true, true, false);
        let result = run(&cli, &handler, &AutoConfirmPrompter).unwrap();

        assert!(result);
        assert_eq!(handler.trashed_paths(), vec![dir]);
    }

    #[test]
    fn test_denied_prompt() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("test.txt");
        fs::write(&file, "hello").unwrap();

        let handler = MockTrash::new();
        let cli = make_cli(vec![file.clone()], false, false, false);
        // Call process_target directly with is_tty=true to test prompt denial
        let result = process_target(&file, &cli, &handler, &DenyPrompter, true);

        assert!(result.is_ok());
        assert!(handler.trashed_paths().is_empty());
    }

    #[test]
    fn test_non_tty_without_force_refuses() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("test.txt");
        fs::write(&file, "hello").unwrap();

        let handler = MockTrash::new();
        let cli = make_cli(vec![file.clone()], false, false, false);
        // Non-TTY without -f should refuse with an error
        let result = process_target(&file, &cli, &handler, &AutoConfirmPrompter, false);

        assert!(result.is_err());
        assert!(handler.trashed_paths().is_empty());
    }

    #[test]
    fn test_multiple_targets_partial_failure() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("exists.txt");
        fs::write(&file, "hello").unwrap();
        // A non-empty directory without -r will fail
        let dir = tmp.path().join("mydir");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("inner.txt"), "data").unwrap();

        let handler = MockTrash::new();
        // force=true for non-TTY, recursive=false so directory fails
        let cli = make_cli(vec![file.clone(), dir], true, false, false);
        let result = run(&cli, &handler, &AutoConfirmPrompter).unwrap();

        // Should return false (partial failure from dir) but still trash the file
        assert!(!result);
        assert_eq!(handler.trashed_paths(), vec![file]);
    }

    #[test]
    fn test_symlink_to_dir_without_recursive() {
        let tmp = TempDir::new().unwrap();
        let real_dir = tmp.path().join("realdir");
        fs::create_dir(&real_dir).unwrap();
        let link = tmp.path().join("linkdir");
        std::os::unix::fs::symlink(&real_dir, &link).unwrap();

        let handler = MockTrash::new();
        // No -r flag — symlink to directory should still be accepted
        let cli = make_cli(vec![link.clone()], true, false, false);
        let result = run(&cli, &handler, &AutoConfirmPrompter).unwrap();

        assert!(result);
        assert_eq!(handler.trashed_paths(), vec![link]);
    }
}

use anyhow::Result;
use rust_i18n::t;
use std::path::Path;

use crate::cli::Cli;
use crate::prompt::Prompter;
use crate::trash::TrashHandler;

// chrono is used for formatting timestamps in run_restore()

pub fn run(cli: &Cli, handler: &dyn TrashHandler, prompter: &dyn Prompter) -> Result<bool> {
    if cli.cleanup {
        handler.cleanup(prompter)?;
        return Ok(true);
    }

    if cli.restore {
        return run_restore(cli, handler, prompter);
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

fn run_restore(cli: &Cli, handler: &dyn TrashHandler, prompter: &dyn Prompter) -> Result<bool> {
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdin());

    // Reject multiple filter arguments
    if cli.targets.len() > 1 {
        anyhow::bail!("--restore accepts at most one filter pattern");
    }

    // Use first target as an optional filter pattern
    let filter = cli.targets.first().and_then(|p| p.to_str());

    let items = handler.list_restorable(filter)?;

    if items.is_empty() {
        println!("{}", t!("restore_nothing"));
        return Ok(true);
    }

    // Build display list
    let display_options: Vec<String> = items
        .iter()
        .map(|item| {
            let date_str = item
                .deleted_at
                .map(|ts| {
                    chrono::DateTime::from_timestamp(ts, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                })
                .unwrap_or_else(|| "unknown".to_string());
            format!("{} ({})", item.original_path.display(), date_str)
        })
        .collect();

    // Select items to restore
    let selected = if is_tty {
        let defaults = vec![false; display_options.len()];
        let sel = prompter.multi_select(&t!("restore_select"), &display_options, &defaults)?;
        if sel.is_empty() {
            println!("{}", t!("restore_cancelled"));
            return Ok(true);
        }
        sel
    } else if cli.force {
        // Non-TTY with -f: select all
        (0..items.len()).collect()
    } else {
        anyhow::bail!(t!("error_restore_non_interactive"));
    };

    let mut all_ok = true;

    for idx in selected {
        let item = &items[idx];
        let mut dest = item.original_path.clone();

        // Ensure parent directory exists
        if let Some(parent) = dest.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)?;
        }

        // Conflict handling
        if dest.exists() {
            if !is_tty && cli.force {
                // Non-interactive: skip on conflict (safe default)
                eprintln!(
                    "{}",
                    t!(
                        "restore_skipped",
                        name = item.display_name.to_string_lossy()
                    )
                );
                continue;
            }

            let name_str = item.display_name.to_string_lossy().to_string();
            let rename_dest = generate_rename_path(&dest);
            let rename_label = t!(
                "restore_conflict_rename",
                name = rename_dest.display().to_string()
            );

            let options: Vec<String> = vec![
                t!("restore_conflict_overwrite").to_string(),
                t!("restore_conflict_skip").to_string(),
                rename_label.to_string(),
            ];

            let choice = prompter.select(
                &t!("restore_conflict", name = name_str),
                &options,
                1, // default to Skip
            )?;

            match choice {
                0 => {
                    // Overwrite: remove existing
                    // Check symlink first to avoid following symlink-to-dir
                    let meta = std::fs::symlink_metadata(&dest)?;
                    if meta.is_dir() {
                        std::fs::remove_dir_all(&dest)?;
                    } else {
                        std::fs::remove_file(&dest)?;
                    }
                }
                1 => {
                    // Skip
                    if cli.verbose {
                        eprintln!("{}", t!("restore_skipped", name = name_str));
                    }
                    continue;
                }
                _ => {
                    // Rename
                    dest = rename_dest;
                }
            }
        }

        match handler.restore_to(&item.id, &dest) {
            Ok(()) => {
                if cli.verbose {
                    println!(
                        "{}",
                        t!(
                            "restore_success",
                            name = item.display_name.to_string_lossy(),
                            path = dest.display().to_string()
                        )
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "saferm: {}",
                    t!(
                        "error_restore_failed",
                        name = item.display_name.to_string_lossy(),
                        reason = e.to_string()
                    )
                );
                all_ok = false;
            }
        }
    }

    Ok(all_ok)
}

/// Generate a rename path by appending ".restored" or a counter suffix.
fn generate_rename_path(path: &Path) -> std::path::PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str());
    let parent = path.parent().unwrap_or(Path::new("."));

    for i in 1u64.. {
        let candidate = match ext {
            Some(e) => parent.join(format!(
                "{}.restored{}.{}",
                stem,
                if i == 1 {
                    String::new()
                } else {
                    format!("{}", i)
                },
                e
            )),
            None => parent.join(format!(
                "{}.restored{}",
                stem,
                if i == 1 {
                    String::new()
                } else {
                    format!("{}", i)
                }
            )),
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
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

        fn list_restorable(
            &self,
            _filter: Option<&str>,
        ) -> Result<Vec<crate::trash::RestorableItem>> {
            Ok(vec![])
        }

        fn restore_to(&self, _item_id: &std::ffi::OsStr, _destination: &Path) -> Result<()> {
            Ok(())
        }
    }

    struct DenyPrompter;

    impl Prompter for DenyPrompter {
        fn confirm(&self, _message: &str) -> Result<bool> {
            Ok(false)
        }

        fn select(&self, _message: &str, _options: &[String], default: usize) -> Result<usize> {
            Ok(default)
        }

        fn multi_select(
            &self,
            _message: &str,
            _options: &[String],
            _defaults: &[bool],
        ) -> Result<Vec<usize>> {
            Ok(vec![])
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
            restore: false,
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

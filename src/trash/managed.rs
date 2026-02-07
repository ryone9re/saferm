use anyhow::{Context, Result};
use chrono::Local;
use rust_i18n::t;
use std::fs;
use std::path::{Path, PathBuf};

use super::TrashHandler;
use crate::prompt::Prompter;

pub struct ManagedTrash {
    base_dir: PathBuf,
}

impl Default for ManagedTrash {
    fn default() -> Self {
        Self::new()
    }
}

impl ManagedTrash {
    pub fn new() -> Self {
        let base_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("saferm")
            .join("trash");
        Self { base_dir }
    }

    #[cfg(test)]
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn files_dir(&self) -> PathBuf {
        self.base_dir.join("files")
    }

    fn info_dir(&self) -> PathBuf {
        self.base_dir.join("info")
    }

    fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.files_dir())
            .with_context(|| format!("failed to create trash files dir: {:?}", self.files_dir()))?;
        fs::create_dir_all(self.info_dir())
            .with_context(|| format!("failed to create trash info dir: {:?}", self.info_dir()))?;
        Ok(())
    }

    fn unique_name(&self, original_name: &str) -> String {
        let files_dir = self.files_dir();
        if !files_dir.join(original_name).exists() {
            return original_name.to_string();
        }

        // Handle name collisions by appending a counter
        let stem = Path::new(original_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(original_name);
        let ext = Path::new(original_name)
            .extension()
            .and_then(|s| s.to_str());

        for i in 1u64.. {
            let candidate = match ext {
                Some(e) => format!("{}.{}.{}", stem, i, e),
                None => format!("{}.{}", stem, i),
            };
            if !files_dir.join(&candidate).exists() {
                return candidate;
            }
        }
        unreachable!()
    }

    fn write_trashinfo(&self, trash_name: &str, original_path: &Path) -> Result<()> {
        let info_path = self.info_dir().join(format!("{}.trashinfo", trash_name));
        let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let content = format!(
            "[Trash Info]\nPath={}\nDeletionDate={}\n",
            original_path.display(),
            now,
        );
        fs::write(&info_path, content)
            .with_context(|| format!("failed to write trashinfo: {:?}", info_path))?;
        Ok(())
    }
}

impl TrashHandler for ManagedTrash {
    fn trash(&self, path: &Path) -> Result<()> {
        // Symlinks: remove directly to avoid canonicalize() resolving the target
        if path.is_symlink() {
            return std::fs::remove_file(path).with_context(|| {
                t!(
                    "error_trash_failed",
                    name = path.display().to_string(),
                    reason = "failed to remove symlink"
                )
            });
        }

        self.ensure_dirs()?;

        let canonical = path
            .canonicalize()
            .with_context(|| format!("failed to resolve path: {:?}", path))?;

        let original_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let trash_name = self.unique_name(original_name);
        let dest = self.files_dir().join(&trash_name);

        fs::rename(&canonical, &dest).with_context(|| {
            t!(
                "error_trash_failed",
                name = path.display().to_string(),
                reason = "rename failed"
            )
        })?;

        self.write_trashinfo(&trash_name, &canonical)?;
        Ok(())
    }

    fn cleanup(&self, prompter: &dyn Prompter) -> Result<()> {
        let files_dir = self.files_dir();
        if !files_dir.exists() {
            println!("{}", t!("cleanup_nothing"));
            return Ok(());
        }

        let entries: Vec<_> = fs::read_dir(&files_dir)
            .with_context(|| format!("failed to read trash dir: {:?}", files_dir))?
            .collect();

        if entries.is_empty() {
            println!("{}", t!("cleanup_nothing"));
            return Ok(());
        }

        if !prompter.confirm(&t!("confirm_cleanup_managed"))? {
            println!("{}", t!("cleanup_cancelled"));
            return Ok(());
        }

        // Remove all files
        for entry in fs::read_dir(&files_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                fs::remove_file(&path)?;
            }
        }

        // Remove all info files
        let info_dir = self.info_dir();
        if info_dir.exists() {
            for entry in fs::read_dir(&info_dir)? {
                let entry = entry?;
                fs::remove_file(entry.path())?;
            }
        }

        println!("{}", t!("cleanup_success"));
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "managed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::AutoConfirmPrompter;
    use tempfile::TempDir;

    fn setup() -> (TempDir, ManagedTrash) {
        let tmp = TempDir::new().unwrap();
        let handler = ManagedTrash::with_base_dir(tmp.path().to_path_buf());
        (tmp, handler)
    }

    #[test]
    fn test_trash_file() {
        let (tmp, handler) = setup();

        // Create a file to trash
        let source_dir = TempDir::new().unwrap();
        let file_path = source_dir.path().join("test.txt");
        fs::write(&file_path, "hello").unwrap();

        handler.trash(&file_path).unwrap();

        // Original should be gone
        assert!(!file_path.exists());

        // Should be in trash files dir
        let trashed = tmp.path().join("files").join("test.txt");
        assert!(trashed.exists());
        assert_eq!(fs::read_to_string(&trashed).unwrap(), "hello");

        // Should have trashinfo
        let info = tmp.path().join("info").join("test.txt.trashinfo");
        assert!(info.exists());
        let info_content = fs::read_to_string(&info).unwrap();
        assert!(info_content.contains("[Trash Info]"));
        assert!(info_content.contains("Path="));
        assert!(info_content.contains("DeletionDate="));
    }

    #[test]
    fn test_trash_name_collision() {
        let (tmp, handler) = setup();

        // Create and trash first file
        let source_dir = TempDir::new().unwrap();
        let file1 = source_dir.path().join("dup.txt");
        fs::write(&file1, "first").unwrap();
        handler.trash(&file1).unwrap();

        // Create and trash second file with same name
        let file2 = source_dir.path().join("dup.txt");
        fs::write(&file2, "second").unwrap();
        handler.trash(&file2).unwrap();

        // Both should exist in trash with different names
        let files_dir = tmp.path().join("files");
        assert!(files_dir.join("dup.txt").exists());
        assert!(files_dir.join("dup.1.txt").exists());
        assert_eq!(
            fs::read_to_string(files_dir.join("dup.txt")).unwrap(),
            "first"
        );
        assert_eq!(
            fs::read_to_string(files_dir.join("dup.1.txt")).unwrap(),
            "second"
        );
    }

    #[test]
    fn test_trash_directory() {
        let (_tmp, handler) = setup();

        let source_dir = TempDir::new().unwrap();
        let dir_path = source_dir.path().join("mydir");
        fs::create_dir(&dir_path).unwrap();
        fs::write(dir_path.join("inner.txt"), "inside").unwrap();

        handler.trash(&dir_path).unwrap();
        assert!(!dir_path.exists());
    }

    #[test]
    fn test_cleanup() {
        let (tmp, handler) = setup();

        // Create some files in trash
        handler.ensure_dirs().unwrap();
        fs::write(tmp.path().join("files").join("a.txt"), "a").unwrap();
        fs::write(tmp.path().join("files").join("b.txt"), "b").unwrap();
        fs::write(
            tmp.path().join("info").join("a.txt.trashinfo"),
            "[Trash Info]",
        )
        .unwrap();

        let prompter = AutoConfirmPrompter;
        handler.cleanup(&prompter).unwrap();

        // Files and info should be gone
        assert!(
            fs::read_dir(tmp.path().join("files"))
                .unwrap()
                .next()
                .is_none()
        );
        assert!(
            fs::read_dir(tmp.path().join("info"))
                .unwrap()
                .next()
                .is_none()
        );
    }

    #[test]
    fn test_cleanup_empty() {
        let (_tmp, handler) = setup();

        // Cleanup on empty trash should not error
        let prompter = AutoConfirmPrompter;
        handler.cleanup(&prompter).unwrap();
    }
}

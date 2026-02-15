use anyhow::{Context, Result};
use chrono::Local;
use rust_i18n::t;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use super::{RestorableItem, TrashHandler};
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
        // Allow overriding the trash base dir via env var (useful for testing)
        if let Ok(dir) = std::env::var("SAFERM_MANAGED_TRASH_DIR") {
            return Self {
                base_dir: PathBuf::from(dir),
            };
        }

        let data_dir = dirs::data_dir().or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".local/share"))
        });
        let base_dir = match data_dir {
            Some(dir) => dir,
            None => {
                eprintln!(
                    "saferm: warning: could not determine data directory, using /tmp/saferm/trash"
                );
                PathBuf::from("/tmp/saferm")
            }
        }
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

    fn list_restorable(&self, filter: Option<&str>) -> Result<Vec<RestorableItem>> {
        let info_dir = self.info_dir();
        if !info_dir.exists() {
            return Ok(vec![]);
        }

        let mut items = vec![];
        for entry in fs::read_dir(&info_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "trashinfo") {
                continue;
            }

            let trash_name = match path.file_stem().and_then(|s| s.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Verify the corresponding file still exists in files/
            if !self.files_dir().join(&trash_name).exists() {
                continue;
            }

            let content = fs::read_to_string(&path)?;
            let (original_path, deleted_at) = match parse_trashinfo(&content) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Apply filter
            if let Some(pat) = filter {
                let name_matches = trash_name.contains(pat);
                let path_matches = original_path.to_string_lossy().contains(pat);
                if !name_matches && !path_matches {
                    continue;
                }
            }

            let display_name = original_path
                .file_name()
                .unwrap_or(OsStr::new(&trash_name))
                .to_os_string();

            items.push(RestorableItem {
                id: OsString::from(&trash_name),
                original_path,
                display_name,
                deleted_at,
            });
        }

        Ok(items)
    }

    fn restore_to(&self, item_id: &OsStr, destination: &Path) -> Result<()> {
        let trash_name = item_id.to_string_lossy();
        let src = self.files_dir().join(trash_name.as_ref());

        if !src.exists() {
            anyhow::bail!(t!("restore_not_found"));
        }

        fs::rename(&src, destination).with_context(|| {
            t!(
                "error_restore_failed",
                name = trash_name,
                reason = "rename failed"
            )
        })?;

        // Clean up the .trashinfo file
        let info_path = self.info_dir().join(format!("{}.trashinfo", trash_name));
        let _ = fs::remove_file(info_path);

        Ok(())
    }
}

/// Parse a .trashinfo file and return (original_path, deleted_at_unix_seconds or None).
fn parse_trashinfo(content: &str) -> Result<(PathBuf, Option<i64>)> {
    let mut path: Option<PathBuf> = None;
    let mut date: Option<i64> = None;

    for line in content.lines() {
        if let Some(p) = line.strip_prefix("Path=") {
            path = Some(PathBuf::from(p));
        } else if let Some(d) = line.strip_prefix("DeletionDate=")
            && let Ok(dt) = chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%dT%H:%M:%S")
            && let chrono::LocalResult::Single(local_dt) = dt.and_local_timezone(Local)
        {
            date = Some(local_dt.timestamp());
        }
    }

    match path {
        Some(p) => Ok((p, date)),
        None => anyhow::bail!("invalid trashinfo: missing Path"),
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

    #[test]
    fn test_list_restorable() {
        let (_tmp, handler) = setup();

        // Trash some files
        let source_dir = TempDir::new().unwrap();
        let file1 = source_dir.path().join("alpha.txt");
        let file2 = source_dir.path().join("beta.txt");
        fs::write(&file1, "aaa").unwrap();
        fs::write(&file2, "bbb").unwrap();
        handler.trash(&file1).unwrap();
        handler.trash(&file2).unwrap();

        // List all
        let items = handler.list_restorable(None).unwrap();
        assert_eq!(items.len(), 2);

        // Filter by pattern
        let filtered = handler.list_restorable(Some("alpha")).unwrap();
        assert_eq!(filtered.len(), 1);
        assert!(
            filtered[0]
                .original_path
                .to_string_lossy()
                .contains("alpha.txt")
        );

        // Filter with no match
        let empty = handler.list_restorable(Some("nonexistent")).unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_list_restorable_empty_trash() {
        let (_tmp, handler) = setup();
        let items = handler.list_restorable(None).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_restore_to_file() {
        let (_tmp, handler) = setup();

        // Trash a file
        let source_dir = TempDir::new().unwrap();
        let file_path = source_dir.path().join("restore_me.txt");
        fs::write(&file_path, "important data").unwrap();
        handler.trash(&file_path).unwrap();
        assert!(!file_path.exists());

        // List and restore
        let items = handler.list_restorable(None).unwrap();
        assert_eq!(items.len(), 1);

        let dest = source_dir.path().join("restored.txt");
        handler.restore_to(&items[0].id, &dest).unwrap();

        assert!(dest.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "important data");

        // Trash should now be empty
        let after = handler.list_restorable(None).unwrap();
        assert!(after.is_empty());
    }

    #[test]
    fn test_restore_to_original_path() {
        let (_tmp, handler) = setup();

        // Trash a file
        let source_dir = TempDir::new().unwrap();
        let file_path = source_dir.path().join("original.txt");
        fs::write(&file_path, "original content").unwrap();
        handler.trash(&file_path).unwrap();
        assert!(!file_path.exists());

        // Restore to original path
        let items = handler.list_restorable(None).unwrap();
        handler
            .restore_to(&items[0].id, &items[0].original_path)
            .unwrap();

        assert!(file_path.exists());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "original content");
    }

    #[test]
    fn test_restore_to_missing_parent_dir() {
        let (_tmp, handler) = setup();

        // Trash a file
        let source_dir = TempDir::new().unwrap();
        let file_path = source_dir.path().join("test.txt");
        fs::write(&file_path, "data").unwrap();
        handler.trash(&file_path).unwrap();

        // Restore to a path with a non-existent parent directory
        // Note: parent dir creation is handled in ops.rs, not in the backend.
        // Backend only does the rename. Let's test the basic restore.
        let items = handler.list_restorable(None).unwrap();
        let new_dest = source_dir.path().join("test_restored.txt");
        handler.restore_to(&items[0].id, &new_dest).unwrap();
        assert!(new_dest.exists());
    }

    #[test]
    fn test_restore_not_found() {
        let (_tmp, handler) = setup();

        // Try to restore a non-existent item
        let result = handler.restore_to(
            std::ffi::OsStr::new("nonexistent"),
            Path::new("/tmp/dest.txt"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_restore_directory() {
        let (_tmp, handler) = setup();

        // Trash a directory
        let source_dir = TempDir::new().unwrap();
        let dir_path = source_dir.path().join("mydir");
        fs::create_dir(&dir_path).unwrap();
        fs::write(dir_path.join("inner.txt"), "inside").unwrap();
        handler.trash(&dir_path).unwrap();
        assert!(!dir_path.exists());

        // Restore it
        let items = handler.list_restorable(None).unwrap();
        assert_eq!(items.len(), 1);
        handler.restore_to(&items[0].id, &dir_path).unwrap();

        assert!(dir_path.exists());
        assert!(dir_path.join("inner.txt").exists());
        assert_eq!(
            fs::read_to_string(dir_path.join("inner.txt")).unwrap(),
            "inside"
        );
    }
}

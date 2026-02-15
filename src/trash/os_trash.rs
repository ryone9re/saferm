use anyhow::{Context, Result};
use rust_i18n::t;
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use super::{RestorableItem, TrashHandler};
use crate::prompt::Prompter;

pub struct OsTrash;

// macOS-specific helper methods for restore metadata tracking
#[cfg(target_os = "macos")]
impl OsTrash {
    /// Directory for saferm's restore metadata on macOS
    fn info_dir() -> PathBuf {
        let data_dir = dirs::data_dir().unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".local/share"))
                .unwrap_or_else(|| PathBuf::from("/tmp/saferm"))
        });
        data_dir.join("saferm").join("os-trash-info")
    }

    /// The macOS user trash directory
    fn trash_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".Trash"))
            .unwrap_or_else(|| PathBuf::from("/tmp/.Trash"))
    }

    fn ensure_info_dir() -> Result<()> {
        fs::create_dir_all(Self::info_dir())?;
        Ok(())
    }

    /// Snapshot the names of files in ~/.Trash/
    fn snapshot_trash() -> HashSet<OsString> {
        let trash_dir = Self::trash_dir();
        fs::read_dir(&trash_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect()
    }

    /// Write restore metadata after a successful trash operation
    fn write_restore_meta(trash_name: &OsStr, original_path: &Path) -> Result<()> {
        Self::ensure_info_dir()?;
        let id = uuid_v4();
        let info_path = Self::info_dir().join(format!("{}.trashinfo", id));
        let trash_path = Self::trash_dir().join(trash_name);
        let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let content = format!(
            "[Trash Info]\nPath={}\nTrashPath={}\nDeletionDate={}\n",
            original_path.display(),
            trash_path.display(),
            now,
        );
        fs::write(&info_path, content)?;
        Ok(())
    }

    /// Parse a macOS restore metadata file
    fn parse_restore_meta(content: &str) -> Option<(PathBuf, PathBuf, Option<i64>)> {
        let mut path: Option<PathBuf> = None;
        let mut trash_path: Option<PathBuf> = None;
        let mut date: Option<i64> = None;

        for line in content.lines() {
            if let Some(p) = line.strip_prefix("Path=") {
                path = Some(PathBuf::from(p));
            } else if let Some(tp) = line.strip_prefix("TrashPath=") {
                trash_path = Some(PathBuf::from(tp));
            } else if let Some(d) = line.strip_prefix("DeletionDate=")
                && let Ok(dt) = chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%dT%H:%M:%S")
                && let chrono::LocalResult::Single(local_dt) = dt.and_local_timezone(chrono::Local)
            {
                date = Some(local_dt.timestamp());
            }
        }

        match (path, trash_path) {
            (Some(p), Some(tp)) => Some((p, tp, date)),
            _ => None,
        }
    }
}

/// Simple UUID v4 generation without external crate
#[cfg(target_os = "macos")]
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    // Use timestamp + process id for uniqueness (sufficient for CLI tool)
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (now.as_secs() & 0xFFFF_FFFF) as u32,
        (now.subsec_nanos() >> 16) & 0xFFFF,
        now.subsec_nanos() & 0x0FFF,
        0x8000 | (std::process::id() & 0x3FFF),
        now.as_nanos() & 0xFFFF_FFFF_FFFF,
    )
}

impl TrashHandler for OsTrash {
    fn trash(&self, path: &Path) -> Result<()> {
        // Symlinks: remove directly since they are just pointers,
        // and the trash crate may fail for symlinks in certain directories.
        if path.is_symlink() {
            return std::fs::remove_file(path).with_context(|| {
                t!(
                    "error_trash_failed",
                    name = path.display().to_string(),
                    reason = "failed to remove symlink"
                )
            });
        }

        #[cfg(target_os = "macos")]
        {
            // Best-effort metadata tracking for restore on macOS
            let original_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

            let before = Self::snapshot_trash();

            trash::delete(path).with_context(|| {
                t!(
                    "error_trash_failed",
                    name = path.display().to_string(),
                    reason = "OS trash operation failed"
                )
            })?;

            let after = Self::snapshot_trash();
            let new_entries: Vec<_> = after.difference(&before).collect();

            // Only write metadata if we can confidently identify the new entry
            if new_entries.len() == 1 {
                let _ = Self::write_restore_meta(new_entries[0], &original_path);
            }

            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            trash::delete(path).with_context(|| {
                t!(
                    "error_trash_failed",
                    name = path.display().to_string(),
                    reason = "OS trash operation failed"
                )
            })
        }
    }

    fn cleanup(&self, _prompter: &dyn Prompter) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            eprintln!("{}", t!("cleanup_macos_hint"));
            Ok(())
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let items = trash::os_limited::list().with_context(|| {
                t!(
                    "error_cleanup_failed",
                    reason = "failed to list trash items"
                )
            })?;

            if items.is_empty() {
                println!("{}", t!("cleanup_nothing"));
                return Ok(());
            }

            if !_prompter.confirm(&t!("confirm_cleanup"))? {
                println!("{}", t!("cleanup_cancelled"));
                return Ok(());
            }

            trash::os_limited::purge_all(items)
                .with_context(|| t!("error_cleanup_failed", reason = "purge failed"))?;
            println!("{}", t!("cleanup_success"));
            Ok(())
        }
    }

    fn backend_name(&self) -> &'static str {
        "os"
    }

    fn list_restorable(&self, filter: Option<&str>) -> Result<Vec<RestorableItem>> {
        #[cfg(target_os = "macos")]
        {
            let info_dir = Self::info_dir();
            if !info_dir.exists() {
                return Ok(vec![]);
            }

            let mut items = vec![];
            let mut stale_files = vec![];

            for entry in fs::read_dir(&info_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_none_or(|e| e != "trashinfo") {
                    continue;
                }

                let id = match path.file_stem() {
                    Some(s) => s.to_os_string(),
                    None => continue,
                };

                let content = match fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let (original_path, trash_path, deleted_at) =
                    match Self::parse_restore_meta(&content) {
                        Some(v) => v,
                        None => continue,
                    };

                // Prune stale records: verify the trashed file still exists
                if !trash_path.exists() {
                    stale_files.push(path.clone());
                    continue;
                }

                // Apply filter
                if let Some(pat) = filter {
                    let name = original_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();
                    let path_str = original_path.to_string_lossy();
                    if !name.contains(pat) && !path_str.contains(pat) {
                        continue;
                    }
                }

                let display_name = original_path
                    .file_name()
                    .unwrap_or(OsStr::new("unknown"))
                    .to_os_string();

                items.push(RestorableItem {
                    id,
                    original_path,
                    display_name,
                    deleted_at,
                });
            }

            // Clean up stale metadata files
            for stale in stale_files {
                let _ = fs::remove_file(stale);
            }

            Ok(items)
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let os_items = trash::os_limited::list().with_context(|| {
                t!(
                    "error_restore_failed",
                    name = "trash",
                    reason = "failed to list trash items"
                )
            })?;

            let mut items = vec![];
            for item in os_items {
                let name_str = item.name.to_string_lossy();

                // Apply filter
                if let Some(pat) = filter {
                    let original_str = item.original_path().to_string_lossy().to_string();
                    if !name_str.contains(pat) && !original_str.contains(pat) {
                        continue;
                    }
                }

                items.push(RestorableItem {
                    id: item.id.clone(),
                    original_path: item.original_path(),
                    display_name: item.name.clone(),
                    deleted_at: Some(item.time_deleted),
                });
            }

            Ok(items)
        }
    }

    fn restore_to(&self, item_id: &OsStr, destination: &Path) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let info_dir = Self::info_dir();
            let info_path = info_dir.join(format!("{}.trashinfo", item_id.to_string_lossy()));

            let content =
                fs::read_to_string(&info_path).with_context(|| t!("restore_not_found"))?;

            let (_original_path, trash_path, _deleted_at) = Self::parse_restore_meta(&content)
                .ok_or_else(|| anyhow::anyhow!(t!("restore_not_found")))?;

            if !trash_path.exists() {
                anyhow::bail!(t!("restore_not_found"));
            }

            fs::rename(&trash_path, destination).with_context(|| {
                t!(
                    "error_restore_failed",
                    name = trash_path.display().to_string(),
                    reason = "rename failed"
                )
            })?;

            // Clean up metadata
            let _ = fs::remove_file(&info_path);

            Ok(())
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            // Find the TrashItem matching the given id
            let items = trash::os_limited::list().with_context(|| {
                t!(
                    "error_restore_failed",
                    name = "trash",
                    reason = "failed to list trash items"
                )
            })?;

            let to_restore: Vec<_> = items.into_iter().filter(|i| i.id == item_id).collect();

            if to_restore.is_empty() {
                anyhow::bail!(t!("restore_not_found"));
            }

            let original_path = to_restore[0].original_path();

            // If dest differs and the original path is occupied (rename/overwrite case),
            // temporarily move the occupying file so restore_all won't collide.
            let temp_evict: Option<PathBuf> =
                if destination != original_path && original_path.exists() {
                    let parent = original_path.parent().unwrap_or(Path::new("."));
                    let base_name = original_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();
                    let mut tmp = parent.join(format!(
                        ".saferm-evict-{}-{}",
                        std::process::id(),
                        base_name
                    ));
                    let mut counter = 0u64;
                    while tmp.exists() {
                        counter += 1;
                        tmp = parent.join(format!(
                            ".saferm-evict-{}-{}-{}",
                            std::process::id(),
                            counter,
                            base_name
                        ));
                    }
                    fs::rename(&original_path, &tmp)?;
                    Some(tmp)
                } else {
                    None
                };

            match trash::os_limited::restore_all(&to_restore) {
                Ok(()) => {
                    // If destination differs from original, move after native restore
                    if destination != original_path {
                        if let Err(e) = fs::rename(&original_path, destination) {
                            // Rename failed — rollback evicted file before returning error
                            if let Some(tmp) = &temp_evict {
                                if let Err(re) = fs::rename(tmp, &original_path) {
                                    eprintln!(
                                        "saferm: warning: rollback failed for '{}': {}",
                                        original_path.display(),
                                        re
                                    );
                                }
                            }
                            return Err(e.into());
                        }
                    }
                    // Put back the evicted file
                    if let Some(tmp) = temp_evict {
                        if let Err(e) = fs::rename(&tmp, &original_path) {
                            eprintln!(
                                "saferm: warning: failed to restore evicted file '{}': {}",
                                original_path.display(),
                                e
                            );
                        }
                    }
                    Ok(())
                }
                Err(e) => {
                    // Rollback: put back the evicted file
                    if let Some(tmp) = temp_evict {
                        if let Err(re) = fs::rename(&tmp, &original_path) {
                            eprintln!(
                                "saferm: warning: rollback failed for '{}': {}",
                                original_path.display(),
                                re
                            );
                        }
                    }
                    match e {
                        trash::Error::RestoreCollision { .. } => {
                            anyhow::bail!(t!(
                                "restore_conflict",
                                name = to_restore[0].name.to_string_lossy()
                            ))
                        }
                        other => Err(anyhow::anyhow!(other)),
                    }
                }
            }
        }
    }
}

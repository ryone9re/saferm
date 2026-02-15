pub mod managed;
pub mod os_trash;

use anyhow::Result;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// An item in the trash that can be restored.
pub struct RestorableItem {
    /// Backend-opaque stable key (e.g., trash filename for managed, OsString id for os_limited)
    pub id: OsString,
    /// The original path before the file was trashed
    pub original_path: PathBuf,
    /// Display name for UI (original filename)
    pub display_name: OsString,
    /// Deletion timestamp as unix seconds (None if unknown)
    pub deleted_at: Option<i64>,
}

pub trait TrashHandler {
    fn trash(&self, path: &Path) -> Result<()>;
    fn cleanup(&self, prompter: &dyn crate::prompt::Prompter) -> Result<()>;
    fn backend_name(&self) -> &'static str;

    /// List items in the trash that can be restored, optionally filtered by a substring pattern.
    fn list_restorable(&self, filter: Option<&str>) -> Result<Vec<RestorableItem>>;

    /// Restore a trashed item (identified by `item_id`) to the given `destination` path.
    fn restore_to(&self, item_id: &OsStr, destination: &Path) -> Result<()>;
}

pub fn create_handler() -> Box<dyn TrashHandler> {
    if let Ok(backend) = std::env::var("SAFERM_TRASH_BACKEND") {
        return match backend.as_str() {
            "os" => Box::new(os_trash::OsTrash),
            "managed" => Box::new(managed::ManagedTrash::new()),
            other => {
                eprintln!(
                    "saferm: unknown SAFERM_TRASH_BACKEND '{}', using default",
                    other
                );
                default_handler()
            }
        };
    }
    default_handler()
}

fn default_handler() -> Box<dyn TrashHandler> {
    if should_use_os_trash() {
        Box::new(os_trash::OsTrash)
    } else {
        Box::new(managed::ManagedTrash::new())
    }
}

fn should_use_os_trash() -> bool {
    if cfg!(target_os = "macos") {
        return true;
    }

    // On Linux, check for a desktop environment
    if cfg!(target_os = "linux") {
        let has_desktop = std::env::var("XDG_CURRENT_DESKTOP").is_ok()
            || std::env::var("DESKTOP_SESSION").is_ok();
        return has_desktop;
    }

    // Default to managed trash on unknown platforms
    false
}

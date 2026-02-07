pub mod managed;
pub mod os_trash;

use anyhow::Result;
use std::path::Path;

pub trait TrashHandler {
    fn trash(&self, path: &Path) -> Result<()>;
    fn cleanup(&self, prompter: &dyn crate::prompt::Prompter) -> Result<()>;
    fn backend_name(&self) -> &'static str;
}

pub fn create_handler() -> Box<dyn TrashHandler> {
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

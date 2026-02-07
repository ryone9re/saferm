use anyhow::{Context, Result};
use rust_i18n::t;
use std::path::Path;

use super::TrashHandler;
use crate::prompt::Prompter;

pub struct OsTrash;

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

        trash::delete(path).with_context(|| {
            t!(
                "error_trash_failed",
                name = path.display().to_string(),
                reason = "OS trash operation failed"
            )
        })
    }

    fn cleanup(&self, _prompter: &dyn Prompter) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            eprintln!("{}", t!("cleanup_macos_hint"));
            Ok(())
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let items: Vec<_> = trash::os_limited::list()
                .with_context(|| {
                    t!(
                        "error_cleanup_failed",
                        reason = "failed to list trash items"
                    )
                })?
                .collect();

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
}

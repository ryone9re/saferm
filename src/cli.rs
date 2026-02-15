use clap::Parser;
use std::path::PathBuf;

/// A safe rm replacement — moves files to trash instead of permanent deletion.
#[derive(Parser, Debug)]
#[command(name = "saferm", version, about)]
pub struct Cli {
    /// Files or directories to remove (or filter pattern when used with --restore)
    #[arg(required_unless_present_any = ["cleanup", "restore"])]
    pub targets: Vec<PathBuf>,

    /// Remove directories and their contents recursively
    #[arg(short = 'r', short_alias = 'R', long = "recursive")]
    pub recursive: bool,

    /// Ignore nonexistent files; skip confirmation in non-interactive mode
    #[arg(short, long)]
    pub force: bool,

    /// Prompt before every removal (default behavior).
    /// Accepted for rm-compatibility but has no additional effect —
    /// saferm always prompts in interactive (TTY) mode.
    #[arg(short, long)]
    pub interactive: bool,

    /// Remove empty directories
    #[arg(short, long = "dir")]
    pub dir: bool,

    /// Explain what is being done
    #[arg(short, long)]
    pub verbose: bool,

    /// Empty the trash
    #[arg(long, conflicts_with = "restore")]
    pub cleanup: bool,

    /// Restore files from the trash to their original location
    #[arg(long)]
    pub restore: bool,
}

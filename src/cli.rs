use clap::Parser;
use std::path::PathBuf;

/// A safe rm replacement — moves files to trash instead of permanent deletion.
#[derive(Parser, Debug)]
#[command(name = "saferm", version, about)]
pub struct Cli {
    /// Files or directories to remove
    #[arg(required_unless_present = "cleanup")]
    pub targets: Vec<PathBuf>,

    /// Remove directories and their contents recursively
    #[arg(short = 'r', short_alias = 'R', long = "recursive")]
    pub recursive: bool,

    /// Ignore nonexistent files and never prompt
    #[arg(short, long)]
    pub force: bool,

    /// Prompt before every removal (default behavior)
    #[arg(short, long)]
    pub interactive: bool,

    /// Remove empty directories
    #[arg(short, long = "dir")]
    pub dir: bool,

    /// Explain what is being done
    #[arg(short, long)]
    pub verbose: bool,

    /// Empty the trash
    #[arg(long)]
    pub cleanup: bool,
}

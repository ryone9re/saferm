use anyhow::Result;
use clap::Parser;

use saferm::cli::Cli;
use saferm::ops;
use saferm::prompt::InteractivePrompter;
use saferm::trash;

fn main() -> Result<()> {
    saferm::i18n::init();

    let cli = Cli::parse();
    let handler = trash::create_handler();
    let prompter = InteractivePrompter;

    let all_ok = ops::run(&cli, handler.as_ref(), &prompter)?;

    if !all_ok {
        std::process::exit(1);
    }

    Ok(())
}

use anyhow::Result;
use dialoguer::Confirm;

pub trait Prompter {
    fn confirm(&self, message: &str) -> Result<bool>;
}

pub struct InteractivePrompter;

impl Prompter for InteractivePrompter {
    fn confirm(&self, message: &str) -> Result<bool> {
        let result = Confirm::new()
            .with_prompt(message)
            .default(false)
            .interact()?;
        Ok(result)
    }
}

pub struct AutoConfirmPrompter;

impl Prompter for AutoConfirmPrompter {
    fn confirm(&self, _message: &str) -> Result<bool> {
        Ok(true)
    }
}

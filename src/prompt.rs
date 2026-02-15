use anyhow::Result;
use dialoguer::{Confirm, MultiSelect, Select};

pub trait Prompter {
    fn confirm(&self, message: &str) -> Result<bool>;
    fn select(&self, message: &str, options: &[String], default: usize) -> Result<usize>;
    fn multi_select(
        &self,
        message: &str,
        options: &[String],
        defaults: &[bool],
    ) -> Result<Vec<usize>>;
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

    fn select(&self, message: &str, options: &[String], default: usize) -> Result<usize> {
        let result = Select::new()
            .with_prompt(message)
            .items(options)
            .default(default)
            .interact()?;
        Ok(result)
    }

    fn multi_select(
        &self,
        message: &str,
        options: &[String],
        defaults: &[bool],
    ) -> Result<Vec<usize>> {
        let result = MultiSelect::new()
            .with_prompt(message)
            .items(options)
            .defaults(defaults)
            .interact()?;
        Ok(result)
    }
}

pub struct AutoConfirmPrompter;

impl Prompter for AutoConfirmPrompter {
    fn confirm(&self, _message: &str) -> Result<bool> {
        Ok(true)
    }

    fn select(&self, _message: &str, _options: &[String], default: usize) -> Result<usize> {
        Ok(default)
    }

    fn multi_select(
        &self,
        _message: &str,
        options: &[String],
        _defaults: &[bool],
    ) -> Result<Vec<usize>> {
        // Auto-select all items
        Ok((0..options.len()).collect())
    }
}

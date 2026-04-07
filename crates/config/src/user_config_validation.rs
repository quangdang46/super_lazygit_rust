//! User configuration validation logic.

use crate::{AppConfig, ConfigValidationError};

impl AppConfig {
    /// Validates the configuration and returns an error if validation fails.
    pub fn validate_user_config(&self) -> Result<(), ConfigValidationError> {
        validate_enum(
            "gui.statusPanelView",
            &self.gui.status_panel_view,
            &["dashboard", "allBranchesLog"],
        )?;

        validate_enum(
            "git.autoForwardBranches",
            &self.git.auto_forward_branches,
            &["none", "onlyMainBranches", "allBranches"],
        )?;

        // Validate keybindings recursively
        self.keybindings.validate()?;

        // Validate custom commands
        validate_custom_commands(&self.custom_commands)?;

        Ok(())
    }
}

fn validate_enum(
    name: &str,
    value: &str,
    allowed_values: &[&str],
) -> Result<(), ConfigValidationError> {
    if allowed_values.contains(&value) {
        Ok(())
    } else {
        Err(ConfigValidationError::new(format!(
            "Unexpected value '{}' for '{}'. Allowed values: {}",
            value,
            name,
            allowed_values.join(", ")
        )))
    }
}

fn validate_custom_commands(custom_commands: &[crate::CustomCommand]) -> Result<(), ConfigValidationError> {
    for custom_command in custom_commands {
        validate_custom_command_key(&custom_command.key)?;

        if !custom_command.command_menu.is_empty() {
            // Validate that other fields are not set when using commandMenu
            if !custom_command.context.is_empty()
                || !custom_command.command.is_empty()
                || !custom_command.prompts.is_empty()
                || !custom_command.loading_text.is_empty()
                || !custom_command.output.is_empty()
                || !custom_command.output_title.is_empty()
                || custom_command.after.is_some()
            {
                let command_ref = if !custom_command.key.is_empty() {
                    format!(" with key '{}'", custom_command.key)
                } else {
                    String::new()
                };
                return Err(ConfigValidationError::new(format!(
                    "Error with custom command{}: it is not allowed to use both commandMenu and any of the other fields except key and description.",
                    command_ref
                )));
            }

            // Recursively validate command menu
            validate_custom_commands(&custom_command.command_menu)?;
        } else {
            // Validate prompts
            for prompt in &custom_command.prompts {
                validate_custom_command_prompt(prompt)?;
            }

            // Validate output field
            validate_enum(
                "customCommand.output",
                &custom_command.output,
                &["", "none", "terminal", "log", "logWithPty", "popup"],
            )?;
        }
    }
    Ok(())
}

const CUSTOM_KEYBINDINGS_DOCS_URL: &str =
    "https://github.com/jesseduffield/lazygit/blob/master/docs/keybindings/Custom_Keybindings.md";

fn validate_custom_command_key(key: &str) -> Result<(), ConfigValidationError> {
    if crate::is_valid_keybinding_key(key) {
        Ok(())
    } else {
        Err(ConfigValidationError::new(format!(
            "Unrecognized key '{}' for custom command. For permitted values see {}",
            key,
            CUSTOM_KEYBINDINGS_DOCS_URL
        )))
    }
}

fn validate_custom_command_prompt(
    prompt: &crate::CustomCommandPrompt,
) -> Result<(), ConfigValidationError> {
    for option in &prompt.options {
        if !crate::is_valid_keybinding_key(&option.key) {
            return Err(ConfigValidationError::new(format!(
                "Unrecognized key '{}' for custom command prompt option. For permitted values see {}",
                option.key,
                CUSTOM_KEYBINDINGS_DOCS_URL
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_enum_success() {
        assert!(validate_enum("test", "value", &["value", "other"]).is_ok());
    }

    #[test]
    fn test_validate_enum_failure() {
        let result = validate_enum("test", "invalid", &["value", "other"]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unexpected value 'invalid'"));
    }

    #[test]
    fn test_app_config_validate_default() {
        let config = AppConfig::default();
        assert!(config.validate_user_config().is_ok());
    }
}

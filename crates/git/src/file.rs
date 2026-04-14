use std::path::{Path, PathBuf};

use super_lazygit_config::EditorConfig;

pub struct FileCommands;

impl FileCommands {
    pub fn new() -> Self {
        Self
    }

    pub fn cat(&self, path: &Path) -> std::io::Result<String> {
        std::fs::read_to_string(path)
    }

    pub fn get_edit_cmd_str(
        &self,
        editor_config: &EditorConfig,
        shell: &str,
        filenames: &[PathBuf],
        guess_default_editor: impl FnOnce() -> String,
    ) -> (String, bool) {
        let quoted_filenames: Vec<String> = filenames
            .iter()
            .map(|f| shell_quote(&f.to_string_lossy()))
            .collect();

        let template_values: std::collections::HashMap<&str, String> =
            std::collections::HashMap::from([("filename", quoted_filenames.join(" "))]);

        let resolved =
            editor_config
                .resolve_edit_command(shell, Path::new("<placeholder>"), || guess_default_editor());
        let cmd_str = resolve_placeholder_in_template(&resolved.command, &template_values);
        (cmd_str, resolved.suspend)
    }

    pub fn get_edit_at_line_cmd_str(
        &self,
        editor_config: &EditorConfig,
        shell: &str,
        filename: &Path,
        line_number: usize,
        guess_default_editor: impl FnOnce() -> String,
    ) -> (String, bool) {
        let resolved =
            editor_config.resolve_edit_at_line_command(shell, filename, line_number, || {
                guess_default_editor()
            });
        (resolved.command, resolved.suspend)
    }

    pub fn get_edit_at_line_and_wait_cmd_str(
        &self,
        editor_config: &EditorConfig,
        shell: &str,
        filename: &Path,
        line_number: usize,
        guess_default_editor: impl FnOnce() -> String,
    ) -> String {
        editor_config
            .resolve_edit_at_line_and_wait_command(shell, filename, line_number, || {
                guess_default_editor()
            })
            .command
    }

    pub fn get_open_dir_in_editor_cmd_str(
        &self,
        editor_config: &EditorConfig,
        shell: &str,
        path: &Path,
        guess_default_editor: impl FnOnce() -> String,
    ) -> (String, bool) {
        let resolved =
            editor_config.resolve_open_dir_command(shell, path, || guess_default_editor());
        (resolved.command, resolved.suspend)
    }

    pub fn guess_default_editor(&self) -> String {
        guess_default_editor_from_env()
    }
}

impl Default for FileCommands {
    fn default() -> Self {
        Self::new()
    }
}

fn guess_default_editor_from_env() -> String {
    let editor = std::env::var("GIT_EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_default();

    editor
        .trim()
        .split(' ')
        .next()
        .unwrap_or_default()
        .to_string()
}

fn shell_quote(value: &str) -> String {
    #[cfg(windows)]
    {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
    #[cfg(not(windows))]
    {
        if value.is_empty() {
            "''".to_string()
        } else {
            format!("'{}'", value.replace('\'', r#"'"'"'"#))
        }
    }
}

fn resolve_placeholder_in_template(
    template: &str,
    values: &std::collections::HashMap<&str, String>,
) -> String {
    let mut result = template.to_string();
    for (key, value) in values {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }
    result
}

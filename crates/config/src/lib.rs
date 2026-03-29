use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub workspace: WorkspaceConfig,
    pub editor: EditorConfig,
    pub theme: ThemeConfig,
    pub keybindings: KeybindingConfig,
    pub diagnostics: DiagnosticsConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceConfig {
    pub roots: Vec<PathBuf>,
    pub ignores: Vec<String>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            ignores: default_workspace_ignores(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorConfig {
    pub command: String,
    pub args: Vec<String>,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            command: String::from("vim"),
            args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub preset: ThemePreset,
    pub colors: ThemeColors,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreset {
    #[default]
    DefaultDark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeColors {
    pub background: String,
    pub foreground: String,
    pub accent: String,
    pub success: String,
    pub warning: String,
    pub danger: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            background: String::from("#111318"),
            foreground: String::from("#d8dee9"),
            accent: String::from("#88c0d0"),
            success: String::from("#a3be8c"),
            warning: String::from("#ebcb8b"),
            danger: String::from("#bf616a"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindingConfig {
    pub overrides: Vec<KeybindingOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeybindingOverride {
    pub action: String,
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiagnosticsConfig {
    pub enabled: bool,
    pub log_samples: bool,
    pub slow_render_threshold_ms: u64,
    pub watcher_burst_threshold: usize,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_samples: true,
            slow_render_threshold_ms: 16,
            watcher_burst_threshold: 8,
        }
    }
}

#[must_use]
pub fn default_workspace_ignores() -> Vec<String> {
    vec![
        String::from(".git"),
        String::from("node_modules"),
        String::from("target"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_exposes_foundation_surfaces() {
        let config = AppConfig::default();

        assert!(config.workspace.roots.is_empty());
        assert_eq!(config.workspace.ignores, default_workspace_ignores());
        assert_eq!(config.editor.command, "vim");
        assert!(config.editor.args.is_empty());
        assert_eq!(config.theme.preset, ThemePreset::DefaultDark);
        assert!(config.keybindings.overrides.is_empty());
        assert!(config.diagnostics.enabled);
    }
}

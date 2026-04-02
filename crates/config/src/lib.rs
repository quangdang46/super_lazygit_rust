use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod legacy_yaml_migration;

pub use legacy_yaml_migration::{compute_migrated_config, ChangesSet};

const CONFIG_DIR_NAME: &str = "super-lazygit";
const LEGACY_CONFIG_DIR_NAME: &str = "lazygit";
const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub workspace: WorkspaceConfig,
    pub os: OsConfig,
    pub editor: EditorConfig,
    pub theme: ThemeConfig,
    pub keybindings: KeybindingConfig,
    pub diagnostics: DiagnosticsConfig,
    pub services: BTreeMap<String, String>,
}

impl AppConfig {
    pub fn load() -> Result<LoadedConfig, ConfigLoadError> {
        Self::load_with_discovery(ConfigDiscovery::from_env())
    }

    pub fn load_with_discovery(
        discovery: ConfigDiscovery,
    ) -> Result<LoadedConfig, ConfigLoadError> {
        if let Some(path) = discovery.explicit_path {
            return Self::load_from_path(path, true);
        }

        for path in discovery.candidates {
            if path.is_file() {
                return Self::load_from_path(path, false);
            }
        }

        Ok(LoadedConfig {
            config: Self::default(),
            source: ConfigSource::Defaults,
        })
    }

    fn load_from_path(path: PathBuf, explicit: bool) -> Result<LoadedConfig, ConfigLoadError> {
        if explicit && !path.is_file() {
            return Err(ConfigLoadError::MissingExplicitPath { path });
        }

        let contents = fs::read_to_string(&path).map_err(|source| ConfigLoadError::Read {
            path: path.clone(),
            source,
        })?;
        let config = toml::from_str(&contents).map_err(|source| ConfigLoadError::Parse {
            path: path.clone(),
            source: Box::new(source),
        })?;

        Ok(LoadedConfig {
            config,
            source: ConfigSource::File(path),
        })
    }
}

pub fn default_config_toml() -> Result<String, toml::ser::Error> {
    toml::to_string_pretty(&AppConfig::default())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    pub config: AppConfig,
    pub source: ConfigSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSource {
    Defaults,
    File(PathBuf),
}

impl ConfigSource {
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Defaults => None,
            Self::File(path) => Some(path.as_path()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDiscovery {
    pub explicit_path: Option<PathBuf>,
    pub config_dir: Option<PathBuf>,
    pub candidates: Vec<PathBuf>,
}

impl ConfigDiscovery {
    #[must_use]
    pub fn from_env() -> Self {
        Self::with_overrides(
            env::var_os("CONFIG_DIR")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            env::var_os("SUPER_LAZYGIT_CONFIG")
                .or_else(|| env::var_os("LG_CONFIG_FILE"))
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            env::var_os("XDG_CONFIG_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
        )
    }

    #[must_use]
    pub fn from_overrides(explicit_dir: Option<PathBuf>, explicit_path: Option<PathBuf>) -> Self {
        Self::with_overrides(
            explicit_dir,
            explicit_path.or_else(|| {
                env::var_os("SUPER_LAZYGIT_CONFIG")
                    .or_else(|| env::var_os("LG_CONFIG_FILE"))
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
            }),
            env::var_os("XDG_CONFIG_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
        )
    }

    #[must_use]
    pub fn new(
        explicit_path: Option<PathBuf>,
        xdg_config_home: Option<PathBuf>,
        home_dir: Option<PathBuf>,
    ) -> Self {
        Self::with_overrides(None, explicit_path, xdg_config_home, home_dir)
    }

    #[must_use]
    pub fn with_overrides(
        explicit_dir: Option<PathBuf>,
        explicit_path: Option<PathBuf>,
        xdg_config_home: Option<PathBuf>,
        home_dir: Option<PathBuf>,
    ) -> Self {
        let default_dirs = default_config_dirs(xdg_config_home, home_dir);
        let default_dir = default_dirs.first().cloned();
        let has_explicit_dir = explicit_dir.is_some();
        let config_dir = explicit_path
            .as_deref()
            .map(config_dir_for_path)
            .or(explicit_dir)
            .or_else(|| default_dir.clone());
        let mut candidates = Vec::new();

        if explicit_path.is_none() {
            if let Some(config_dir) = config_dir.as_ref() {
                push_unique(&mut candidates, config_file_path(config_dir));
            }

            if !has_explicit_dir {
                for default_dir in default_dirs {
                    push_unique(&mut candidates, config_file_path(&default_dir));
                }
            }
        }

        Self {
            explicit_path,
            config_dir,
            candidates,
        }
    }

    #[must_use]
    pub fn config_dir(&self) -> Option<&Path> {
        self.config_dir.as_deref()
    }

    #[must_use]
    pub fn primary_config_path(&self) -> Option<&Path> {
        self.explicit_path
            .as_deref()
            .or_else(|| self.candidates.first().map(PathBuf::as_path))
    }
}

#[derive(Debug, Error)]
pub enum ConfigLoadError {
    #[error("config file not found: {path}")]
    MissingExplicitPath { path: PathBuf },
    #[error("failed to read config file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OsConfig {
    #[serde(alias = "open", skip_serializing_if = "String::is_empty")]
    pub open: String,
    #[serde(alias = "openLink", skip_serializing_if = "String::is_empty")]
    pub open_link: String,
    #[serde(alias = "copyToClipboardCmd", skip_serializing_if = "String::is_empty")]
    pub copy_to_clipboard_cmd: String,
    #[serde(
        alias = "readFromClipboardCmd",
        skip_serializing_if = "String::is_empty"
    )]
    pub read_from_clipboard_cmd: String,
    #[serde(alias = "shellFunctionsFile", skip_serializing_if = "String::is_empty")]
    pub shell_functions_file: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorConfig {
    pub command: String,
    pub args: Vec<String>,
    #[serde(alias = "editPreset", skip_serializing_if = "String::is_empty")]
    pub edit_preset: String,
    #[serde(alias = "edit", skip_serializing_if = "String::is_empty")]
    pub edit: String,
    #[serde(alias = "editAtLine", skip_serializing_if = "String::is_empty")]
    pub edit_at_line: String,
    #[serde(alias = "editAtLineAndWait", skip_serializing_if = "String::is_empty")]
    pub edit_at_line_and_wait: String,
    #[serde(alias = "openDirInEditor", skip_serializing_if = "String::is_empty")]
    pub open_dir_in_editor: String,
    #[serde(alias = "editInTerminal", skip_serializing_if = "Option::is_none")]
    pub edit_in_terminal: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEditorCommand {
    pub command: String,
    pub suspend: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorTemplateKind {
    Edit,
    EditAtLine,
    EditAtLineAndWait,
    OpenDirInEditor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditorPreset {
    edit_template: String,
    edit_at_line_template: String,
    edit_at_line_and_wait_template: String,
    open_dir_in_editor_template: String,
    suspend: bool,
}

impl EditorConfig {
    #[must_use]
    pub fn resolve_edit_command(
        &self,
        shell: &str,
        filename: &Path,
        guess_default_editor: impl FnOnce() -> String,
    ) -> ResolvedEditorCommand {
        self.resolve_command(
            EditorTemplateKind::Edit,
            shell,
            filename,
            None,
            guess_default_editor,
        )
    }

    #[must_use]
    pub fn resolve_edit_at_line_command(
        &self,
        shell: &str,
        filename: &Path,
        line: usize,
        guess_default_editor: impl FnOnce() -> String,
    ) -> ResolvedEditorCommand {
        self.resolve_command(
            EditorTemplateKind::EditAtLine,
            shell,
            filename,
            Some(line),
            guess_default_editor,
        )
    }

    #[must_use]
    pub fn resolve_edit_at_line_and_wait_command(
        &self,
        shell: &str,
        filename: &Path,
        line: usize,
        guess_default_editor: impl FnOnce() -> String,
    ) -> ResolvedEditorCommand {
        self.resolve_command(
            EditorTemplateKind::EditAtLineAndWait,
            shell,
            filename,
            Some(line),
            guess_default_editor,
        )
    }

    #[must_use]
    pub fn resolve_open_dir_command(
        &self,
        shell: &str,
        dir: &Path,
        guess_default_editor: impl FnOnce() -> String,
    ) -> ResolvedEditorCommand {
        self.resolve_command(
            EditorTemplateKind::OpenDirInEditor,
            shell,
            dir,
            None,
            guess_default_editor,
        )
    }

    fn resolve_command(
        &self,
        kind: EditorTemplateKind,
        shell: &str,
        target: &Path,
        line: Option<usize>,
        guess_default_editor: impl FnOnce() -> String,
    ) -> ResolvedEditorCommand {
        let guessed_editor = normalize_editor_name(&guess_default_editor());
        let preset = self.editor_preset(shell, &guessed_editor);
        let template = self.template_for_kind(kind, &preset);
        let command = resolve_placeholder_string(
            &template,
            target,
            line,
            matches!(kind, EditorTemplateKind::OpenDirInEditor),
        );

        ResolvedEditorCommand {
            command,
            suspend: self.edit_in_terminal.unwrap_or(preset.suspend),
        }
    }

    fn template_for_kind(&self, kind: EditorTemplateKind, preset: &EditorPreset) -> String {
        let configured = match kind {
            EditorTemplateKind::Edit => &self.edit,
            EditorTemplateKind::EditAtLine => &self.edit_at_line,
            EditorTemplateKind::EditAtLineAndWait => &self.edit_at_line_and_wait,
            EditorTemplateKind::OpenDirInEditor => &self.open_dir_in_editor,
        };
        if !configured.is_empty() {
            return configured.clone();
        }

        if let Some(legacy) = self.legacy_template(kind) {
            return legacy;
        }

        match kind {
            EditorTemplateKind::Edit => preset.edit_template.clone(),
            EditorTemplateKind::EditAtLine => preset.edit_at_line_template.clone(),
            EditorTemplateKind::EditAtLineAndWait => preset.edit_at_line_and_wait_template.clone(),
            EditorTemplateKind::OpenDirInEditor => preset.open_dir_in_editor_template.clone(),
        }
    }

    fn legacy_template(&self, kind: EditorTemplateKind) -> Option<String> {
        if self.command.trim().is_empty() && self.args.is_empty() {
            return None;
        }

        let mut parts = Vec::with_capacity(self.args.len() + 2);
        if !self.command.trim().is_empty() {
            parts.push(shell_quote(self.command.trim()));
        }
        parts.extend(self.args.iter().map(|arg| shell_quote(arg)));
        parts.push(
            match kind {
                EditorTemplateKind::OpenDirInEditor => "{{dir}}",
                EditorTemplateKind::Edit
                | EditorTemplateKind::EditAtLine
                | EditorTemplateKind::EditAtLineAndWait => "{{filename}}",
            }
            .to_string(),
        );
        Some(parts.join(" "))
    }

    fn editor_preset(&self, shell: &str, guessed_editor: &str) -> EditorPreset {
        let preset_name = if self.edit_preset.is_empty() {
            editor_name_to_preset(guessed_editor).unwrap_or("vim")
        } else {
            self.edit_preset.as_str()
        };

        match preset_name {
            "vi" => standard_terminal_editor_preset("vi"),
            "vim" => standard_terminal_editor_preset("vim"),
            "nvim" => standard_terminal_editor_preset("nvim"),
            "nvim-remote" => nvim_remote_preset(shell),
            "lvim" => standard_terminal_editor_preset("lvim"),
            "emacs" => standard_terminal_editor_preset("emacs"),
            "micro" => EditorPreset {
                edit_template: "micro {{filename}}".to_string(),
                edit_at_line_template: "micro +{{line}} {{filename}}".to_string(),
                edit_at_line_and_wait_template: "micro +{{line}} {{filename}}".to_string(),
                open_dir_in_editor_template: "micro {{dir}}".to_string(),
                suspend: true,
            },
            "nano" => standard_terminal_editor_preset("nano"),
            "kakoune" => standard_terminal_editor_preset("kak"),
            "helix" => EditorPreset {
                edit_template: "helix -- {{filename}}".to_string(),
                edit_at_line_template: "helix -- {{filename}}:{{line}}".to_string(),
                edit_at_line_and_wait_template: "helix -- {{filename}}:{{line}}".to_string(),
                open_dir_in_editor_template: "helix -- {{dir}}".to_string(),
                suspend: true,
            },
            "helix (hx)" => EditorPreset {
                edit_template: "hx -- {{filename}}".to_string(),
                edit_at_line_template: "hx -- {{filename}}:{{line}}".to_string(),
                edit_at_line_and_wait_template: "hx -- {{filename}}:{{line}}".to_string(),
                open_dir_in_editor_template: "hx -- {{dir}}".to_string(),
                suspend: true,
            },
            "vscode" => EditorPreset {
                edit_template: "code --reuse-window -- {{filename}}".to_string(),
                edit_at_line_template: "code --reuse-window --goto -- {{filename}}:{{line}}"
                    .to_string(),
                edit_at_line_and_wait_template:
                    "code --reuse-window --goto --wait -- {{filename}}:{{line}}".to_string(),
                open_dir_in_editor_template: "code -- {{dir}}".to_string(),
                suspend: false,
            },
            "sublime" => EditorPreset {
                edit_template: "subl -- {{filename}}".to_string(),
                edit_at_line_template: "subl -- {{filename}}:{{line}}".to_string(),
                edit_at_line_and_wait_template: "subl --wait -- {{filename}}:{{line}}".to_string(),
                open_dir_in_editor_template: "subl -- {{dir}}".to_string(),
                suspend: false,
            },
            "bbedit" => EditorPreset {
                edit_template: "bbedit -- {{filename}}".to_string(),
                edit_at_line_template: "bbedit +{{line}} -- {{filename}}".to_string(),
                edit_at_line_and_wait_template: "bbedit +{{line}} --wait -- {{filename}}"
                    .to_string(),
                open_dir_in_editor_template: "bbedit -- {{dir}}".to_string(),
                suspend: false,
            },
            "xcode" => EditorPreset {
                edit_template: "xed -- {{filename}}".to_string(),
                edit_at_line_template: "xed --line {{line}} -- {{filename}}".to_string(),
                edit_at_line_and_wait_template: "xed --line {{line}} --wait -- {{filename}}"
                    .to_string(),
                open_dir_in_editor_template: "xed -- {{dir}}".to_string(),
                suspend: false,
            },
            "zed" => EditorPreset {
                edit_template: "zed -- {{filename}}".to_string(),
                edit_at_line_template: "zed -- {{filename}}:{{line}}".to_string(),
                edit_at_line_and_wait_template: "zed --wait -- {{filename}}:{{line}}".to_string(),
                open_dir_in_editor_template: "zed -- {{dir}}".to_string(),
                suspend: false,
            },
            "acme" => EditorPreset {
                edit_template: "B {{filename}}".to_string(),
                edit_at_line_template: "B {{filename}}:{{line}}".to_string(),
                edit_at_line_and_wait_template: "E {{filename}}:{{line}}".to_string(),
                open_dir_in_editor_template: "B {{dir}}".to_string(),
                suspend: false,
            },
            _ => standard_terminal_editor_preset("vim"),
        }
    }
}

fn standard_terminal_editor_preset(editor: &str) -> EditorPreset {
    EditorPreset {
        edit_template: format!("{editor} -- {{{{filename}}}}"),
        edit_at_line_template: format!("{editor} +{{{{line}}}} -- {{{{filename}}}}"),
        edit_at_line_and_wait_template: format!("{editor} +{{{{line}}}} -- {{{{filename}}}}"),
        open_dir_in_editor_template: format!("{editor} -- {{{{dir}}}}"),
        suspend: true,
    }
}

fn nvim_remote_preset(shell: &str) -> EditorPreset {
    let (edit_template, edit_at_line_template, open_dir_in_editor_template) = if shell
        .ends_with("fish")
        || env::var_os("FISH_VERSION").is_some()
    {
        (
            r#"begin; if test -z "$NVIM"; nvim -- {{filename}}; else; nvim --server "$NVIM" --remote-send "q"; nvim --server "$NVIM" --remote-tab {{filename}}; end; end"#,
            r#"begin; if test -z "$NVIM"; nvim +{{line}} -- {{filename}}; else; nvim --server "$NVIM" --remote-send "q"; nvim --server "$NVIM" --remote-tab {{filename}}; nvim --server "$NVIM" --remote-send ":{{line}}<CR>"; end; end"#,
            r#"begin; if test -z "$NVIM"; nvim -- {{dir}}; else; nvim --server "$NVIM" --remote-send "q"; nvim --server "$NVIM" --remote-tab {{dir}}; end; end"#,
        )
    } else if shell.ends_with("nu")
        || shell.ends_with("nushell")
        || env::var_os("NU_VERSION").is_some()
    {
        (
            r#"if ($env | get -i NVIM | is-empty) { nvim -- {{filename}} } else { nvim --server $env.NVIM --remote-send "q"; nvim --server $env.NVIM --remote-tab {{filename}} }"#,
            r#"if ($env | get -i NVIM | is-empty) { nvim +{{line}} -- {{filename}} } else { nvim --server $env.NVIM --remote-send "q"; nvim --server $env.NVIM --remote-tab {{filename}}; nvim --server $env.NVIM --remote-send ":{{line}}<CR>" }"#,
            r#"if ($env | get -i NVIM | is-empty) { nvim -- {{dir}} } else { nvim --server $env.NVIM --remote-send "q"; nvim --server $env.NVIM --remote-tab {{dir}} }"#,
        )
    } else {
        (
            r#"[ -z "$NVIM" ] && (nvim -- {{filename}}) || (nvim --server "$NVIM" --remote-send "q" && nvim --server "$NVIM" --remote-tab {{filename}})"#,
            r#"[ -z "$NVIM" ] && (nvim +{{line}} -- {{filename}}) || (nvim --server "$NVIM" --remote-send "q" &&  nvim --server "$NVIM" --remote-tab {{filename}} && nvim --server "$NVIM" --remote-send ":{{line}}<CR>")"#,
            r#"[ -z "$NVIM" ] && (nvim -- {{dir}}) || (nvim --server "$NVIM" --remote-send "q" && nvim --server "$NVIM" --remote-tab {{dir}})"#,
        )
    };

    EditorPreset {
        edit_template: edit_template.to_string(),
        edit_at_line_template: edit_at_line_template.to_string(),
        edit_at_line_and_wait_template: "nvim +{{line}} {{filename}}".to_string(),
        open_dir_in_editor_template: open_dir_in_editor_template.to_string(),
        suspend: env::var_os("NVIM").is_none(),
    }
}

fn editor_name_to_preset(editor: &str) -> Option<&'static str> {
    match editor {
        "vi" => Some("vi"),
        "vim" => Some("vim"),
        "nvim" => Some("nvim"),
        "nvim-remote" => Some("nvim-remote"),
        "lvim" => Some("lvim"),
        "emacs" => Some("emacs"),
        "micro" => Some("micro"),
        "nano" => Some("nano"),
        "kak" => Some("kakoune"),
        "kakoune" => Some("kakoune"),
        "helix" => Some("helix"),
        "hx" => Some("helix (hx)"),
        "code" => Some("vscode"),
        "vscode" => Some("vscode"),
        "subl" => Some("sublime"),
        "sublime" => Some("sublime"),
        "bbedit" => Some("bbedit"),
        "xed" => Some("xcode"),
        "xcode" => Some("xcode"),
        "zed" => Some("zed"),
        "acme" => Some("acme"),
        _ => None,
    }
}

fn normalize_editor_name(editor: &str) -> String {
    editor
        .trim()
        .split(' ')
        .next()
        .unwrap_or_default()
        .to_string()
}

fn resolve_placeholder_string(
    template: &str,
    target: &Path,
    line: Option<usize>,
    _is_dir: bool,
) -> String {
    let mut resolved = template.to_string();
    let quoted_target = shell_quote(&target.display().to_string());
    let line = line.map(|value| value.to_string()).unwrap_or_default();

    for (placeholder, value) in [
        ("{{filename}}", quoted_target.as_str()),
        ("{{.filename}}", quoted_target.as_str()),
        ("{{dir}}", quoted_target.as_str()),
        ("{{.dir}}", quoted_target.as_str()),
        ("{{line}}", line.as_str()),
        ("{{.line}}", line.as_str()),
    ] {
        resolved = resolved.replace(placeholder, value);
    }

    resolved
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub preset: ThemePreset,
    pub colors: ThemeColors,
    #[serde(alias = "activeBorderColor")]
    pub active_border_color: Vec<String>,
    #[serde(alias = "inactiveBorderColor")]
    pub inactive_border_color: Vec<String>,
    #[serde(alias = "searchingActiveBorderColor")]
    pub searching_active_border_color: Vec<String>,
    #[serde(alias = "optionsTextColor")]
    pub options_text_color: Vec<String>,
    #[serde(alias = "selectedLineBgColor")]
    pub selected_line_bg_color: Vec<String>,
    #[serde(alias = "inactiveViewSelectedLineBgColor")]
    pub inactive_view_selected_line_bg_color: Vec<String>,
    #[serde(alias = "cherryPickedCommitFgColor")]
    pub cherry_picked_commit_fg_color: Vec<String>,
    #[serde(alias = "cherryPickedCommitBgColor")]
    pub cherry_picked_commit_bg_color: Vec<String>,
    #[serde(alias = "markedBaseCommitFgColor")]
    pub marked_base_commit_fg_color: Vec<String>,
    #[serde(alias = "markedBaseCommitBgColor")]
    pub marked_base_commit_bg_color: Vec<String>,
    #[serde(alias = "unstagedChangesColor")]
    pub unstaged_changes_color: Vec<String>,
    #[serde(alias = "defaultFgColor")]
    pub default_fg_color: Vec<String>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            preset: ThemePreset::DefaultDark,
            colors: ThemeColors::default(),
            active_border_color: vec![String::from("green"), String::from("bold")],
            inactive_border_color: vec![String::from("default")],
            searching_active_border_color: vec![String::from("cyan"), String::from("bold")],
            options_text_color: vec![String::from("blue")],
            selected_line_bg_color: vec![String::from("blue")],
            inactive_view_selected_line_bg_color: vec![String::from("bold")],
            cherry_picked_commit_fg_color: vec![String::from("blue")],
            cherry_picked_commit_bg_color: vec![String::from("cyan")],
            marked_base_commit_fg_color: vec![String::from("blue")],
            marked_base_commit_bg_color: vec![String::from("yellow")],
            unstaged_changes_color: vec![String::from("red")],
            default_fg_color: vec![String::from("default")],
        }
    }
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

#[must_use]
pub fn default_config_dir(
    xdg_config_home: Option<PathBuf>,
    home_dir: Option<PathBuf>,
) -> Option<PathBuf> {
    default_config_dirs(xdg_config_home, home_dir)
        .into_iter()
        .next()
}

#[must_use]
pub fn default_config_dirs(
    xdg_config_home: Option<PathBuf>,
    home_dir: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(path) = xdg_config_home {
        push_unique(&mut dirs, path.join(CONFIG_DIR_NAME));
        push_unique(&mut dirs, path.join(LEGACY_CONFIG_DIR_NAME));
    } else if let Some(path) = home_dir {
        let config_root = path.join(".config");
        push_unique(&mut dirs, config_root.join(CONFIG_DIR_NAME));
        push_unique(&mut dirs, config_root.join(LEGACY_CONFIG_DIR_NAME));
    }
    dirs
}

#[must_use]
pub fn config_file_path(config_dir: impl AsRef<Path>) -> PathBuf {
    config_dir.as_ref().join(CONFIG_FILE_NAME)
}

fn config_dir_for_path(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn push_unique(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !paths.iter().any(|path| path == &candidate) {
        paths.push(candidate);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn default_config_exposes_foundation_surfaces() {
        let config = AppConfig::default();
        let resolved =
            config
                .editor
                .resolve_edit_command("sh", Path::new("/tmp/repo/file.txt"), String::new);

        assert!(config.workspace.roots.is_empty());
        assert_eq!(config.workspace.ignores, default_workspace_ignores());
        assert_eq!(config.os.open, "");
        assert_eq!(config.os.open_link, "");
        assert_eq!(config.os.copy_to_clipboard_cmd, "");
        assert_eq!(config.os.read_from_clipboard_cmd, "");
        assert_eq!(config.os.shell_functions_file, "");
        assert_eq!(config.editor.command, "");
        assert!(config.editor.args.is_empty());
        assert_eq!(resolved.command, "vim -- '/tmp/repo/file.txt'");
        assert!(resolved.suspend);
        assert_eq!(config.theme.preset, ThemePreset::DefaultDark);
        assert_eq!(config.theme.active_border_color, ["green", "bold"]);
        assert_eq!(config.theme.selected_line_bg_color, ["blue"]);
        assert_eq!(config.theme.inactive_view_selected_line_bg_color, ["bold"]);
        assert_eq!(config.theme.default_fg_color, ["default"]);
        assert!(config.keybindings.overrides.is_empty());
        assert!(config.diagnostics.enabled);
    }

    #[test]
    fn default_config_toml_renders_documented_sections() {
        let rendered = default_config_toml().expect("serialize default config");

        assert!(rendered.contains("[workspace]"));
        assert!(rendered.contains("[os]"));
        assert!(rendered.contains("[editor]"));
        assert!(rendered.contains("[theme]"));
        assert!(rendered.contains("active_border_color = ["));
        assert!(rendered.contains("selected_line_bg_color = ["));
        assert!(rendered.contains("[diagnostics]"));
    }

    #[test]
    fn load_with_discovery_parses_upstream_os_command_overrides() {
        let parsed: AppConfig = toml::from_str(
            r#"
[os]
open = "custom-open {{filename}}"
openLink = "custom-link {{link}}"
copyToClipboardCmd = "copy {{text}}"
readFromClipboardCmd = "paste"
shellFunctionsFile = "~/.config/lazygit/functions.sh"
"#,
        )
        .expect("parse os config");

        assert_eq!(parsed.os.open, "custom-open {{filename}}");
        assert_eq!(parsed.os.open_link, "custom-link {{link}}");
        assert_eq!(parsed.os.copy_to_clipboard_cmd, "copy {{text}}");
        assert_eq!(parsed.os.read_from_clipboard_cmd, "paste");
        assert_eq!(
            parsed.os.shell_functions_file,
            "~/.config/lazygit/functions.sh"
        );
    }

    #[test]
    fn config_dir_helpers_prefer_xdg_before_home() {
        assert_eq!(
            default_config_dir(
                Some(PathBuf::from("/xdg")),
                Some(PathBuf::from("/home/quangdang"))
            ),
            Some(PathBuf::from("/xdg/super-lazygit"))
        );
        assert_eq!(
            default_config_dir(None, Some(PathBuf::from("/home/quangdang"))),
            Some(PathBuf::from("/home/quangdang/.config/super-lazygit"))
        );
        assert_eq!(
            config_file_path(Path::new("/tmp/config-dir")),
            PathBuf::from("/tmp/config-dir/config.toml")
        );
        assert_eq!(
            default_config_dirs(
                Some(PathBuf::from("/xdg")),
                Some(PathBuf::from("/home/quangdang"))
            ),
            vec![
                PathBuf::from("/xdg/super-lazygit"),
                PathBuf::from("/xdg/lazygit"),
            ]
        );
    }

    #[test]
    fn discovery_tracks_effective_config_dir_and_candidate_path() {
        let discovery = ConfigDiscovery::with_overrides(
            None,
            Some(PathBuf::from("/tmp/custom.toml")),
            Some(PathBuf::from("/xdg")),
            Some(PathBuf::from("/home/quangdang")),
        );

        assert_eq!(
            discovery,
            ConfigDiscovery {
                explicit_path: Some(PathBuf::from("/tmp/custom.toml")),
                config_dir: Some(PathBuf::from("/tmp")),
                candidates: vec![],
            }
        );
        assert_eq!(
            discovery.primary_config_path(),
            Some(Path::new("/tmp/custom.toml"))
        );
    }

    #[test]
    fn discovery_prefers_explicit_dir_before_standard_locations() {
        let discovery = ConfigDiscovery::with_overrides(
            Some(PathBuf::from("/tmp/override")),
            None,
            Some(PathBuf::from("/xdg")),
            Some(PathBuf::from("/home/quangdang")),
        );

        assert_eq!(
            discovery,
            ConfigDiscovery {
                explicit_path: None,
                config_dir: Some(PathBuf::from("/tmp/override")),
                candidates: vec![PathBuf::from("/tmp/override/config.toml")],
            }
        );
        assert_eq!(
            discovery.primary_config_path(),
            Some(Path::new("/tmp/override/config.toml"))
        );
    }

    #[test]
    fn discovery_adds_legacy_lazygit_candidate_after_primary_dir() {
        let discovery = ConfigDiscovery::with_overrides(
            None,
            None,
            Some(PathBuf::from("/xdg")),
            Some(PathBuf::from("/home/quangdang")),
        );

        assert_eq!(
            discovery.candidates,
            vec![
                PathBuf::from("/xdg/super-lazygit/config.toml"),
                PathBuf::from("/xdg/lazygit/config.toml"),
            ]
        );
    }

    #[test]
    fn discovery_prefers_legacy_lg_config_file_env_alias() {
        let original_super = std::env::var_os("SUPER_LAZYGIT_CONFIG");
        let original_legacy = std::env::var_os("LG_CONFIG_FILE");
        let original_dir = std::env::var_os("CONFIG_DIR");
        std::env::remove_var("SUPER_LAZYGIT_CONFIG");
        std::env::set_var("LG_CONFIG_FILE", "/tmp/legacy-config.toml");
        std::env::set_var("CONFIG_DIR", "/tmp/legacy-dir");

        let discovery = ConfigDiscovery::from_env();

        match original_super {
            Some(value) => std::env::set_var("SUPER_LAZYGIT_CONFIG", value),
            None => std::env::remove_var("SUPER_LAZYGIT_CONFIG"),
        }
        match original_legacy {
            Some(value) => std::env::set_var("LG_CONFIG_FILE", value),
            None => std::env::remove_var("LG_CONFIG_FILE"),
        }
        match original_dir {
            Some(value) => std::env::set_var("CONFIG_DIR", value),
            None => std::env::remove_var("CONFIG_DIR"),
        }

        assert_eq!(
            discovery.primary_config_path(),
            Some(Path::new("/tmp/legacy-config.toml"))
        );
        assert_eq!(discovery.config_dir(), Some(Path::new("/tmp")));
    }

    #[test]
    fn load_with_discovery_uses_defaults_when_no_file_exists() {
        let loaded = AppConfig::load_with_discovery(ConfigDiscovery::new(None, None, None))
            .expect("default config");

        assert_eq!(
            loaded,
            LoadedConfig {
                config: AppConfig::default(),
                source: ConfigSource::Defaults,
            }
        );
    }

    #[test]
    fn load_with_discovery_reads_first_existing_candidate() {
        let tempdir = tempfile::tempdir().expect("config tempdir");
        let config_home = tempdir.path().join("xdg");
        let config_path = config_home.join("super-lazygit").join("config.toml");
        fs::create_dir_all(config_path.parent().expect("config parent"))
            .expect("create config dir");
        fs::write(
            &config_path,
            r#"
[editor]
command = "nvim"
"#,
        )
        .expect("write config");

        let loaded = AppConfig::load_with_discovery(ConfigDiscovery::new(
            None,
            Some(config_home),
            Some(tempdir.path().join("home")),
        ))
        .expect("load config");

        assert_eq!(loaded.config.editor.command, "nvim");
        assert_eq!(loaded.source.path(), Some(config_path.as_path()));
    }

    #[test]
    fn load_with_discovery_prefers_explicit_dir_over_standard_candidates() {
        let tempdir = tempfile::tempdir().expect("config tempdir");
        let explicit_dir = tempdir.path().join("override");
        let explicit_path = explicit_dir.join("config.toml");
        fs::create_dir_all(&explicit_dir).expect("create explicit config dir");
        fs::write(&explicit_path, "[editor]\ncommand = \"helix\"\n")
            .expect("write explicit config");

        let xdg_path = tempdir
            .path()
            .join("xdg")
            .join("super-lazygit")
            .join("config.toml");
        fs::create_dir_all(xdg_path.parent().expect("xdg parent")).expect("create xdg dir");
        fs::write(&xdg_path, "[editor]\ncommand = \"nvim\"\n").expect("write xdg config");

        let loaded = AppConfig::load_with_discovery(ConfigDiscovery::with_overrides(
            Some(explicit_dir),
            None,
            Some(tempdir.path().join("xdg")),
            Some(tempdir.path().join("home")),
        ))
        .expect("load config");

        assert_eq!(loaded.config.editor.command, "helix");
        assert_eq!(loaded.source.path(), Some(explicit_path.as_path()));
    }

    #[test]
    fn load_with_discovery_errors_when_explicit_path_is_missing() {
        let error = AppConfig::load_with_discovery(ConfigDiscovery::new(
            Some(PathBuf::from("/tmp/missing-super-lazygit-config.toml")),
            None,
            None,
        ))
        .expect_err("explicit path should fail");

        assert!(matches!(
            error,
            ConfigLoadError::MissingExplicitPath { ref path }
                if path == Path::new("/tmp/missing-super-lazygit-config.toml")
        ));
    }

    #[test]
    fn load_with_discovery_surfaces_toml_errors_with_path_context() {
        let tempdir = tempfile::tempdir().expect("config tempdir");
        let config_path = tempdir.path().join("broken.toml");
        fs::write(&config_path, "not = [valid").expect("write broken config");

        let error = AppConfig::load_with_discovery(ConfigDiscovery::new(
            Some(config_path.clone()),
            None,
            None,
        ))
        .expect_err("invalid toml should fail");

        assert!(matches!(
            error,
            ConfigLoadError::Parse { ref path, .. } if path == &config_path
        ));
    }
    #[test]
    fn legacy_editor_command_appends_filename_after_quoted_args() {
        let config = EditorConfig {
            command: "nvim".to_string(),
            args: vec!["-f".to_string(), "--clean".to_string()],
            ..Default::default()
        };

        let resolved =
            config.resolve_edit_command("sh", Path::new("/tmp/repo/file.txt"), String::new);

        assert_eq!(
            resolved.command,
            "'nvim' '-f' '--clean' '/tmp/repo/file.txt'"
        );
        assert!(resolved.suspend);
    }

    #[test]
    fn guessed_editor_uses_vscode_preset_without_tui_suspend() {
        let resolved = EditorConfig::default().resolve_edit_at_line_and_wait_command(
            "sh",
            Path::new("/tmp/repo/file.txt"),
            42,
            || "code --wait".to_string(),
        );

        assert_eq!(
            resolved.command,
            "code --reuse-window --goto --wait -- '/tmp/repo/file.txt':42"
        );
        assert!(!resolved.suspend);
    }

    #[test]
    fn open_dir_command_resolves_custom_dir_placeholder() {
        let config = EditorConfig {
            open_dir_in_editor: "code --add {{dir}}".to_string(),
            ..Default::default()
        };

        let resolved = config.resolve_open_dir_command("sh", Path::new("/tmp/repo"), String::new);

        assert_eq!(resolved.command, "code --add '/tmp/repo'");
        assert!(resolved.suspend);
    }

    #[test]
    fn nvim_remote_preset_matches_upstream_shell_variants() {
        let config = EditorConfig {
            edit_preset: "nvim-remote".to_string(),
            ..Default::default()
        };

        let fish =
            config.resolve_edit_command("fish", Path::new("/tmp/repo/file.txt"), String::new);
        assert!(fish
            .command
            .starts_with("begin; if test -z \"$NVIM\"; nvim -- '/tmp/repo/file.txt'; else;"));
        assert!(fish.suspend);

        let nu = config.resolve_edit_command("nu", Path::new("/tmp/repo/file.txt"), String::new);
        assert!(nu.command.starts_with(
            "if ($env | get -i NVIM | is-empty) { nvim -- '/tmp/repo/file.txt' } else {"
        ));
        assert!(nu.suspend);

        let sh = config.resolve_edit_command("sh", Path::new("/tmp/repo/file.txt"), String::new);
        assert!(sh
            .command
            .starts_with("[ -z \"$NVIM\" ] && (nvim -- '/tmp/repo/file.txt') ||"));
        assert!(sh.suspend);
    }

    #[test]
    fn guessed_editor_maps_upstream_aliases_to_presets() {
        let hx = EditorConfig::default().resolve_edit_command(
            "sh",
            Path::new("/tmp/repo/file.txt"),
            || "hx".to_string(),
        );
        assert_eq!(hx.command, "hx -- '/tmp/repo/file.txt'");
        assert!(hx.suspend);

        let subl = EditorConfig::default().resolve_edit_command(
            "sh",
            Path::new("/tmp/repo/file.txt"),
            || "subl".to_string(),
        );
        assert_eq!(subl.command, "subl -- '/tmp/repo/file.txt'");
        assert!(!subl.suspend);
    }
}

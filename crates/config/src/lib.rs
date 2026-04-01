use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const CONFIG_DIR_NAME: &str = "super-lazygit";
const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub workspace: WorkspaceConfig,
    pub editor: EditorConfig,
    pub theme: ThemeConfig,
    pub keybindings: KeybindingConfig,
    pub diagnostics: DiagnosticsConfig,
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
            None,
            env::var_os("SUPER_LAZYGIT_CONFIG")
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
        let default_dir = default_config_dir(xdg_config_home, home_dir);
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
                if let Some(default_dir) = default_dir {
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

#[must_use]
pub fn default_config_dir(
    xdg_config_home: Option<PathBuf>,
    home_dir: Option<PathBuf>,
) -> Option<PathBuf> {
    xdg_config_home
        .map(|path| path.join(CONFIG_DIR_NAME))
        .or_else(|| home_dir.map(|path| path.join(".config").join(CONFIG_DIR_NAME)))
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

        assert!(config.workspace.roots.is_empty());
        assert_eq!(config.workspace.ignores, default_workspace_ignores());
        assert_eq!(config.editor.command, "vim");
        assert!(config.editor.args.is_empty());
        assert_eq!(config.theme.preset, ThemePreset::DefaultDark);
        assert!(config.keybindings.overrides.is_empty());
        assert!(config.diagnostics.enabled);
    }

    #[test]
    fn default_config_toml_renders_documented_sections() {
        let rendered = default_config_toml().expect("serialize default config");

        assert!(rendered.contains("[workspace]"));
        assert!(rendered.contains("[editor]"));
        assert!(rendered.contains("[theme]"));
        assert!(rendered.contains("[diagnostics]"));
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
}

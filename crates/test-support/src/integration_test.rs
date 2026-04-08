//! Integration test infrastructure for super-lazygit.
//!
//! This module provides the core types for defining and running integration tests
//! against the super-lazygit TUI, following the patterns established in lazygit's
//! integration test framework.

use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// The description used for unit tests to avoid panicking when trying to get
/// a test's name via its file path.
pub const UNIT_TEST_DESCRIPTION: &str = "test test";

/// Default dimensions for running tests in headless mode.
pub const DEFAULT_WIDTH: u32 = 150;
pub const DEFAULT_HEIGHT: u32 = 100;

/// The delay in milliseconds between keypresses or mouse clicks in tests.
/// Defaults to zero. Set via INPUT_DELAY environment variable.
pub fn input_delay() -> u64 {
    let delay_str = env::var("INPUT_DELAY").unwrap_or_default();
    if delay_str.is_empty() {
        return 0;
    }

    delay_str
        .parse::<u64>()
        .expect("INPUT_DELAY must be a valid integer")
}

/// Extracts the test name from a file path.
/// The path should be in the format ".../integration/tests/<name>.go"
/// and this function returns just the `<name>` portion.
pub fn test_name_from_file_path(path: &str) -> &str {
    let name = path.split("integration/tests/").nth(1).unwrap_or(path);
    &name[..name.len() - 3]
}

/// Restricts a test to run only on specific git versions.
#[derive(Debug, Clone, Default)]
pub struct GitVersionRestriction {
    /// Minimum version (inclusive). Set via AtLeast().
    from: Option<String>,
    /// Maximum version (exclusive). Set via Before().
    before: Option<String>,
    /// Exact versions to match. Set via Includes().
    includes: Option<Vec<String>>,
}

/// Creates a version restriction that requires at least the given version (inclusive).
pub fn at_least(version: &str) -> GitVersionRestriction {
    GitVersionRestriction {
        from: Some(version.to_string()),
        before: None,
        includes: None,
    }
}

/// Creates a version restriction that requires versions before the given version (exclusive).
pub fn before(version: &str) -> GitVersionRestriction {
    GitVersionRestriction {
        from: None,
        before: Some(version.to_string()),
        includes: None,
    }
}

/// Creates a version restriction that only includes the given versions.
pub fn includes(versions: &[&str]) -> GitVersionRestriction {
    GitVersionRestriction {
        from: None,
        before: None,
        includes: Some(versions.iter().map(|s| s.to_string()).collect()),
    }
}

impl GitVersionRestriction {
    /// Returns true if the given version satisfies this restriction.
    ///
    /// # Panics
    ///
    /// Panics if the restriction has an invalid version string.
    pub fn should_run_on_version(&self, version: &GitVersion) -> bool {
        if let Some(ref from) = self.from {
            let from_ver =
                GitVersion::from_str(from).unwrap_or_else(|_| panic!("Invalid git version string: {from}"));
            return version.is_at_least(&from_ver);
        }

        if let Some(ref before) = self.before {
            let before_ver = GitVersion::from_str(before)
                .unwrap_or_else(|_| panic!("Invalid git version string: {before}"));
            return version.is_older_than(&before_ver);
        }

        if let Some(ref includes) = self.includes {
            return includes.iter().any(|s| {
                let v = GitVersion::from_str(s).unwrap_or_else(|_| panic!("Invalid git version string: {s}"));
                version.major == v.major && version.minor == v.minor && version.patch == v.patch
            });
        }

        true
    }
}

/// Represents a parsed git version (e.g., "2.30.0").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl FromStr for GitVersion {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid git version format: {s}"));
        }

        let major = parts[0]
            .parse()
            .map_err(|_| format!("Invalid major version: {}", parts[0]))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| format!("Invalid minor version: {}", parts[1]))?;
        let patch = parts[2]
            .parse()
            .map_err(|_| format!("Invalid patch version: {}", parts[2]))?;

        Ok(GitVersion {
            major,
            minor,
            patch,
        })
    }
}

impl GitVersion {
    /// Creates a new GitVersion from a version string.
    ///
    /// # Panics
    ///
    /// Panics if the string cannot be parsed.
    pub fn parse(s: &str) -> GitVersion {
        GitVersion::from_str(s).unwrap_or_else(|_| panic!("Invalid git version string: {s}"))
    }

    /// Returns true if this version is at least the given version.
    pub fn is_at_least(&self, other: &GitVersion) -> bool {
        if self.major != other.major {
            return self.major >= other.major;
        }
        if self.minor != other.minor {
            return self.minor >= other.minor;
        }
        self.patch >= other.patch
    }

    /// Returns true if this version is older than the given version.
    pub fn is_older_than(&self, other: &GitVersion) -> bool {
        if self.major != other.major {
            return self.major < other.major;
        }
        if self.minor != other.minor {
            return self.minor < other.minor;
        }
        self.patch < other.patch
    }
}

/// Arguments for creating a new integration test.
#[derive(Default)]
pub struct NewIntegrationTestArgs {
    /// Briefly describes what happens in the test and what it's testing for.
    pub description: String,
    /// Prepares a repo for testing.
    pub setup_repo: Option<Box<dyn Fn(&TestShell)>>,
    /// Takes a config and mutates it. The mutated context will be passed to the GUI.
    pub setup_config: Option<Box<dyn Fn(&mut AppConfig)>>,
    /// Runs the test.
    pub run: Option<Box<dyn Fn(&TestDriver, KeybindingConfig)>>,
    /// Additional args passed to lazygit.
    pub extra_cmd_args: Vec<String>,
    /// Additional environment variables.
    pub extra_env_vars: std::collections::HashMap<String, String>,
    /// For when a test is flaky.
    pub skip: bool,
    /// To run a test only on certain git versions.
    pub git_version: GitVersionRestriction,
    /// Width and height when running in headless mode.
    pub width: u32,
    pub height: u32,
    /// If true, this is not a test but a demo to be added to docs.
    pub is_demo: bool,
}


/// Test shell operations for setting up test repositories.
/// Provides git command execution and file manipulation for test setup.
pub struct TestShell {
    pwd: PathBuf,
    env: Vec<(String, String)>,
}

impl TestShell {
    pub fn new(pwd: &Path, env: &[(String, String)]) -> Self {
        TestShell {
            pwd: pwd.to_path_buf(),
            env: env.to_vec(),
        }
    }

    /// Creates an empty commit with the given message.
    pub fn empty_commit(&self, message: &str) -> &Self {
        let _ = std::process::Command::new("git")
            .args(["commit", "--allow-empty", "-m", message])
            .current_dir(&self.pwd)
            .output();
        self
    }

    /// Creates a file with the given content.
    pub fn create_file(&self, path: &str, content: &str) -> &Self {
        let full_path = self.pwd.join(path);
        if let Some(parent) = full_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&full_path, content);
        self
    }

    /// Stages a file with git add.
    pub fn git_add(&self, path: &str) -> &Self {
        let _ = std::process::Command::new("git")
            .args(["add", path])
            .current_dir(&self.pwd)
            .output();
        self
    }

    /// Creates a file and stages it.
    pub fn create_file_and_add(&self, path: &str, content: &str) -> &Self {
        self.create_file(path, content).git_add(path)
    }

    /// Updates a file with new content.
    pub fn update_file(&self, path: &str, content: &str) -> &Self {
        self.create_file(path, content)
    }
}

/// Stub for test driver.
/// In a full implementation, this would drive the GUI by pressing keys, selecting items, etc.
pub struct TestDriver;

impl TestDriver {
    pub fn new(_gui: &GuiDriver, _shell: &TestShell, _keys: KeybindingConfig) -> Self {
        TestDriver
    }

    pub fn set_caption(&self, _caption: &str) {}
    pub fn set_caption_prefix(&self, _prefix: &str) {}
    pub fn wait(&self, _ms: u64) {}
    pub fn global_press(&self, _key: &str) {}
    pub fn expect_popup(&self) -> PopupDriver {
        PopupDriver
    }
}

/// Stub for popup driver.
pub struct PopupDriver;

impl PopupDriver {
    pub fn prompt(&self) -> PromptDriver {
        PromptDriver
    }
}

/// Stub for prompt driver.
pub struct PromptDriver;

impl PromptDriver {
    pub fn title(&self, _t: &str) -> &Self {
        self
    }
    pub fn suggestion_lines(&self, _matchers: &[&dyn Fn(&str) -> bool]) -> &Self {
        self
    }
    pub fn r#type(&self, _content: &str) -> &Self {
        self
    }
    pub fn confirm(&self) -> &Self {
        self
    }
    pub fn cancel(&self) -> &Self {
        self
    }
}

/// Stub for GUI driver interface.
/// In a full implementation, this would provide access to the GUI state and controls.
pub struct GuiDriver;

impl GuiDriver {
    pub fn fail(&self, _error_msg: &str) {}
    pub fn keys(&self) -> KeybindingConfig {
        KeybindingConfig
    }
    pub fn check_all_toasts_acknowledged(&self) {}
}

/// Stub for keybinding configuration.
/// In a full implementation, this would be the actual keybinding config.
#[derive(Debug, Clone, Default)]
pub struct KeybindingConfig;

impl KeybindingConfig {
    pub fn universal(&self) -> UniversalKeybindings {
        UniversalKeybindings::default()
    }
}

/// Stub for universal keybindings.
#[derive(Debug, Clone)]
pub struct UniversalKeybindings {
    pub execute_shell_command: String,
}

impl Default for UniversalKeybindings {
    fn default() -> Self {
        UniversalKeybindings {
            execute_shell_command: "<c-e>".to_string(),
        }
    }
}

/// Stub for app config.
/// This is a simplified version of the config.AppConfig from crates/config.
#[derive(Debug, Clone, Default)]
pub struct AppConfig;

impl AppConfig {
    pub fn new() -> Self {
        AppConfig
    }
}

/// Describes an integration test that will be run against the super-lazygit GUI.
///
/// Our unit tests will use the description field to avoid a panic caused by attempting
/// to get the test's name via its file's path.
pub struct IntegrationTest {
    name: String,
    description: String,
    extra_cmd_args: Vec<String>,
    extra_env_vars: std::collections::HashMap<String, String>,
    skip: bool,
    setup_repo: Option<Box<dyn Fn(&TestShell)>>,
    setup_config: Option<Box<dyn Fn(&mut AppConfig)>>,
    run: Option<Box<dyn Fn(&TestDriver, KeybindingConfig)>>,
    git_version: GitVersionRestriction,
    width: u32,
    height: u32,
    is_demo: bool,
}

impl IntegrationTest {
    /// Creates a new integration test from the given arguments.
    pub fn new(args: NewIntegrationTestArgs) -> Self {
        let name = if args.description != UNIT_TEST_DESCRIPTION {
            // In Go, this panics if we're in a unit test for our integration tests,
            // so we're using "test test" as a sentinel value.
            // In Rust, we'd need access to the caller's file path, which is complex.
            // For now, we use the description as a fallback.
            args.description.clone()
        } else {
            String::new()
        };

        IntegrationTest {
            name,
            description: args.description,
            extra_cmd_args: args.extra_cmd_args,
            extra_env_vars: args.extra_env_vars,
            skip: args.skip,
            setup_repo: args.setup_repo,
            setup_config: args.setup_config,
            run: args.run,
            git_version: args.git_version,
            width: args.width,
            height: args.height,
            is_demo: args.is_demo,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn extra_cmd_args(&self) -> &[String] {
        &self.extra_cmd_args
    }

    pub fn extra_env_vars(&self) -> &std::collections::HashMap<String, String> {
        &self.extra_env_vars
    }

    pub fn skip(&self) -> bool {
        self.skip
    }

    pub fn is_demo(&self) -> bool {
        self.is_demo
    }

    pub fn should_run_for_git_version(&self, version: &GitVersion) -> bool {
        self.git_version.should_run_on_version(version)
    }

    pub fn setup_config(&self, config: &mut AppConfig) {
        if let Some(ref setup) = self.setup_config {
            setup(config);
        }
    }

    pub fn setup_repo(&self, shell: &TestShell) {
        if let Some(ref setup) = self.setup_repo {
            setup(shell);
        }
    }

    /// Runs the integration test.
    ///
    /// # Panics
    ///
    /// Panics if the current directory cannot be obtained or if INPUT_DELAY parsing fails.
    pub fn run(&self, gui: GuiDriver) {
        let pwd = std::env::current_dir().expect("Failed to get current directory");

        let shell = TestShell::new(&pwd, &[]);
        let keys = gui.keys();
        let test_driver = TestDriver::new(&gui, &shell, keys.clone());

        if input_delay() > 0 {
            test_driver.set_caption("");
            test_driver.set_caption_prefix("");
        }

        if let Some(ref run_fn) = self.run {
            run_fn(&test_driver, keys);
        }

        gui.check_all_toasts_acknowledged();

        if input_delay() > 0 {
            test_driver.set_caption("");
            test_driver.set_caption_prefix("");
            test_driver.wait(2000);
        }
    }

    /// Returns the dimensions for running in headless mode.
    pub fn headless_dimensions(&self) -> (u32, u32) {
        if self.width == 0 && self.height == 0 {
            return (DEFAULT_WIDTH, DEFAULT_HEIGHT);
        }
        (self.width, self.height)
    }

    /// Returns true if the test requires headless mode.
    pub fn requires_headless(&self) -> bool {
        self.width != 0 && self.height != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_version_parse() {
        let v = GitVersion::parse("2.30.0");
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 30);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_git_version_is_at_least() {
        let v1 = GitVersion::parse("2.30.0");
        let v2 = GitVersion::parse("2.30.0");
        let v3 = GitVersion::parse("2.31.0");
        let v4 = GitVersion::parse("3.0.0");

        assert!(v1.is_at_least(&v2));
        assert!(v3.is_at_least(&v1));
        assert!(v4.is_at_least(&v1));
        assert!(!v1.is_at_least(&v3));
    }

    #[test]
    fn test_git_version_is_older_than() {
        let v1 = GitVersion::parse("2.30.0");
        let v2 = GitVersion::parse("2.30.0");
        let v3 = GitVersion::parse("2.31.0");
        let v4 = GitVersion::parse("3.0.0");

        assert!(!v1.is_older_than(&v2));
        assert!(v1.is_older_than(&v3));
        assert!(v1.is_older_than(&v4));
        assert!(!v3.is_older_than(&v1));
    }

    #[test]
    fn test_git_version_restriction_at_least() {
        let restriction = at_least("2.30.0");
        assert!(restriction.should_run_on_version(&GitVersion::parse("2.30.0")));
        assert!(restriction.should_run_on_version(&GitVersion::parse("2.31.0")));
        assert!(restriction.should_run_on_version(&GitVersion::parse("3.0.0")));
        assert!(!restriction.should_run_on_version(&GitVersion::parse("2.29.0")));
    }

    #[test]
    fn test_git_version_restriction_before() {
        let restriction = before("2.30.0");
        assert!(!restriction.should_run_on_version(&GitVersion::parse("2.30.0")));
        assert!(!restriction.should_run_on_version(&GitVersion::parse("2.31.0")));
        assert!(restriction.should_run_on_version(&GitVersion::parse("2.29.0")));
    }

    #[test]
    fn test_git_version_restriction_includes() {
        let restriction = includes(&["2.30.0", "2.31.0"]);
        assert!(restriction.should_run_on_version(&GitVersion::parse("2.30.0")));
        assert!(restriction.should_run_on_version(&GitVersion::parse("2.31.0")));
        assert!(!restriction.should_run_on_version(&GitVersion::parse("2.29.0")));
        assert!(!restriction.should_run_on_version(&GitVersion::parse("3.0.0")));
    }

    #[test]
    fn test_input_delay_default() {
        // Clear the env var if set
        env::remove_var("INPUT_DELAY");
        assert_eq!(input_delay(), 0);
    }

    #[test]
    fn test_test_name_from_file_path() {
        let path = "/some/path/integration/tests/my_test.go";
        assert_eq!(test_name_from_file_path(path), "my_test");
    }

    #[test]
    fn test_integration_test_default_dimensions() {
        let args = NewIntegrationTestArgs {
            description: "test".to_string(),
            ..Default::default()
        };
        let test = IntegrationTest::new(args);
        assert_eq!(test.headless_dimensions(), (DEFAULT_WIDTH, DEFAULT_HEIGHT));
        assert!(!test.requires_headless());
    }

    #[test]
    fn test_integration_test_custom_dimensions() {
        let args = NewIntegrationTestArgs {
            description: "test".to_string(),
            width: 800,
            height: 600,
            ..Default::default()
        };
        let test = IntegrationTest::new(args);
        assert_eq!(test.headless_dimensions(), (800, 600));
        assert!(test.requires_headless());
    }

    #[test]
    fn test_integration_test_skip() {
        let args = NewIntegrationTestArgs {
            description: "test".to_string(),
            skip: true,
            ..Default::default()
        };
        let test = IntegrationTest::new(args);
        assert!(test.skip());
    }

    #[test]
    fn test_integration_test_is_demo() {
        let args = NewIntegrationTestArgs {
            description: "test".to_string(),
            is_demo: true,
            ..Default::default()
        };
        let test = IntegrationTest::new(args);
        assert!(test.is_demo());
    }
}

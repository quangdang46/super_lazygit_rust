use std::collections::HashMap;
use std::io;
use std::process::{Command, Output};
use std::sync::Mutex;

/// Cached git config reader.
///
/// This provides efficient repeated access to git config values by caching them.
pub struct CachedGitConfig {
    cache: Mutex<HashMap<String, String>>,
}

impl CachedGitConfig {
    /// Create a new cached git config with a default runner.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Get a config value by key.
    ///
    /// Uses `--get --null` to handle values with newlines.
    pub fn get(&self, key: &str) -> String {
        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(value) = cache.get(key) {
                return value.clone();
            }
        }

        // Not in cache, fetch it
        let value = self.get_aux(key);

        // Store in cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(key.to_string(), value.clone());
        }

        value
    }

    /// Get a config value using general args (e.g., "--local --get-regexp mykey").
    pub fn get_general(&self, args: &str) -> String {
        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(value) = cache.get(args) {
                return value.clone();
            }
        }

        // Not in cache, fetch it
        let value = self.get_general_aux(args);

        // Store in cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(args.to_string(), value.clone());
        }

        value
    }

    /// Get a config value as a boolean.
    pub fn get_bool(&self, key: &str) -> bool {
        is_truthy(&self.get(key))
    }

    /// Clear the cache.
    pub fn drop_cache(&self) {
        let mut cache = self.cache.lock().unwrap();
        *cache = HashMap::new();
    }

    fn get_aux(&self, key: &str) -> String {
        let output = Command::new("git")
            .args(["config", "--get", "--null", key])
            .output();

        self.process_output(output, key)
    }

    fn get_general_aux(&self, args: &str) -> String {
        let git_args = std::iter::once("config")
            .chain(args.split_whitespace())
            .collect::<Vec<_>>();

        let output = Command::new("git").args(&git_args).output();

        self.process_general_output(output, args)
    }

    fn process_output(&self, output: io::Result<Output>, key: &str) -> String {
        match output {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout).trim_end_matches('\0').to_string()
            }
            Ok(o) => {
                // Key not found or other error
                String::new()
            }
            Err(e) => {
                // Error executing command
                String::new()
            }
        }
    }

    fn process_general_output(&self, output: io::Result<Output>, args: &str) -> String {
        match output {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            }
            Ok(_) => String::new(),
            Err(_) => String::new(),
        }
    }
}

impl Default for CachedGitConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a string value is truthy.
fn is_truthy(value: &str) -> bool {
    let lc_value = value.to_lowercase();
    lc_value == "true"
        || lc_value == "1"
        || lc_value == "yes"
        || lc_value == "on"
        || lc_value == "y"
        || lc_value == "t"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_truthy() {
        assert!(is_truthy("true"));
        assert!(is_truthy("True"));
        assert!(is_truthy("TRUE"));
        assert!(is_truthy("1"));
        assert!(is_truthy("yes"));
        assert!(is_truthy("YES"));
        assert!(is_truthy("on"));
        assert!(is_truthy("y"));
        assert!(is_truthy("t"));

        assert!(!is_truthy("false"));
        assert!(!is_truthy("0"));
        assert!(!is_truthy("no"));
        assert!(!is_truthy("off"));
        assert!(!is_truthy(""));
    }

    #[test]
    fn test_cached_git_config() {
        let config = CachedGitConfig::new();
        // This will return empty string if not in a git repo
        let value = config.get("user.name");
        // Just test it doesn't panic
        let _ = value;
    }
}

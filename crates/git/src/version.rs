use regex::Regex;
use std::process::Command;

use super_lazygit_core::version_number::VersionNumber;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitVersion {
    pub inner: VersionNumber,
    pub additional: String,
}

#[derive(Debug, Error)]
#[error("unexpected git version format: {0}")]
pub struct ParseGitVersionError(String);

impl GitVersion {
    pub fn is_older_than(&self, major: u32, minor: u32, patch: u32) -> bool {
        self.inner
            .is_older_than(&VersionNumber::new(major, minor, patch))
    }

    pub fn is_older_than_version(&self, version: &GitVersion) -> bool {
        self.is_older_than(
            version.inner.major,
            version.inner.minor,
            version.inner.patch,
        )
    }

    pub fn is_at_least(&self, major: u32, minor: u32, patch: u32) -> bool {
        !self.is_older_than(major, minor, patch)
    }

    pub fn is_at_least_version(&self, version: &GitVersion) -> bool {
        self.is_at_least(
            version.inner.major,
            version.inner.minor,
            version.inner.patch,
        )
    }
}

pub fn parse_git_version(version_str: &str) -> Result<GitVersion, ParseGitVersionError> {
    let re = Regex::new(r"[^\d]*(\d+)(\.\d+)?(\.\d+)?(.*)").unwrap();
    let caps = re
        .captures(version_str)
        .ok_or_else(|| ParseGitVersionError(version_str.to_string()))?;
    let matches: Vec<&str> = caps
        .iter()
        .map(|m| m.map(|m| m.as_str()).unwrap_or(""))
        .collect();

    if matches.len() < 5 {
        return Err(ParseGitVersionError(version_str.to_string()));
    }

    let major: u32 = matches[1]
        .parse()
        .map_err(|_| ParseGitVersionError(version_str.to_string()))?;

    let minor: u32 = if matches[2].len() > 1 {
        matches[2][1..]
            .parse()
            .map_err(|_| ParseGitVersionError(version_str.to_string()))?
    } else {
        0
    };

    let patch: u32 = if matches[3].len() > 1 {
        matches[3][1..]
            .parse()
            .map_err(|_| ParseGitVersionError(version_str.to_string()))?
    } else {
        0
    };

    let additional = matches[4].trim().to_string();

    Ok(GitVersion {
        inner: VersionNumber::new(major, minor, patch),
        additional,
    })
}

pub fn get_git_version() -> Result<GitVersion, std::io::Error> {
    let output = Command::new("git").arg("--version").output()?;
    if !output.status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("git --version failed: {}", output.status),
        ));
    }
    let version_str = String::from_utf8_lossy(&output.stdout);
    parse_git_version(version_str.trim())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_standard_version() {
        let v = parse_git_version("git version 2.39.0").unwrap();
        assert_eq!(v.inner.major, 2);
        assert_eq!(v.inner.minor, 39);
        assert_eq!(v.inner.patch, 0);
        assert_eq!(v.additional, "");
    }

    #[test]
    fn parse_apple_version() {
        let v = parse_git_version("git version 2.37.1 (Apple Git-137.1)").unwrap();
        assert_eq!(v.inner.major, 2);
        assert_eq!(v.inner.minor, 37);
        assert_eq!(v.inner.patch, 1);
        assert_eq!(v.additional, "(Apple Git-137.1)");
    }

    #[test]
    fn parse_version_without_patch() {
        let v = parse_git_version("git version 2.39").unwrap();
        assert_eq!(v.inner.major, 2);
        assert_eq!(v.inner.minor, 39);
        assert_eq!(v.inner.patch, 0);
    }

    #[test]
    fn parse_invalid_version() {
        assert!(parse_git_version("not git output").is_err());
        assert!(parse_git_version("").is_err());
    }

    #[test]
    fn is_older_than() {
        let v = GitVersion {
            inner: VersionNumber::new(2, 30, 0),
            additional: String::new(),
        };
        assert!(v.is_older_than(2, 33, 1));
        assert!(!v.is_older_than(2, 30, 0));
        assert!(!v.is_older_than(2, 29, 0));
        assert!(v.is_older_than(2, 30, 1));
    }

    #[test]
    fn is_at_least() {
        let v = GitVersion {
            inner: VersionNumber::new(2, 33, 1),
            additional: String::new(),
        };
        assert!(v.is_at_least(2, 33, 1));
        assert!(v.is_at_least(2, 30, 0));
        assert!(v.is_at_least(2, 33, 0));
        assert!(!v.is_at_least(2, 33, 2));
    }

    #[test]
    fn comparison_between_versions() {
        let v1 = GitVersion {
            inner: VersionNumber::new(2, 33, 1),
            additional: String::new(),
        };
        let v2 = GitVersion {
            inner: VersionNumber::new(2, 34, 0),
            additional: String::new(),
        };
        assert!(v1.is_older_than_version(&v2));
        assert!(!v2.is_older_than_version(&v1));
        assert!(v1.is_at_least_version(&v1));
        assert!(!v1.is_at_least_version(&v2));
    }
}

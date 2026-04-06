use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionNumber {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl VersionNumber {
    #[must_use]
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    #[must_use]
    pub fn is_older_than(&self, other: &VersionNumber) -> bool {
        let this = self.major * 1_000_000 + self.minor * 1_000 + self.patch;
        let that = other.major * 1_000_000 + other.minor * 1_000 + other.patch;
        this < that
    }
}

impl fmt::Display for VersionNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Ord for VersionNumber {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
    }
}

impl PartialOrd for VersionNumber {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseVersionError {
    input: String,
}

impl fmt::Display for ParseVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unexpected version format: {}", self.input)
    }
}

impl std::error::Error for ParseVersionError {}

pub fn parse_version_number(version_str: &str) -> Result<VersionNumber, ParseVersionError> {
    let s = version_str.strip_prefix('v').unwrap_or(version_str);

    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return Err(ParseVersionError {
            input: version_str.to_string(),
        });
    }

    let major = parts[0].parse::<u32>().map_err(|_| ParseVersionError {
        input: version_str.to_string(),
    })?;
    let minor = parts[1].parse::<u32>().map_err(|_| ParseVersionError {
        input: version_str.to_string(),
    })?;
    let patch = if parts.len() == 3 {
        parts[2].parse::<u32>().map_err(|_| ParseVersionError {
            input: version_str.to_string(),
        })?
    } else {
        0
    };

    Ok(VersionNumber::new(major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_version() {
        let v = parse_version_number("2.33.1").unwrap();
        assert_eq!(v, VersionNumber::new(2, 33, 1));
    }

    #[test]
    fn parse_version_with_v_prefix() {
        let v = parse_version_number("v1.2.3").unwrap();
        assert_eq!(v, VersionNumber::new(1, 2, 3));
    }

    #[test]
    fn parse_version_without_patch() {
        let v = parse_version_number("2.33").unwrap();
        assert_eq!(v, VersionNumber::new(2, 33, 0));
    }

    #[test]
    fn parse_invalid_version_returns_error() {
        assert!(parse_version_number("not-a-version").is_err());
        assert!(parse_version_number("").is_err());
        assert!(parse_version_number("1").is_err());
    }

    #[test]
    fn is_older_than_works() {
        let v1 = VersionNumber::new(2, 30, 0);
        let v2 = VersionNumber::new(2, 33, 1);
        assert!(v1.is_older_than(&v2));
        assert!(!v2.is_older_than(&v1));
        assert!(!v1.is_older_than(&v1));
    }

    #[test]
    fn ord_matches_is_older_than() {
        let v1 = VersionNumber::new(1, 0, 0);
        let v2 = VersionNumber::new(2, 0, 0);
        let v3 = VersionNumber::new(2, 1, 0);
        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
    }

    #[test]
    fn display_format() {
        let v = VersionNumber::new(2, 33, 1);
        assert_eq!(v.to_string(), "2.33.1");
    }
}

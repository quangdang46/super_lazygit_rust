use std::path::PathBuf;

use regex::Regex;

use super_lazygit_core::state::TagItem;

use crate::GitCommandBuilder;
use crate::GitResult;

/// Retrieves git tags
pub struct TagLoader {
    repo_path: PathBuf,
}

impl TagLoader {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    /// GetTags returns a list of tags sorted by creation date (descending)
    pub fn get_tags(&self) -> GitResult<Vec<TagItem>> {
        // get remote branches, sorted by creation date (descending)
        // see: https://git-scm.com/docs/git-tag#Documentation/git-tag.txt---sortltkeygt
        let output = git_builder_output(
            &self.repo_path,
            GitCommandBuilder::new("tag")
                .arg(["--list", "-n", "--sort=-creatordate"]),
        )?;

        if !output.status.success() {
            return Err(crate::GitError::OperationFailed {
                message: format!(
                    "git tag failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parse_tags(&stdout))
    }
}

/// Parses the git tag output into TagItem structs
/// Each line is either:
/// - "tagname" (lightweight tag)
/// - "tagname  commit message preview" (annotated tag)
fn parse_tags(output: &str) -> Vec<TagItem> {
    let line_regex = Regex::new(r"^([^\s]+)(\s+)?(.*)$").unwrap();

    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }

            let caps = line_regex.captures(line)?;
            let tag_name = caps.get(1)?.as_str().to_string();
            let message = caps
                .get(3)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();

            // For lightweight tags, message will be the commit subject line
            // For annotated tags, message is the tag message (first line)

            Some(TagItem {
                name: tag_name.clone(),
                target_oid: String::new(), // Would need additional queries to populate
                target_short_oid: String::new(),
                summary: message.clone(),
                annotated: !message.is_empty(),
            })
        })
        .collect()
}

fn git_builder_output(
    repo_path: &PathBuf,
    builder: GitCommandBuilder,
) -> GitResult<std::process::Output> {
    use std::process::Command;

    let argv = builder.into_args();
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path).args(&argv);

    cmd.output()
        .map_err(|e| crate::GitError::OperationFailed {
            message: format!("failed to execute git: {}", e),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tags_empty() {
        assert!(parse_tags("").is_empty());
        assert!(parse_tags("  ").is_empty());
    }

    #[test]
    fn test_parse_tags_lightweight() {
        let output = "v1.0.0\nv1.0.1\nv2.0.0";
        let tags = parse_tags(output);
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0].name, "v1.0.0");
        assert_eq!(tags[0].summary, "");
        assert!(!tags[0].annotated);
    }

    #[test]
    fn test_parse_tags_annotated() {
        let output = "v1.0.0  Release version 1.0.0\nv1.0.1  Bugfix release";
        let tags = parse_tags(output);
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "v1.0.0");
        assert_eq!(tags[0].summary, "Release version 1.0.0");
        assert!(tags[0].annotated);
        assert_eq!(tags[1].name, "v1.0.1");
        assert_eq!(tags[1].summary, "Bugfix release");
        assert!(tags[1].annotated);
    }

    #[test]
    fn test_parse_tags_mixed() {
        let output = "v1.0.0\nv1.0.1  With annotation\nv2.0.0";
        let tags = parse_tags(output);
        assert_eq!(tags.len(), 3);
        assert!(!tags[0].annotated);
        assert!(tags[1].annotated);
        assert!(!tags[2].annotated);
    }
}

use std::path::Path;

use crate::{git_stdout, GitResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchConfig {
    pub remote: String,
    pub merge: String,
}

pub fn needs_gpg_subprocess(
    key: &str,
    git_config_get_bool: impl Fn(&str) -> bool,
    override_gpg: bool,
) -> bool {
    if override_gpg {
        return false;
    }
    git_config_get_bool(key)
}

pub fn needs_gpg_subprocess_for_commit(
    git_config_get_bool: impl Fn(&str) -> bool,
    override_gpg: bool,
) -> bool {
    needs_gpg_subprocess("commit.gpgSign", git_config_get_bool, override_gpg)
}

pub fn get_gpg_tag_sign(git_config_get_bool: impl Fn(&str) -> bool) -> bool {
    git_config_get_bool("tag.gpgSign")
}

pub fn get_core_editor(git_config_get: impl Fn(&str) -> String) -> String {
    git_config_get("core.editor")
}

pub fn get_remote_url(git_config_get: impl Fn(&str) -> String) -> String {
    git_config_get("remote.origin.url")
}

pub fn get_show_untracked_files(git_config_get: impl Fn(&str) -> String) -> String {
    git_config_get("status.showUntrackedFiles")
}

pub fn get_push_to_current(git_config_get: impl Fn(&str) -> String) -> bool {
    git_config_get("push.default") == "current"
}

pub fn get_branches_config(
    repo_path: &Path,
) -> GitResult<std::collections::HashMap<String, BranchConfig>> {
    let output = git_stdout(
        repo_path,
        ["config", "--local", "--get-regexp", "^branch\\."],
    )?;

    let mut result = std::collections::HashMap::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        if parts.len() != 2 {
            continue;
        }

        let key = parts[0];
        let value = parts[1];

        if let Some(last_dot) = key.rfind('.') {
            let config_key = &key[last_dot + 1..];
            let branch_prefix = "branch.";
            if key.starts_with(branch_prefix) {
                let branch_name = &key[branch_prefix.len()..last_dot];

                let entry = result
                    .entry(branch_name.to_string())
                    .or_insert_with(|| BranchConfig {
                        remote: String::new(),
                        merge: String::new(),
                    });

                match config_key {
                    "remote" => entry.remote = value.to_string(),
                    "merge" => entry.merge = value.trim_start_matches("refs/heads/").to_string(),
                    _ => {}
                }
            }
        }
    }

    Ok(result)
}

pub fn get_git_flow_prefixes(git_config_get_general: impl Fn(&str) -> String) -> String {
    git_config_get_general("--local --get-regexp gitflow.prefix")
}

pub fn get_core_comment_char(git_config_get: impl Fn(&str) -> String) -> u8 {
    let comment_char_str = git_config_get("core.commentChar");
    if comment_char_str.len() == 1 {
        comment_char_str.as_bytes()[0]
    } else {
        b'#'
    }
}

pub fn get_rebase_update_refs(git_config_get_bool: impl Fn(&str) -> bool) -> bool {
    git_config_get_bool("rebase.updateRefs")
}

pub fn get_merge_ff(git_config_get: impl Fn(&str) -> String) -> String {
    git_config_get("merge.ff")
}

use std::ffi::{OsStr, OsString};
use std::path::Path;

use crate::GitCommandBuilder;
use crate::GitResult;

pub struct DiffToolCmdOptions {
    pub filepath: String,
    pub from_commit: String,
    pub to_commit: String,
    pub reverse: bool,
    pub is_directory: bool,
    pub staged: bool,
}

pub fn diff_cmd_obj(
    repo_path: &Path,
    pager_config: &PagerConfig,
    user_config: &UserConfig,
    diff_args: &[&str],
) -> GitResult<Vec<OsString>> {
    let ext_diff_cmd = pager_config.get_external_diff_command();
    let use_ext_diff = !ext_diff_cmd.is_empty();
    let use_ext_diff_git_config = pager_config.get_use_external_diff_git_config();
    let ignore_whitespace = user_config.git.ignore_whitespace_in_diff_view;

    let mut builder = GitCommandBuilder::new("diff")
        .config("diff.noprefix=false")
        .config_if(use_ext_diff, format!("diff.external={}", ext_diff_cmd))
        .arg_if_else(
            use_ext_diff || use_ext_diff_git_config,
            "--ext-diff",
            "--no-ext-diff",
        )
        .arg(["--submodule"])
        .arg([format!("--color={}", pager_config.get_color_arg())])
        .arg_if(ignore_whitespace, ["--ignore-all-space"])
        .arg([format!("--unified={}", user_config.git.diff_context_size)])
        .arg(diff_args)
        .dir(repo_path);

    Ok(builder.to_argv())
}

pub fn get_diff(repo_path: &Path, staged: bool, additional_args: &[&str]) -> GitResult<String> {
    let output = crate::git_builder_output(
        repo_path,
        GitCommandBuilder::new("diff")
            .config("diff.noprefix=false")
            .arg(["--no-ext-diff", "--no-color"])
            .arg_if(staged, ["--staged"])
            .dir(repo_path)
            .arg(additional_args),
    )?;

    if !output.status.success() {
        return Err(crate::GitError::OperationFailed {
            message: format!(
                "git diff failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn open_diff_tool_cmd_obj(
    repo_path: &Path,
    opts: &DiffToolCmdOptions,
) -> GitResult<Vec<OsString>> {
    let mut builder = GitCommandBuilder::new("difftool")
        .arg(["--no-prompt"])
        .arg_if(opts.is_directory, ["--dir-diff"])
        .arg_if(opts.staged, ["--cached"])
        .arg_if(!opts.from_commit.is_empty(), [&opts.from_commit])
        .arg_if(!opts.to_commit.is_empty(), [&opts.to_commit])
        .arg_if(opts.reverse, ["-R"])
        .arg(["--", opts.filepath.as_str()]);

    Ok(builder.to_argv())
}

pub fn diff_index_cmd_obj(repo_path: &Path, diff_args: &[&str]) -> GitResult<Vec<OsString>> {
    let builder = GitCommandBuilder::new("diff-index")
        .config("diff.noprefix=false")
        .arg(["--submodule", "--no-ext-diff", "--no-color", "--patch"])
        .arg(diff_args);

    Ok(builder.to_argv())
}

pub struct PagerConfig {
    external_diff_command: String,
    use_external_diff_git_config: bool,
    color_arg: String,
}

impl PagerConfig {
    pub fn get_external_diff_command(&self) -> &str {
        &self.external_diff_command
    }

    pub fn get_use_external_diff_git_config(&self) -> bool {
        self.use_external_diff_git_config
    }

    pub fn get_color_arg(&self) -> &str {
        &self.color_arg
    }
}

pub struct UserConfig {
    pub git: GitConfig,
}

pub struct GitConfig {
    pub ignore_whitespace_in_diff_view: bool,
    pub diff_context_size: u32,
}

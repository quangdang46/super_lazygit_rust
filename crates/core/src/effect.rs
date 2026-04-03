use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::state::{
    CommitHistoryMode, ComparisonTarget, DiffPresentation, GitFlowBranchType, JobId, MergeVariant,
    RepoId, ResetMode, SelectedHunk, StashMode,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    StartRepoScan,
    ConfigureWatcher {
        repo_ids: Vec<RepoId>,
    },
    ScheduleWatcherDebounce,
    RefreshRepoSummaries {
        repo_ids: Vec<RepoId>,
    },
    RefreshRepoSummary {
        repo_id: RepoId,
    },
    LoadRepoDetail {
        repo_id: RepoId,
        selected_path: Option<PathBuf>,
        diff_presentation: DiffPresentation,
        commit_ref: Option<String>,
        commit_history_mode: CommitHistoryMode,
        ignore_whitespace_in_diff: bool,
        diff_context_lines: u16,
        rename_similarity_threshold: u8,
    },
    LoadRepoDiff {
        repo_id: RepoId,
        comparison_target: Option<ComparisonTarget>,
        compare_with: Option<ComparisonTarget>,
        selected_path: Option<PathBuf>,
        diff_presentation: DiffPresentation,
        ignore_whitespace_in_diff: bool,
        diff_context_lines: u16,
        rename_similarity_threshold: u8,
    },
    FindBaseCommitForFixup {
        repo_id: RepoId,
        commit_oids: Vec<String>,
    },
    LoadCommitMessageForReword {
        repo_id: RepoId,
        commit: String,
        summary: String,
    },
    CheckBranchMerged {
        repo_id: RepoId,
        branch_name: String,
    },
    OpenEditor {
        cwd: PathBuf,
        target: PathBuf,
    },
    RunGitCommand(GitCommandRequest),
    RunShellCommand(ShellCommandRequest),
    RunPatchSelection(PatchSelectionJob),
    PersistCache,
    PersistConfig,
    ScheduleRender,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCommandRequest {
    pub job_id: JobId,
    pub repo_id: RepoId,
    pub command: GitCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CredentialStrategy {
    None,
    Prompt,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellCommandRequest {
    pub job_id: JobId,
    pub repo_id: RepoId,
    pub command: String,
    pub argv: Vec<String>,
    pub stdin: Option<String>,
    pub env: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub dont_log: bool,
    pub stream_output: bool,
    pub suppress_output_unless_error: bool,
    pub use_pty: bool,
    pub ignore_empty_error: bool,
    pub credential_strategy: CredentialStrategy,
    pub task: Option<String>,
    pub mutex_key: Option<String>,
}

impl ShellCommandRequest {
    #[must_use]
    pub fn new(job_id: JobId, repo_id: RepoId, command: impl Into<String>) -> Self {
        let command = command.into();
        Self {
            job_id,
            repo_id,
            argv: shell_command_args(&command),
            command,
            stdin: None,
            env: Vec::new(),
            working_dir: None,
            dont_log: false,
            stream_output: false,
            suppress_output_unless_error: false,
            use_pty: false,
            ignore_empty_error: false,
            credential_strategy: CredentialStrategy::None,
            task: None,
            mutex_key: None,
        }
    }

    #[must_use]
    pub fn from_args<I, S>(
        job_id: JobId,
        repo_id: RepoId,
        program: impl Into<String>,
        args: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let program = program.into();
        let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
        let mut argv = Vec::with_capacity(args.len() + 1);
        argv.push(program);
        argv.extend(args);
        let command = quote_command_args(argv.iter().map(String::as_str));
        Self {
            job_id,
            repo_id,
            command,
            argv,
            stdin: None,
            env: Vec::new(),
            working_dir: None,
            dont_log: false,
            stream_output: false,
            suppress_output_unless_error: false,
            use_pty: false,
            ignore_empty_error: false,
            credential_strategy: CredentialStrategy::None,
            task: None,
            mutex_key: None,
        }
    }

    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.argv
    }

    #[must_use]
    pub fn set_stdin(mut self, input: impl Into<String>) -> Self {
        self.stdin = Some(input.into());
        self
    }

    #[must_use]
    pub fn add_env_vars<I, S>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.env.extend(vars.into_iter().map(Into::into));
        self
    }

    #[must_use]
    pub fn env_vars(&self) -> &[String] {
        &self.env
    }

    #[must_use]
    pub fn set_wd(mut self, wd: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(wd.into());
        self
    }

    #[must_use]
    pub fn dont_log(mut self) -> Self {
        self.dont_log = true;
        self
    }

    #[must_use]
    pub fn should_log(&self) -> bool {
        !self.dont_log
    }

    #[must_use]
    pub fn stream_output(mut self) -> Self {
        self.stream_output = true;
        self
    }

    #[must_use]
    pub const fn should_stream_output(&self) -> bool {
        self.stream_output
    }

    #[must_use]
    pub fn suppress_output_unless_error(mut self) -> Self {
        self.suppress_output_unless_error = true;
        self
    }

    #[must_use]
    pub const fn should_suppress_output_unless_error(&self) -> bool {
        self.suppress_output_unless_error
    }

    #[must_use]
    pub fn use_pty(mut self) -> Self {
        self.use_pty = true;
        self
    }

    #[must_use]
    pub const fn should_use_pty(&self) -> bool {
        self.use_pty
    }

    #[must_use]
    pub fn ignore_empty_error(mut self) -> Self {
        self.ignore_empty_error = true;
        self
    }

    #[must_use]
    pub const fn should_ignore_empty_error(&self) -> bool {
        self.ignore_empty_error
    }

    #[must_use]
    pub fn with_mutex(mut self, mutex_key: impl Into<String>) -> Self {
        self.mutex_key = Some(mutex_key.into());
        self
    }

    #[must_use]
    pub fn mutex_key(&self) -> Option<&str> {
        self.mutex_key.as_deref()
    }

    #[must_use]
    pub fn prompt_on_credential_request(mut self, task: impl Into<String>) -> Self {
        self.credential_strategy = CredentialStrategy::Prompt;
        self.use_pty = true;
        self.task = Some(task.into());
        self
    }

    #[must_use]
    pub fn fail_on_credential_request(mut self) -> Self {
        self.credential_strategy = CredentialStrategy::Fail;
        self.use_pty = true;
        self
    }

    #[must_use]
    pub const fn credential_strategy(&self) -> CredentialStrategy {
        self.credential_strategy
    }

    #[must_use]
    pub fn task(&self) -> Option<&str> {
        self.task.as_deref()
    }
}

impl fmt::Display for ShellCommandRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&quote_command_args(self.argv.iter().map(String::as_str)))
    }
}

fn shell_command_args(command: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        vec!["cmd".to_string(), "/C".to_string(), command.to_string()]
    }

    #[cfg(not(windows))]
    {
        vec!["sh".to_string(), "-lc".to_string(), command.to_string()]
    }
}

fn quote_command_args<'a, I>(args: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    args.into_iter()
        .map(|arg| {
            if arg.contains(' ') {
                format!("\"{arg}\"")
            } else {
                arg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebaseStartMode {
    Interactive,
    Amend,
    Fixup,
    FixupWithMessage,
    ApplyFixups,
    Squash,
    Drop,
    MoveUp { adjacent_commit: String },
    MoveDown { adjacent_commit: String },
    Reword { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitCommand {
    StageSelection,
    UnstageSelection,
    StageFile {
        path: PathBuf,
    },
    DiscardFile {
        path: PathBuf,
    },
    UnstageFile {
        path: PathBuf,
    },
    CommitStaged {
        message: String,
    },
    CommitStagedNoVerify {
        message: String,
    },
    CommitStagedWithEditor,
    AmendHead {
        message: Option<String>,
    },
    StartBisect {
        commit: String,
        term: String,
    },
    MarkBisect {
        commit: String,
        term: String,
    },
    SkipBisect {
        commit: String,
    },
    ResetBisect,
    CreateFixupCommit {
        commit: String,
    },
    CreateAmendCommit {
        original_subject: String,
        message: String,
        include_file_changes: bool,
    },
    AmendCommitAttributes {
        commit: String,
        reset_author: bool,
        co_author: Option<String>,
    },
    RewordCommitWithEditor {
        commit: String,
    },
    StartCommitRebase {
        commit: String,
        mode: RebaseStartMode,
    },
    CherryPickCommit {
        commit: String,
    },
    RevertCommit {
        commit: String,
    },
    ResetToCommit {
        mode: ResetMode,
        target: String,
    },
    RestoreSnapshot {
        target: String,
    },
    ContinueRebase,
    AbortRebase,
    SkipRebase,
    CreateBranch {
        branch_name: String,
    },
    StartGitFlow {
        branch_type: GitFlowBranchType,
        name: String,
    },
    AddRemote {
        remote_name: String,
        remote_url: String,
    },
    CreateTag {
        tag_name: String,
    },
    CreateTagFromCommit {
        tag_name: String,
        commit: String,
    },
    CreateBranchFromCommit {
        branch_name: String,
        commit: String,
    },
    CreateBranchFromRef {
        branch_name: String,
        start_point: String,
        track: bool,
    },
    FinishGitFlow {
        branch_name: String,
    },
    CheckoutBranch {
        branch_ref: String,
    },
    CheckoutRemoteBranch {
        remote_branch_ref: String,
        local_branch_name: String,
    },
    CheckoutTag {
        tag_name: String,
    },
    CheckoutCommit {
        commit: String,
    },
    CheckoutCommitFile {
        commit: String,
        path: PathBuf,
    },
    RenameBranch {
        branch_name: String,
        new_name: String,
    },
    EditRemote {
        current_name: String,
        new_name: String,
        remote_url: String,
    },
    RenameStash {
        stash_ref: String,
        message: String,
    },
    CreateBranchFromStash {
        stash_ref: String,
        branch_name: String,
    },
    DeleteBranch {
        branch_name: String,
        force: bool,
    },
    RemoveRemote {
        remote_name: String,
    },
    DeleteRemoteBranch {
        remote_name: String,
        branch_name: String,
    },
    DeleteTag {
        tag_name: String,
    },
    PushTag {
        remote_name: String,
        tag_name: String,
    },
    CreateStash {
        message: Option<String>,
        mode: StashMode,
    },
    ApplyStash {
        stash_ref: String,
    },
    PopStash {
        stash_ref: String,
    },
    DropStash {
        stash_ref: String,
    },
    CreateWorktree {
        path: PathBuf,
        base_ref: String,
        branch: Option<String>,
        detach: bool,
    },
    DetachWorktree {
        path: PathBuf,
    },
    RemoveWorktree {
        path: PathBuf,
        force: bool,
    },
    AddSubmodule {
        path: PathBuf,
        url: String,
    },
    EditSubmoduleUrl {
        name: String,
        path: PathBuf,
        url: String,
    },
    InitSubmodule {
        path: PathBuf,
    },
    UpdateSubmodule {
        path: PathBuf,
    },
    InitAllSubmodules,
    UpdateAllSubmodules,
    UpdateAllSubmodulesRecursively,
    DeinitAllSubmodules,
    RemoveSubmodule {
        path: PathBuf,
    },
    SetBranchUpstream {
        branch_name: String,
        upstream_ref: String,
    },
    UnsetBranchUpstream {
        branch_name: String,
    },
    FastForwardCurrentBranchFromUpstream {
        upstream_ref: String,
    },
    ForceCheckoutRef {
        target_ref: String,
    },
    MergeRefIntoCurrent {
        target_ref: String,
        variant: MergeVariant,
    },
    RebaseCurrentOntoRef {
        target_ref: String,
    },
    FetchRemote {
        remote_name: String,
    },
    UpdateBranchRefs {
        update_commands: String,
    },
    FetchSelectedRepo,
    PullCurrentBranch,
    PushCurrentBranch,
    NukeWorkingTree,
    RefreshSelectedRepo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchSelectionJob {
    pub job_id: JobId,
    pub repo_id: RepoId,
    pub path: PathBuf,
    pub mode: PatchApplicationMode,
    pub hunks: Vec<SelectedHunk>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchApplicationMode {
    Stage,
    Unstage,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{JobId, RepoId};

    #[test]
    fn shell_command_to_string_quotes_args_with_spaces() {
        let scenarios = [
            (
                vec![
                    "git".to_string(),
                    "push".to_string(),
                    "myfile.txt".to_string(),
                ],
                "git push myfile.txt",
            ),
            (
                vec![
                    "git".to_string(),
                    "push".to_string(),
                    "my file.txt".to_string(),
                ],
                "git push \"my file.txt\"",
            ),
        ];

        for (argv, expected) in scenarios {
            let request = ShellCommandRequest::from_args(
                JobId::new("shell:repo:quoted"),
                RepoId::new("/tmp/repo"),
                argv[0].clone(),
                argv[1..].iter().cloned(),
            );
            assert_eq!(request.to_string(), expected);
        }
    }

    #[test]
    fn shell_command_clone_preserves_task_and_flags() {
        let request = ShellCommandRequest::from_args(
            JobId::new("shell:repo:clone"),
            RepoId::new("/tmp/repo"),
            "git",
            ["fetch"],
        )
        .prompt_on_credential_request("credential-task")
        .with_mutex("fetch-mutex")
        .ignore_empty_error();
        let clone = request.clone();

        assert_ne!(std::ptr::from_ref(&request), std::ptr::from_ref(&clone));
        assert_eq!(clone.task(), Some("credential-task"));
        assert_eq!(clone.mutex_key(), Some("fetch-mutex"));
        assert_eq!(clone.credential_strategy(), CredentialStrategy::Prompt);
        assert!(clone.should_use_pty());
        assert!(clone.should_ignore_empty_error());
    }

    #[test]
    fn shell_command_builder_helpers_match_cmd_obj_mutator_semantics() {
        let request = ShellCommandRequest::from_args(
            JobId::new("shell:repo:builder"),
            RepoId::new("/tmp/repo"),
            "git",
            ["status", "--short"],
        )
        .set_stdin("input")
        .add_env_vars(["A=1", "B=two"])
        .set_wd("/tmp/custom")
        .dont_log()
        .stream_output()
        .suppress_output_unless_error();

        assert_eq!(request.args(), &["git", "status", "--short"]);
        assert_eq!(request.stdin.as_deref(), Some("input"));
        assert_eq!(request.env_vars(), &["A=1", "B=two"]);
        assert_eq!(
            request.working_dir.as_deref(),
            Some(std::path::Path::new("/tmp/custom"))
        );
        assert!(!request.should_log());
        assert!(request.should_stream_output());
        assert!(request.should_suppress_output_unless_error());
    }
}

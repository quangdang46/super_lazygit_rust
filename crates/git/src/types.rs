use std::path::PathBuf;

use super_lazygit_core::{
    CommitHistoryMode, ComparisonTarget, DiffHunk, DiffPresentation, GitCommand,
    PatchApplicationMode, RepoId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceScanRequest {
    pub root: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceScanResult {
    pub root: Option<PathBuf>,
    pub repo_ids: Vec<RepoId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSummaryRequest {
    pub repo_id: RepoId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoDetailRequest {
    pub repo_id: RepoId,
    pub selected_path: Option<PathBuf>,
    pub diff_presentation: DiffPresentation,
    pub commit_ref: Option<String>,
    pub commit_history_mode: CommitHistoryMode,
    pub show_branch_heads: bool,
    pub ignore_whitespace_in_diff: bool,
    pub diff_context_lines: u16,
    pub rename_similarity_threshold: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchMergeStatusRequest {
    pub repo_id: RepoId,
    pub branch_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRequest {
    pub repo_id: RepoId,
    pub comparison_target: Option<ComparisonTarget>,
    pub compare_with: Option<ComparisonTarget>,
    pub selected_path: Option<PathBuf>,
    pub context_lines: u16,
    pub rename_similarity_threshold: u8,
    pub ignore_whitespace_in_diff: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixupBaseCommitRequest {
    pub commit_message: String,
    pub commit_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixupBaseCommitOutcome {
    AlreadyIdeal,
    SquashedIntoFixupCommit(String),
    FixupCommitCreated(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchSelectionRequest {
    pub patch: String,
    pub patch_application_mode: PatchApplicationMode,
    pub hunks_to_apply: Vec<DiffHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommandOutcome {
    pub action: GitCommand,
    pub result: Result<(), String>,
}

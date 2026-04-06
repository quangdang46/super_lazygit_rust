use serde::{Deserialize, Serialize};

use crate::action::Action;
use crate::state::{DiffModel, JobId, RepoDetail, RepoId, RepoSummary, Timestamp};

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Input(InputEvent),
    Action(Action),
    Worker(WorkerEvent),
    Watcher(WatcherEvent),
    Timer(TimerEvent),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputEvent {
    KeyPressed(KeyPress),
    Resize { width: u16, height: u16 },
    Paste(String),
    MouseLeft { column: u16, row: u16 },
    MouseDoubleLeft { column: u16, row: u16 },
    MouseWheelUp { column: u16, row: u16 },
    MouseWheelDown { column: u16, row: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyPress {
    pub key: String,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerEvent {
    RepoScanFailed {
        root: Option<std::path::PathBuf>,
        error: String,
    },
    RepoScanCompleted {
        root: Option<std::path::PathBuf>,
        repo_ids: Vec<RepoId>,
        scanned_at: Timestamp,
    },
    RepoSummaryUpdated {
        job_id: JobId,
        summary: RepoSummary,
    },
    RepoSummaryRefreshStarted {
        job_id: JobId,
        repo_id: RepoId,
    },
    RepoSummaryRefreshFailed {
        job_id: JobId,
        repo_id: RepoId,
        error: String,
    },
    RepoDetailLoaded {
        repo_id: RepoId,
        detail: RepoDetail,
    },
    RepoDiffLoaded {
        repo_id: RepoId,
        diff: DiffModel,
    },
    RepoDiffLoadFailed {
        repo_id: RepoId,
        error: String,
    },
    FixupBaseCommitFound {
        repo_id: RepoId,
        hashes: Vec<String>,
        has_staged_changes: bool,
        warn_about_added_lines: bool,
    },
    FixupBaseCommitLookupFailed {
        repo_id: RepoId,
        error: String,
    },
    CommitMessageForRewordLoaded {
        repo_id: RepoId,
        commit: String,
        summary: String,
        message: String,
    },
    CommitMessageForRewordLoadFailed {
        repo_id: RepoId,
        error: String,
    },
    BranchMergeCheckCompleted {
        repo_id: RepoId,
        branch_name: String,
        merged: bool,
    },
    BranchMergeCheckFailed {
        repo_id: RepoId,
        branch_name: String,
        error: String,
    },
    GitOperationStarted {
        job_id: JobId,
        repo_id: RepoId,
        summary: String,
    },
    GitOperationCompleted {
        job_id: JobId,
        repo_id: RepoId,
        summary: String,
    },
    GitOperationFailed {
        job_id: JobId,
        repo_id: RepoId,
        error: String,
    },
    EditorLaunchFailed {
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatcherEvent {
    RepoInvalidated { repo_id: RepoId },
    WatcherDegraded { message: String },
    WatcherRecovered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimerEvent {
    PeriodicRefreshTick,
    PeriodicFetchTick,
    WatcherDebounceFlush,
    ToastExpiryTick { now: Timestamp },
}

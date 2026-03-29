use serde::{Deserialize, Serialize};

use crate::action::Action;
use crate::state::{JobId, RepoDetail, RepoId, RepoSummary, Timestamp};

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyPress {
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerEvent {
    RepoScanCompleted {
        root: Option<std::path::PathBuf>,
        repo_ids: Vec<RepoId>,
        scanned_at: Timestamp,
    },
    RepoSummaryUpdated {
        summary: RepoSummary,
    },
    RepoDetailLoaded {
        repo_id: RepoId,
        detail: RepoDetail,
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
    ToastExpiryTick { now: Timestamp },
}

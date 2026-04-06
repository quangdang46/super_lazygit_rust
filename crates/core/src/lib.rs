pub mod action;
pub mod diagnostics;
pub mod effect;
pub mod event;
mod hosting_service;
pub mod lines;
pub mod reducer;
pub mod state;
pub mod version_number;

pub use action::Action;
pub use diagnostics::{
    Diagnostics, DiagnosticsSnapshot, GitTiming, RenderTiming, ScanTiming, TimingSample,
    WatcherEvent, WatcherEventKind,
};
pub use effect::{
    CredentialStrategy, Effect, GitCommand, GitCommandRequest, PatchApplicationMode,
    PatchSelectionJob, RebaseStartMode, ShellCommandRequest,
};
pub use event::{
    Event, InputEvent, KeyPress, TimerEvent, WatcherEvent as AppWatcherEvent, WorkerEvent,
};
pub use lines::*;
pub use reducer::{reduce, ReduceResult};
pub use state::*;

use std::collections::VecDeque;
use std::path::PathBuf;

use super_lazygit_core::{AppWatcherEvent, RepoId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchRegistration {
    pub repo_id: RepoId,
    pub path: PathBuf,
}

pub trait WatcherBackend: std::fmt::Debug {
    fn configure(&mut self, registrations: Vec<WatchRegistration>) -> Result<usize, String>;

    fn drain(&mut self) -> Vec<AppWatcherEvent>;
}

#[derive(Debug, Default)]
pub struct NullWatcherBackend {
    pending_events: VecDeque<AppWatcherEvent>,
}

impl WatcherBackend for NullWatcherBackend {
    fn configure(&mut self, registrations: Vec<WatchRegistration>) -> Result<usize, String> {
        Ok(registrations.len())
    }

    fn drain(&mut self) -> Vec<AppWatcherEvent> {
        self.pending_events.drain(..).collect()
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub struct ScriptedWatcherHandle(std::sync::Arc<std::sync::Mutex<ScriptedWatcherState>>);

#[cfg(test)]
#[derive(Debug, Default)]
struct ScriptedWatcherState {
    registrations: Vec<WatchRegistration>,
    pending_events: VecDeque<AppWatcherEvent>,
    next_configure_error: Option<String>,
}

#[cfg(test)]
impl ScriptedWatcherHandle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_event(&self, event: AppWatcherEvent) {
        self.0
            .lock()
            .expect("scripted watcher state")
            .pending_events
            .push_back(event);
    }

    pub fn set_configure_error(&self, message: impl Into<String>) {
        self.0
            .lock()
            .expect("scripted watcher state")
            .next_configure_error = Some(message.into());
    }

    #[must_use]
    pub fn registrations(&self) -> Vec<WatchRegistration> {
        self.0
            .lock()
            .expect("scripted watcher state")
            .registrations
            .clone()
    }
}

#[cfg(test)]
#[derive(Debug)]
pub struct ScriptedWatcherBackend {
    handle: ScriptedWatcherHandle,
}

#[cfg(test)]
impl ScriptedWatcherBackend {
    #[must_use]
    pub fn new(handle: ScriptedWatcherHandle) -> Self {
        Self { handle }
    }
}

#[cfg(test)]
impl WatcherBackend for ScriptedWatcherBackend {
    fn configure(&mut self, registrations: Vec<WatchRegistration>) -> Result<usize, String> {
        let mut state = self.handle.0.lock().expect("scripted watcher state");
        if let Some(error) = state.next_configure_error.take() {
            return Err(error);
        }

        let count = registrations.len();
        state.registrations = registrations;
        Ok(count)
    }

    fn drain(&mut self) -> Vec<AppWatcherEvent> {
        self.handle
            .0
            .lock()
            .expect("scripted watcher state")
            .pending_events
            .drain(..)
            .collect()
    }
}

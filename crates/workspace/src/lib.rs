use std::path::PathBuf;
use std::time::Instant;

use super_lazygit_core::{Diagnostics, DiagnosticsSnapshot, WatcherEventKind};

#[derive(Debug, Clone, Default)]
pub struct WorkspaceRegistry {
    root: Option<PathBuf>,
    diagnostics: Diagnostics,
}

impl WorkspaceRegistry {
    #[must_use]
    pub fn new(root: Option<PathBuf>) -> Self {
        let mut registry = Self {
            root,
            diagnostics: Diagnostics::default(),
        };
        registry.record_scan(
            "workspace.registry.init",
            usize::from(registry.root.is_some()),
        );
        registry
    }

    #[must_use]
    pub fn root(&self) -> Option<&PathBuf> {
        self.root.as_ref()
    }

    pub fn record_scan(&mut self, scope: impl Into<String>, item_count: usize) {
        let started_at = Instant::now();
        self.diagnostics
            .record_scan(scope, started_at.elapsed(), item_count);
    }

    pub fn record_watcher_refresh(&mut self, path_count: usize) {
        let kind = if path_count == 0 {
            WatcherEventKind::Dropped
        } else if path_count > 1 {
            WatcherEventKind::Burst
        } else {
            WatcherEventKind::Refreshed
        };
        self.diagnostics.record_watcher_event(kind, path_count);
    }

    pub fn mark_watcher_started(&mut self, path_count: usize) {
        self.diagnostics
            .record_watcher_event(WatcherEventKind::Created, path_count);
    }

    #[must_use]
    pub fn diagnostics(&self) -> DiagnosticsSnapshot {
        self.diagnostics.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_registry_tracks_scan_and_watcher_activity() {
        let mut workspace = WorkspaceRegistry::new(Some(PathBuf::from("/tmp/repo")));

        workspace.mark_watcher_started(1);
        workspace.record_watcher_refresh(3);

        let snapshot = workspace.diagnostics();
        assert_eq!(snapshot.scans.len(), 1);
        assert_eq!(snapshot.scans[0].scope, "workspace.registry.init");
        assert_eq!(snapshot.watcher_events.len(), 2);
        assert_eq!(snapshot.watcher_churn_count(), 2);
    }
}

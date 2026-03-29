use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimingSample {
    pub name: String,
    pub elapsed: Duration,
}

impl TimingSample {
    #[must_use]
    pub fn new(name: impl Into<String>, elapsed: Duration) -> Self {
        Self {
            name: name.into(),
            elapsed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanTiming {
    pub scope: String,
    pub elapsed: Duration,
    pub item_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitTiming {
    pub operation: String,
    pub elapsed: Duration,
    pub success: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatcherEventKind {
    Created,
    Refreshed,
    Burst,
    Dropped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatcherEvent {
    pub kind: WatcherEventKind,
    pub path_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTiming {
    pub surface: String,
    pub elapsed: Duration,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diagnostics {
    startup: Vec<TimingSample>,
    scans: Vec<ScanTiming>,
    git_operations: Vec<GitTiming>,
    watcher_events: Vec<WatcherEvent>,
    renders: Vec<RenderTiming>,
}

impl Diagnostics {
    pub fn record_startup_stage(&mut self, stage: impl Into<String>, elapsed: Duration) {
        self.startup.push(TimingSample::new(stage, elapsed));
    }

    pub fn record_scan(&mut self, scope: impl Into<String>, elapsed: Duration, item_count: usize) {
        self.scans.push(ScanTiming {
            scope: scope.into(),
            elapsed,
            item_count,
        });
    }

    pub fn record_git_operation(
        &mut self,
        operation: impl Into<String>,
        elapsed: Duration,
        success: bool,
    ) {
        self.git_operations.push(GitTiming {
            operation: operation.into(),
            elapsed,
            success,
        });
    }

    pub fn record_watcher_event(&mut self, kind: WatcherEventKind, path_count: usize) {
        self.watcher_events.push(WatcherEvent { kind, path_count });
    }

    pub fn record_render(&mut self, surface: impl Into<String>, elapsed: Duration) {
        self.renders.push(RenderTiming {
            surface: surface.into(),
            elapsed,
        });
    }

    pub fn extend_snapshot(&mut self, snapshot: DiagnosticsSnapshot) {
        self.startup.extend(snapshot.startup);
        self.scans.extend(snapshot.scans);
        self.git_operations.extend(snapshot.git_operations);
        self.watcher_events.extend(snapshot.watcher_events);
        self.renders.extend(snapshot.renders);
    }

    #[must_use]
    pub fn snapshot(&self) -> DiagnosticsSnapshot {
        DiagnosticsSnapshot {
            startup_total: self.startup.iter().map(|stage| stage.elapsed).sum(),
            startup: self.startup.clone(),
            scans: self.scans.clone(),
            git_operations: self.git_operations.clone(),
            watcher_events: self.watcher_events.clone(),
            renders: self.renders.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsSnapshot {
    pub startup_total: Duration,
    pub startup: Vec<TimingSample>,
    pub scans: Vec<ScanTiming>,
    pub git_operations: Vec<GitTiming>,
    pub watcher_events: Vec<WatcherEvent>,
    pub renders: Vec<RenderTiming>,
}

impl DiagnosticsSnapshot {
    #[must_use]
    pub fn slowest_render(&self) -> Option<&RenderTiming> {
        self.renders.iter().max_by_key(|render| render.elapsed)
    }

    #[must_use]
    pub fn watcher_churn_count(&self) -> usize {
        self.watcher_events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_snapshot_accumulates_samples() {
        let mut diagnostics = Diagnostics::default();

        diagnostics.record_startup_stage("bootstrap", Duration::from_millis(4));
        diagnostics.record_scan("workspace.initial", Duration::from_millis(6), 3);
        diagnostics.record_git_operation("status", Duration::from_millis(9), true);
        diagnostics.record_watcher_event(WatcherEventKind::Created, 2);
        diagnostics.record_render("root", Duration::from_millis(5));

        let snapshot = diagnostics.snapshot();

        assert_eq!(snapshot.startup_total, Duration::from_millis(4));
        assert_eq!(snapshot.scans[0].item_count, 3);
        assert_eq!(snapshot.git_operations[0].operation, "status");
        assert_eq!(snapshot.watcher_churn_count(), 1);
        assert_eq!(
            snapshot
                .slowest_render()
                .map(|render| render.surface.as_str()),
            Some("root")
        );
    }

    #[test]
    fn diagnostics_can_merge_snapshots() {
        let mut aggregate = Diagnostics::default();
        let mut child = Diagnostics::default();
        child.record_render("sidebar", Duration::from_millis(3));

        aggregate.extend_snapshot(child.snapshot());

        assert_eq!(aggregate.snapshot().renders.len(), 1);
    }
}

use std::time::Instant;

use super_lazygit_core::{Diagnostics, DiagnosticsSnapshot};

#[derive(Debug, Clone, Default)]
pub struct GitFacade {
    diagnostics: Diagnostics,
}

impl GitFacade {
    pub fn record_operation(&mut self, operation: impl Into<String>, success: bool) {
        let started_at = Instant::now();
        self.diagnostics
            .record_git_operation(operation, started_at.elapsed(), success);
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
    fn git_facade_records_operation_latency() {
        let mut git = GitFacade::default();

        git.record_operation("status", true);

        let snapshot = git.diagnostics();
        assert_eq!(snapshot.git_operations.len(), 1);
        assert_eq!(snapshot.git_operations[0].operation, "status");
        assert!(snapshot.git_operations[0].success);
    }
}

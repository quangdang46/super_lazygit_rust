use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use super_lazygit_core::{
    Diagnostics, DiagnosticsSnapshot, RepoId, RepoSummary, Timestamp, WatcherEventKind,
    WatcherFreshness, WorkspaceState,
};

const CACHE_DIR: &str = ".super-lazygit";
const CACHE_FILE: &str = "workspace-cache.json";
const CACHE_SCHEMA_VERSION: u32 = 1;
const STALE_CACHE_AGE_SECS: u64 = 300;

#[derive(Debug, Clone, Default)]
pub struct WorkspaceRegistry {
    root: Option<PathBuf>,
    repo_paths: BTreeMap<RepoId, PathBuf>,
    diagnostics: Diagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct WorkspaceCacheDocument {
    schema_version: u32,
    root_fingerprint: String,
    saved_at: Timestamp,
    workspace: WorkspaceState,
    repo_paths: BTreeMap<RepoId, PathBuf>,
}

impl WorkspaceRegistry {
    #[must_use]
    pub fn new(root: Option<PathBuf>) -> Self {
        let mut registry = Self {
            root: root.map(normalize_pathbuf),
            repo_paths: BTreeMap::new(),
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

    pub fn register_scan(&mut self, root: Option<PathBuf>, repo_ids: &[RepoId]) -> Vec<RepoId> {
        if let Some(root) = root {
            self.root = Some(normalize_pathbuf(root));
        }

        let mut normalized_ids = Vec::with_capacity(repo_ids.len());
        let mut normalized_paths = BTreeMap::new();

        for repo_id in repo_ids {
            let normalized_path = normalize_path(Path::new(&repo_id.0));
            let normalized_repo_id = repo_id_from_path(&normalized_path);
            if normalized_paths.contains_key(&normalized_repo_id) {
                continue;
            }

            normalized_ids.push(normalized_repo_id.clone());
            normalized_paths.insert(normalized_repo_id, normalized_path);
        }

        self.repo_paths = normalized_paths;
        normalized_ids
    }

    pub fn register_summary(&mut self, mut summary: RepoSummary) -> RepoSummary {
        let normalized_path = normalize_path(&summary.real_path);
        let normalized_repo_id = repo_id_from_path(&normalized_path);
        summary.repo_id = normalized_repo_id.clone();
        summary.real_path = normalized_path.clone();
        if summary.display_path.is_empty() {
            summary.display_path = normalized_path.display().to_string();
        }
        self.repo_paths.insert(normalized_repo_id, normalized_path);
        summary
    }

    #[must_use]
    pub fn repo_path(&self, repo_id: &RepoId) -> Option<&PathBuf> {
        self.repo_paths.get(repo_id)
    }

    pub fn load_cache(&mut self) -> Option<WorkspaceState> {
        let cache_path = self.cache_path()?;
        let contents = fs::read_to_string(cache_path).ok()?;
        let cache: WorkspaceCacheDocument = serde_json::from_str(&contents).ok()?;
        if cache.schema_version != CACHE_SCHEMA_VERSION {
            return None;
        }
        if cache.root_fingerprint != self.root_fingerprint() {
            return None;
        }

        self.repo_paths = cache.repo_paths.clone();
        let mut workspace = cache.workspace;
        workspace.current_root = self.root.clone();
        workspace.discovered_repo_ids = workspace
            .discovered_repo_ids
            .iter()
            .map(|repo_id| {
                self.repo_path(repo_id)
                    .cloned()
                    .map(|path| repo_id_from_path(&path))
                    .unwrap_or_else(|| repo_id.clone())
            })
            .collect();
        workspace.repo_summaries = workspace
            .repo_summaries
            .into_values()
            .map(|summary| {
                let normalized = self.register_summary(summary);
                (normalized.repo_id.clone(), normalized)
            })
            .collect();

        if workspace
            .selected_repo_id
            .as_ref()
            .is_some_and(|repo_id| !workspace.discovered_repo_ids.contains(repo_id))
        {
            workspace.selected_repo_id = None;
        }
        if workspace.selected_repo_id.is_none() {
            workspace.selected_repo_id = workspace.discovered_repo_ids.first().cloned();
        }

        if is_cache_stale(cache.saved_at) {
            mark_workspace_stale(&mut workspace);
        }

        Some(workspace)
    }

    pub fn persist_cache(&self, workspace: &WorkspaceState) -> io::Result<()> {
        let Some(cache_path) = self.cache_path() else {
            return Ok(());
        };
        let Some(cache_dir) = cache_path.parent() else {
            return Ok(());
        };

        fs::create_dir_all(cache_dir)?;

        let repo_paths = self
            .repo_paths
            .iter()
            .filter(|(repo_id, _)| workspace.discovered_repo_ids.contains(repo_id))
            .map(|(repo_id, path)| (repo_id.clone(), path.clone()))
            .collect();

        let cache = WorkspaceCacheDocument {
            schema_version: CACHE_SCHEMA_VERSION,
            root_fingerprint: self.root_fingerprint(),
            saved_at: current_timestamp(),
            workspace: workspace.clone(),
            repo_paths,
        };
        let encoded = serde_json::to_string_pretty(&cache)
            .map_err(|error| io::Error::other(error.to_string()))?;
        fs::write(cache_path, encoded)
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

    fn cache_path(&self) -> Option<PathBuf> {
        self.root
            .as_ref()
            .map(|root| root.join(CACHE_DIR).join(CACHE_FILE))
    }

    fn root_fingerprint(&self) -> String {
        self.root
            .as_ref()
            .map(|root| root.display().to_string())
            .unwrap_or_else(|| String::from("no-workspace-root"))
    }
}

pub fn for_each_line_in_file(
    path: impl AsRef<Path>,
    callback: impl FnMut(String, usize),
) -> io::Result<()> {
    let file = fs::File::open(path)?;
    for_each_line_in_stream(file, callback);
    Ok(())
}

pub fn for_each_line_in_stream(reader: impl Read, mut callback: impl FnMut(String, usize)) {
    let mut buffered_reader = BufReader::new(reader);
    for index in 0usize.. {
        let mut line = String::new();
        let bytes_read = buffered_reader
            .read_line(&mut line)
            .expect("reading in-memory line stream should not fail");
        if bytes_read == 0 {
            break;
        }
        callback(line, index);
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| normalize_pathbuf(path.to_path_buf()))
}

fn normalize_pathbuf(path: PathBuf) -> PathBuf {
    path.components().collect()
}

fn repo_id_from_path(path: &Path) -> RepoId {
    RepoId::new(path.display().to_string())
}

fn current_timestamp() -> Timestamp {
    Timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
}

fn is_cache_stale(saved_at: Timestamp) -> bool {
    current_timestamp().0.saturating_sub(saved_at.0) > STALE_CACHE_AGE_SECS
}

fn mark_workspace_stale(workspace: &mut WorkspaceState) {
    for summary in workspace.repo_summaries.values_mut() {
        summary.watcher_freshness = WatcherFreshness::Stale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Cursor;

    use tempfile::TempDir;

    fn cache_fixture(
        summary_freshness: WatcherFreshness,
    ) -> (TempDir, WorkspaceRegistry, WorkspaceState) {
        let root = tempfile::tempdir().expect("workspace root");
        let repo_path = root.path().join("repo-a");
        fs::create_dir_all(repo_path.join(".git")).expect("repo fixture");

        let mut registry = WorkspaceRegistry::new(Some(root.path().to_path_buf()));
        let repo_ids = registry.register_scan(
            Some(root.path().to_path_buf()),
            &[RepoId::new(repo_path.display().to_string())],
        );
        let repo_id = repo_ids[0].clone();
        let summary = registry.register_summary(RepoSummary {
            repo_id: repo_id.clone(),
            display_name: String::from("repo-a"),
            real_path: repo_path.clone(),
            display_path: repo_path.display().to_string(),
            watcher_freshness: summary_freshness,
            last_refresh_at: Some(Timestamp(10)),
            ..RepoSummary::default()
        });

        let workspace = WorkspaceState {
            current_root: Some(root.path().to_path_buf()),
            discovered_repo_ids: vec![repo_id.clone()],
            repo_summaries: BTreeMap::from([(repo_id.clone(), summary)]),
            selected_repo_id: Some(repo_id),
            scan_status: super_lazygit_core::ScanStatus::Complete { scanned_repos: 1 },
            last_full_refresh_at: Some(Timestamp(10)),
            ..WorkspaceState::default()
        };

        (root, registry, workspace)
    }

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

    #[test]
    fn register_scan_keeps_repo_ids_stable_across_orderings() {
        let root = tempfile::tempdir().expect("workspace root");
        let repo_a = root.path().join("repo-a");
        let repo_b = root.path().join("repo-b");
        fs::create_dir_all(repo_a.join(".git")).expect("repo a");
        fs::create_dir_all(repo_b.join(".git")).expect("repo b");

        let mut registry = WorkspaceRegistry::new(Some(root.path().to_path_buf()));
        let first = registry.register_scan(
            Some(root.path().to_path_buf()),
            &[
                RepoId::new(repo_a.display().to_string()),
                RepoId::new(repo_b.display().to_string()),
            ],
        );
        let second = registry.register_scan(
            Some(root.path().to_path_buf()),
            &[
                RepoId::new(repo_b.display().to_string()),
                RepoId::new(repo_a.display().to_string()),
            ],
        );

        let mut first_sorted = first;
        first_sorted.sort();
        let mut second_sorted = second;
        second_sorted.sort();
        assert_eq!(first_sorted, second_sorted);
    }

    #[test]
    fn load_cache_returns_none_when_cache_is_missing() {
        let root = tempfile::tempdir().expect("workspace root");
        let mut registry = WorkspaceRegistry::new(Some(root.path().to_path_buf()));

        assert!(registry.load_cache().is_none());
    }

    #[test]
    fn cache_round_trip_hydrates_workspace_state_and_repo_registry() {
        let (_root, registry, workspace) = cache_fixture(WatcherFreshness::Fresh);
        registry.persist_cache(&workspace).expect("persist cache");

        let mut hydrated = WorkspaceRegistry::new(registry.root().cloned());
        let restored = hydrated.load_cache().expect("cache hit");
        let repo_id = restored
            .discovered_repo_ids
            .first()
            .expect("restored repo id")
            .clone();

        assert_eq!(restored.selected_repo_id, Some(repo_id.clone()));
        assert!(restored.repo_summaries.contains_key(&repo_id));
        assert_eq!(
            hydrated.repo_path(&repo_id),
            restored
                .repo_summaries
                .get(&repo_id)
                .map(|summary| &summary.real_path)
        );
    }

    #[test]
    fn load_cache_marks_old_summaries_as_stale() {
        let (_root, registry, workspace) = cache_fixture(WatcherFreshness::Fresh);
        registry.persist_cache(&workspace).expect("persist cache");

        let cache_path = registry.cache_path().expect("cache path");
        let mut cached: WorkspaceCacheDocument =
            serde_json::from_str(&fs::read_to_string(&cache_path).expect("read cache"))
                .expect("decode cache");
        cached.saved_at = Timestamp(0);
        fs::write(
            &cache_path,
            serde_json::to_string_pretty(&cached).expect("encode cache"),
        )
        .expect("write stale cache");

        let mut hydrated = WorkspaceRegistry::new(registry.root().cloned());
        let restored = hydrated.load_cache().expect("cache hit");
        let summary = restored
            .repo_summaries
            .values()
            .next()
            .expect("cached summary");
        assert_eq!(summary.watcher_freshness, WatcherFreshness::Stale);
    }

    #[test]
    fn load_cache_rejects_root_fingerprint_mismatch() {
        let (_root, registry, workspace) = cache_fixture(WatcherFreshness::Fresh);
        registry.persist_cache(&workspace).expect("persist cache");

        let cache_path = registry.cache_path().expect("cache path");
        let mut cached: WorkspaceCacheDocument =
            serde_json::from_str(&fs::read_to_string(&cache_path).expect("read cache"))
                .expect("decode cache");
        cached.root_fingerprint = String::from("different-root");
        fs::write(
            &cache_path,
            serde_json::to_string_pretty(&cached).expect("encode cache"),
        )
        .expect("write mismatched cache");

        let mut hydrated = WorkspaceRegistry::new(registry.root().cloned());
        assert!(hydrated.load_cache().is_none());
    }

    #[test]
    fn for_each_line_in_stream_matches_upstream_cases() {
        let scenarios = [
            ("", Vec::<String>::new()),
            ("abc\n", vec!["abc\n".to_string()]),
            ("abc", vec!["abc".to_string()]),
            ("abc\ndef\n", vec!["abc\n".to_string(), "def\n".to_string()]),
            (
                "abc\n\ndef\n",
                vec!["abc\n".to_string(), "\n".to_string(), "def\n".to_string()],
            ),
            (
                "abc\ndef\nghi",
                vec!["abc\n".to_string(), "def\n".to_string(), "ghi".to_string()],
            ),
        ];

        for (input, expected_lines) in scenarios {
            let mut lines = Vec::new();
            let mut indices = Vec::new();
            for_each_line_in_stream(Cursor::new(input), |line, index| {
                lines.push(line);
                indices.push(index);
            });
            assert_eq!(lines, expected_lines);
            assert_eq!(indices, (0..expected_lines.len()).collect::<Vec<_>>());
        }
    }

    #[test]
    fn for_each_line_in_file_preserves_newlines_and_indices() {
        let root = tempfile::tempdir().expect("workspace root");
        let path = root.path().join("lines.txt");
        fs::write(&path, "abc\n\ndef\n").expect("write fixture");

        let mut lines = Vec::new();
        let mut indices = Vec::new();
        for_each_line_in_file(&path, |line, index| {
            lines.push(line);
            indices.push(index);
        })
        .expect("read file");

        assert_eq!(lines, vec!["abc\n", "\n", "def\n"]);
        assert_eq!(indices, vec![0, 1, 2]);
    }
}

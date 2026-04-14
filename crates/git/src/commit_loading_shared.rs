use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommitLoadingError {
    #[error("git command failed: {0}")]
    GitError(String),
    #[error("process error: {0}")]
    ProcessError(#[from] std::io::Error),
}

pub type CommitLoadingResult<T> = Result<T, CommitLoadingError>;

pub struct CommitLoader {
    repo_path: std::path::PathBuf,
}

impl CommitLoader {
    pub fn new(repo_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
        }
    }

    fn finish_commit(
        commit: Option<crate::CommitItem>,
        filter_paths: &mut Vec<String>,
        filter_path: &str,
        pool: &mut HashMap<String, String>,
        commits: &mut Vec<crate::CommitItem>,
    ) {
        if let Some(mut commit) = commit {
            if !filter_paths.is_empty() && !filter_path.is_empty() {
                let has_non_prefixed = filter_paths
                    .iter()
                    .any(|path| !path.starts_with(filter_path));

                if has_non_prefixed {
                    commit.filter_paths = filter_paths
                        .iter()
                        .map(|path| {
                            let interned = pool
                                .entry(path.clone())
                                .or_insert_with(|| path.clone())
                                .clone();
                            PathBuf::from(interned)
                        })
                        .collect();
                }
            }
            commits.push(commit);
            filter_paths.clear();
        }
    }

    pub fn load_commits<F>(
        &self,
        args: &[&str],
        filter_path: &str,
        parse_log_line: F,
    ) -> CommitLoadingResult<Vec<crate::CommitItem>>
    where
        F: Fn(&str) -> Option<(crate::CommitItem, bool)> + Copy,
    {
        let mut commits = Vec::new();
        let mut current_commit: Option<crate::CommitItem> = None;
        let mut filter_paths: Vec<String> = Vec::new();
        let filter_path = filter_path.to_string();
        let mut pool: HashMap<String, String> = HashMap::new();

        let argv: Vec<String> = std::iter::once(String::from("git"))
            .chain(args.iter().map(|s| s.to_string()))
            .collect();

        let output = Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(&self.repo_path)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if line.is_empty() {
                continue;
            }

            if line.starts_with('+') && line.len() > 1 {
                Self::finish_commit(
                    current_commit.take(),
                    &mut filter_paths,
                    &filter_path,
                    &mut pool,
                    &mut commits,
                );

                if let Some((commit, stop)) = parse_log_line(&line[1..]) {
                    if stop {
                        continue;
                    }
                    current_commit = Some(commit);
                }
            } else if current_commit.is_some() && !filter_path.is_empty() {
                let fields: Vec<&str> = line.split('\t').collect();
                if fields.len() > 1 {
                    filter_paths.extend(fields[1..].iter().map(|s| s.to_string()));
                }
            }
        }

        Self::finish_commit(
            current_commit,
            &mut filter_paths,
            &filter_path,
            &mut pool,
            &mut commits,
        );

        Ok(commits)
    }
}

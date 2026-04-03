use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

pub fn temp_repo() -> io::Result<TempRepo> {
    TempRepo::new()
}

pub fn clean_repo() -> io::Result<TempRepo> {
    TempRepo::new()
}

pub fn dirty_repo() -> io::Result<TempRepo> {
    let repo = TempRepo::new()?;
    repo.write_file("dirty.txt", "dirty\n")?;
    Ok(repo)
}

pub fn staged_and_unstaged_repo() -> io::Result<TempRepo> {
    let repo = TempRepo::new()?;
    repo.write_file("tracked.txt", "base\n")?;
    repo.commit_all("initial")?;

    repo.write_file("staged.txt", "staged\n")?;
    repo.stage("staged.txt")?;

    repo.append_file("tracked.txt", "unstaged\n")?;
    repo.write_file("untracked.txt", "untracked\n")?;
    Ok(repo)
}

pub fn upstream_diverged_repo() -> io::Result<TempRepo> {
    let remote = TempRepo::bare()?;
    let seed = TempRepo::new()?;

    seed.write_file("shared.txt", "base\n")?;
    seed.commit_all("initial")?;
    seed.add_remote("origin", remote.path())?;
    seed.push("origin", "HEAD:main")?;

    let repo = TempRepo::clone_from(remote.path())?;
    repo.git(["checkout", "-B", "main"])?;

    repo.append_file("shared.txt", "local\n")?;
    repo.commit_all("local change")?;

    let upstream = TempRepo::clone_from(remote.path())?;
    upstream.git(["checkout", "-B", "main"])?;
    upstream.append_file("shared.txt", "remote\n")?;
    upstream.commit_all("remote change")?;
    upstream.push("origin", "HEAD:main")?;

    repo.fetch("origin")?;
    Ok(repo)
}

pub fn conflicted_repo() -> io::Result<TempRepo> {
    let repo = TempRepo::new()?;

    repo.write_file("conflict.txt", "base\n")?;
    repo.commit_all("initial")?;

    repo.checkout_new_branch("feature")?;
    repo.write_file("conflict.txt", "feature\n")?;
    repo.commit_all("feature change")?;

    repo.checkout("main")?;
    repo.write_file("conflict.txt", "main\n")?;
    repo.commit_all("main change")?;

    repo.git_expect_failure(["merge", "feature"])?;
    Ok(repo)
}

pub fn stashed_repo() -> io::Result<TempRepo> {
    let repo = TempRepo::new()?;

    repo.write_file("stash.txt", "base\n")?;
    repo.commit_all("initial")?;
    repo.append_file("stash.txt", "stashed\n")?;
    repo.write_file("stash-untracked.txt", "untracked\n")?;
    repo.git([
        "stash",
        "push",
        "--include-untracked",
        "-m",
        "fixture stash",
    ])?;

    Ok(repo)
}

pub fn detached_head_repo() -> io::Result<TempRepo> {
    let repo = TempRepo::new()?;

    repo.write_file("history.txt", "one\n")?;
    repo.commit_all("initial")?;
    repo.append_file("history.txt", "two\n")?;
    repo.commit_all("second")?;

    let head = repo.rev_parse("HEAD~1")?;
    repo.git(["checkout", head.as_str()])?;
    Ok(repo)
}

pub fn commands_testdata_a_file_repo() -> io::Result<TempRepo> {
    let repo = TempRepo::new()?;
    repo.write_file("a_file", "")?;
    Ok(repo)
}

pub fn commands_testdata_a_dir_file_repo() -> io::Result<TempRepo> {
    let repo = TempRepo::new()?;
    repo.write_file("a_dir/file", "")?;
    Ok(repo)
}

pub fn history_preview_repo() -> io::Result<TempRepo> {
    let repo = TempRepo::new()?;

    repo.write_file("history.txt", "one\n")?;
    repo.commit_all("initial")?;

    repo.append_file("history.txt", "two\n")?;
    repo.write_file("notes.md", "# Notes\n")?;
    repo.commit_all("second")?;

    repo.write_file("src/lib.rs", "pub fn answer() -> u32 {\n    42\n}\n")?;
    repo.commit_all("add lib")?;

    Ok(repo)
}

pub fn worktree_repo() -> io::Result<TempRepo> {
    let mut repo = TempRepo::new()?;

    repo.write_file("worktree.txt", "base\n")?;
    repo.commit_all("initial")?;
    repo.checkout_new_branch("feature")?;
    repo.checkout("main")?;
    repo.add_worktree("feature-tree", "feature")?;

    Ok(repo)
}

pub fn submodule_repo() -> io::Result<TempRepo> {
    let mut parent = TempRepo::new()?;

    parent.write_file("parent.txt", "parent\n")?;
    parent.commit_all("initial")?;

    let child_root = tempfile::tempdir()?;
    let child_path = child_root.path().join("child-module");
    fs::create_dir_all(&child_path)?;
    run_git(&child_path, &["init", "--initial-branch=main"])?;
    run_git(&child_path, &["config", "user.name", "Super Lazygit Tests"])?;
    run_git(&child_path, &["config", "user.email", "tests@example.com"])?;
    fs::write(child_path.join("child.txt"), "child\n")?;
    run_git(&child_path, &["add", "."])?;
    run_git(&child_path, &["commit", "-m", "child init"])?;

    let output = Command::new("git")
        .args([
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            child_path.to_str().unwrap_or("child-module"),
            "vendor/child-module",
        ])
        .current_dir(parent.path())
        .output()?;
    ensure_success("git submodule add", &output)?;
    parent.commit_all("add submodule")?;
    parent.attached_dirs.push(child_root);

    Ok(parent)
}

pub fn rebase_in_progress_repo() -> io::Result<TempRepo> {
    let repo = TempRepo::new()?;

    repo.write_file("rebase.txt", "base\n")?;
    repo.commit_all("initial")?;

    repo.checkout_new_branch("feature")?;
    repo.write_file("rebase.txt", "feature\n")?;
    repo.commit_all("feature change")?;

    repo.checkout("main")?;
    repo.write_file("rebase.txt", "main\n")?;
    repo.commit_all("main change")?;

    repo.checkout("feature")?;
    repo.git_expect_failure(["rebase", "main"])?;
    Ok(repo)
}

#[derive(Debug)]
pub struct TempRepo {
    root: TempDir,
    attached_dirs: Vec<TempDir>,
}

impl TempRepo {
    pub fn new() -> io::Result<Self> {
        let root = tempfile::tempdir()?;
        let repo = Self {
            root,
            attached_dirs: Vec::new(),
        };
        repo.git(["init", "--initial-branch=main"])?;
        repo.git(["config", "user.name", "Super Lazygit Tests"])?;
        repo.git(["config", "user.email", "tests@example.com"])?;
        Ok(repo)
    }

    pub fn bare() -> io::Result<Self> {
        let root = tempfile::tempdir()?;
        let repo = Self {
            root,
            attached_dirs: Vec::new(),
        };
        repo.git(["init", "--bare", "--initial-branch=main"])?;
        Ok(repo)
    }

    pub fn clone_from(remote: impl AsRef<Path>) -> io::Result<Self> {
        let root = tempfile::tempdir()?;
        let status = Command::new("git")
            .arg("clone")
            .arg(remote.as_ref())
            .arg(root.path())
            .output()?;
        ensure_success("git clone", &status)?;

        let repo = Self {
            root,
            attached_dirs: Vec::new(),
        };
        repo.git(["config", "user.name", "Super Lazygit Tests"])?;
        repo.git(["config", "user.email", "tests@example.com"])?;
        Ok(repo)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        self.root.path()
    }

    pub fn write_file(&self, relative: impl AsRef<Path>, contents: &str) -> io::Result<()> {
        let path = self.path().join(relative.as_ref());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)
    }

    pub fn append_file(&self, relative: impl AsRef<Path>, contents: &str) -> io::Result<()> {
        let path = self.path().join(relative.as_ref());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut existing = if path.exists() {
            fs::read_to_string(&path)?
        } else {
            String::new()
        };
        existing.push_str(contents);
        fs::write(path, existing)
    }

    pub fn stage(&self, relative: impl AsRef<Path>) -> io::Result<()> {
        let output = Command::new("git")
            .arg("add")
            .arg(relative.as_ref())
            .current_dir(self.path())
            .output()?;
        ensure_success("git add", &output)
    }

    pub fn commit_all(&self, message: &str) -> io::Result<()> {
        self.git(["add", "."])?;
        self.git(["commit", "-m", message])
    }

    pub fn checkout(&self, reference: &str) -> io::Result<()> {
        self.git(["checkout", reference])
    }

    pub fn checkout_new_branch(&self, branch: &str) -> io::Result<()> {
        self.git(["checkout", "-b", branch])
    }

    pub fn add_remote(&self, name: &str, path: impl AsRef<Path>) -> io::Result<()> {
        let output = Command::new("git")
            .arg("remote")
            .arg("add")
            .arg(name)
            .arg(path.as_ref())
            .current_dir(self.path())
            .output()?;
        ensure_success("git remote add", &output)
    }

    pub fn fetch(&self, remote: &str) -> io::Result<()> {
        self.git(["fetch", remote])
    }

    pub fn push(&self, remote: &str, refspec: &str) -> io::Result<()> {
        self.git(["push", "-u", remote, refspec])
    }

    pub fn rev_parse(&self, rev: &str) -> io::Result<String> {
        let output = self.git_capture(["rev-parse", rev])?;
        stdout_string(output)
    }

    pub fn status_porcelain(&self) -> io::Result<String> {
        let output = self.git_capture(["status", "--short"])?;
        stdout_string(output)
    }

    pub fn stash_list(&self) -> io::Result<String> {
        let output = self.git_capture(["stash", "list"])?;
        stdout_string(output)
    }

    pub fn worktree_list(&self) -> io::Result<String> {
        let output = self.git_capture(["worktree", "list", "--porcelain"])?;
        stdout_string(output)
    }

    pub fn current_branch(&self) -> io::Result<String> {
        let output = self.git_capture(["branch", "--show-current"])?;
        stdout_string(output)
    }

    pub fn symbolic_head(&self) -> io::Result<String> {
        let output = self.git_capture(["symbolic-ref", "--quiet", "--short", "HEAD"])?;
        stdout_string(output)
    }

    pub fn add_worktree(&mut self, name: &str, branch: &str) -> io::Result<PathBuf> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(name);
        let output = Command::new("git")
            .arg("worktree")
            .arg("add")
            .arg(&path)
            .arg(branch)
            .current_dir(self.path())
            .output()?;
        ensure_success("git worktree add", &output)?;
        self.attached_dirs.push(dir);
        Ok(path)
    }

    pub fn git<I, S>(&self, args: I) -> io::Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.git_output(args)?;
        ensure_success("git", &output)
    }

    pub fn git_capture<I, S>(&self, args: I) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.git_output(args)?;
        ensure_success("git", &output)?;
        Ok(output)
    }

    pub fn git_expect_failure<I, S>(&self, args: I) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.git_output(args)?;
        if output.status.success() {
            return Err(io::Error::other("expected git command to fail"));
        }
        Ok(output)
    }

    fn git_output<I, S>(&self, args: I) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Command::new("git")
            .args(args)
            .current_dir(self.path())
            .output()
    }
}

fn run_git(dir: &Path, args: &[&str]) -> io::Result<()> {
    let output = Command::new("git").args(args).current_dir(dir).output()?;
    ensure_success("git", &output)
}

fn ensure_success(command: &str, output: &Output) -> io::Result<()> {
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(io::Error::other(format!(
        "{command} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status, stdout, stderr
    )))
}

fn stdout_string(output: Output) -> io::Result<String> {
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_clean_repo() -> io::Result<()> {
        let repo = clean_repo()?;

        assert!(repo.path().join(".git").exists());
        assert_eq!(repo.status_porcelain()?, "");
        assert_eq!(repo.current_branch()?, "main");
        Ok(())
    }

    #[test]
    fn creates_dirty_repo() -> io::Result<()> {
        let repo = dirty_repo()?;

        let status = repo.status_porcelain()?;
        assert!(status.contains("?? dirty.txt"));
        Ok(())
    }

    #[test]
    fn creates_staged_and_unstaged_mix() -> io::Result<()> {
        let repo = staged_and_unstaged_repo()?;

        let status = repo.status_porcelain()?;
        assert!(status.contains("A  staged.txt"));
        assert!(status.contains(" M tracked.txt"));
        assert!(status.contains("?? untracked.txt"));
        Ok(())
    }

    #[test]
    fn creates_upstream_divergence() -> io::Result<()> {
        let repo = upstream_diverged_repo()?;

        let ahead = repo.git_capture(["rev-list", "--count", "origin/main..HEAD"])?;
        let behind = repo.git_capture(["rev-list", "--count", "HEAD..origin/main"])?;

        assert_eq!(stdout_string(ahead)?, "1");
        assert_eq!(stdout_string(behind)?, "1");
        Ok(())
    }

    #[test]
    fn creates_conflicted_repo() -> io::Result<()> {
        let repo = conflicted_repo()?;

        let status = repo.status_porcelain()?;
        assert!(status.contains("UU conflict.txt"));
        Ok(())
    }

    #[test]
    fn creates_stashed_repo() -> io::Result<()> {
        let repo = stashed_repo()?;

        assert!(repo.stash_list()?.contains("fixture stash"));
        assert_eq!(repo.status_porcelain()?, "");
        Ok(())
    }

    #[test]
    fn creates_detached_head_repo() -> io::Result<()> {
        let repo = detached_head_repo()?;

        let head = repo.git_expect_failure(["symbolic-ref", "--quiet", "HEAD"])?;
        assert!(head.status.code().is_some());
        Ok(())
    }

    #[test]
    fn creates_history_preview_repo() -> io::Result<()> {
        let repo = history_preview_repo()?;

        let log = repo.git_capture(["log", "--format=%s", "-n", "3"])?;
        let log = stdout_string(log)?;
        assert!(log.contains("add lib"));
        assert!(log.contains("second"));
        assert!(log.contains("initial"));
        Ok(())
    }

    #[test]
    fn creates_worktree_repo() -> io::Result<()> {
        let repo = worktree_repo()?;

        let worktrees = repo.worktree_list()?;
        assert!(worktrees.contains("branch refs/heads/feature"));
        assert!(worktrees.contains("feature-tree"));
        Ok(())
    }

    #[test]
    fn creates_submodule_repo() -> io::Result<()> {
        let repo = submodule_repo()?;

        let status = stdout_string(repo.git_capture(["submodule", "status"])?)?;
        assert!(status.contains("vendor/child-module"));
        assert!(repo.path().join("vendor/child-module/.git").exists());
        Ok(())
    }

    #[test]
    fn creates_exact_empty_top_level_a_file_fixture() -> io::Result<()> {
        let repo = commands_testdata_a_file_repo()?;
        let path = repo.path().join("a_file");

        assert!(path.is_file());
        assert_eq!(fs::read_to_string(path)?, "");
        Ok(())
    }

    #[test]
    fn creates_exact_empty_nested_a_dir_file_fixture() -> io::Result<()> {
        let repo = commands_testdata_a_dir_file_repo()?;
        let path = repo.path().join("a_dir").join("file");

        assert!(path.is_file());
        assert_eq!(fs::read_to_string(path)?, "");
        Ok(())
    }
}

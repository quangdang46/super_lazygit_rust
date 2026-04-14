// Ported from ./references/lazygit-master/pkg/gui/presentation/icons/git_icons.go

use std::sync::RwLock;
use std::collections::HashMap;

use super_lazygit_core::state::{BranchItem, CommitItem, RemoteItem, TagItem, StashItem};

static BRANCH_ICON: RwLock<&'static str> = RwLock::new("\u{f062c}");
static DETACHED_HEAD_ICON: &str = "\ue729";
static TAG_ICON: &str = "\uf02b";
static COMMIT_ICON: RwLock<&'static str> = RwLock::new("\u{f0718}");
static MERGE_COMMIT_ICON: RwLock<&'static str> = RwLock::new("\u{f062d}");
static DEFAULT_REMOTE_ICON: RwLock<&'static str> = RwLock::new("\u{f02a2}");
static STASH_ICON: &str = "\uf01c";
pub static LINKED_WORKTREE_ICON: RwLock<&'static str> = RwLock::new("\u{f0339}");
pub static MISSING_LINKED_WORKTREE_ICON: RwLock<&'static str> = RwLock::new("\u{f033a}");

static REMOTE_ICONS: RwLock<HashMap<&'static str, &'static str>> = RwLock::new(HashMap::from([
    ("github.com", "\ue709"),
    ("bitbucket.org", "\ue703"),
    ("gitlab.com", "\uf296"),
    ("dev.azure.com", "\u{f0805}"),
    ("codeberg.org", "\uf330"),
    ("git.FreeBSD.org", "\uf30c"),
    ("gitlab.archlinux.org", "\uf303"),
    ("gitlab.freedesktop.org", "\uf360"),
    ("gitlab.gnome.org", "\uf361"),
    ("gnu.org", "\ue779"),
    ("invent.kde.org", "\uf373"),
    ("kernel.org", "\uf31a"),
    ("salsa.debian.org", "\uf306"),
    ("sr.ht", "\uf1db"),
]));

pub fn patch_for_nerd_fonts_v2() {
    *BRANCH_ICON.write().unwrap() = "\ufb2b";
    *COMMIT_ICON.write().unwrap() = "\ufc16";
    *MERGE_COMMIT_ICON.write().unwrap() = "\ufb2c";
    *DEFAULT_REMOTE_ICON.write().unwrap() = "\uf7a1";
    *LINKED_WORKTREE_ICON.write().unwrap() = "\uf838";
    *MISSING_LINKED_WORKTREE_ICON.write().unwrap() = "\uf839";
    
    let mut icons = REMOTE_ICONS.write().unwrap();
    icons.insert("dev.azure.com", "\ufd03");
}

pub fn icon_for_branch(branch: &BranchItem) -> String {
    if branch.detached_head {
        DETACHED_HEAD_ICON.to_string()
    } else {
        BRANCH_ICON.read().unwrap().to_string()
    }
}

pub fn icon_for_remote_branch(_branch: &BranchItem) -> String {
    BRANCH_ICON.read().unwrap().to_string()
}

pub fn icon_for_tag(_tag: &TagItem) -> String {
    TAG_ICON.to_string()
}

pub fn icon_for_commit(commit: &CommitItem) -> String {
    if commit.is_merge() {
        MERGE_COMMIT_ICON.read().unwrap().to_string()
    } else {
        COMMIT_ICON.read().unwrap().to_string()
    }
}

pub fn icon_for_remote(remote: &RemoteItem) -> String {
    let icons = REMOTE_ICONS.read().unwrap();
    for (domain, icon) in icons.iter() {
        for url in &remote.urls {
            if url.contains(domain) {
                return icon.to_string();
            }
        }
    }
    DEFAULT_REMOTE_ICON.read().unwrap().to_string()
}

pub fn icon_for_stash(_stash: &StashItem) -> String {
    STASH_ICON.to_string()
}

pub fn icon_for_worktree(missing: bool) -> String {
    if missing {
        MISSING_LINKED_WORKTREE_ICON.read().unwrap().to_string()
    } else {
        LINKED_WORKTREE_ICON.read().unwrap().to_string()
    }
}

pub fn linked_worktree_icon() -> String {
    LINKED_WORKTREE_ICON.read().unwrap().to_string()
}

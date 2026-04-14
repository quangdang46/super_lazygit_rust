// Ported from ./references/lazygit-master/pkg/gui/presentation/icons/icons.go

use std::sync::RwLock;

static IS_ICON_ENABLED: RwLock<bool> = RwLock::new(false);

pub struct IconProperties {
    pub icon: String,
    pub color: String,
}

impl IconProperties {
    pub fn new(icon: &str, color: &str) -> Self {
        Self {
            icon: icon.to_string(),
            color: color.to_string(),
        }
    }
}

pub fn is_icon_enabled() -> bool {
    *IS_ICON_ENABLED.read().unwrap()
}

pub fn set_nerd_fonts_version(version: &str) {
    if version.is_empty() {
        *IS_ICON_ENABLED.write().unwrap() = false;
    } else {
        if version != "2" && version != "3" {
            panic!("Unsupported nerdFontVersion {}", version);
        }

        if version == "2" {
            patch_git_icons_for_nerd_fonts_v2();
            patch_file_icons_for_nerd_fonts_v2();
        }

        *IS_ICON_ENABLED.write().unwrap() = true;
    }
}

mod git_icons;
mod file_icons;

pub use git_icons::{
    icon_for_branch, icon_for_commit, icon_for_remote, icon_for_remote_branch, icon_for_stash,
    icon_for_tag, icon_for_worktree, linked_worktree_icon, LINKED_WORKTREE_ICON,
};
pub use file_icons::{
    default_directory_icon, default_file_icon, default_submodule_icon, icon_for_file,
};

fn patch_git_icons_for_nerd_fonts_v2() {
    git_icons::patch_for_nerd_fonts_v2();
}

fn patch_file_icons_for_nerd_fonts_v2() {
    file_icons::patch_for_nerd_fonts_v2();
}

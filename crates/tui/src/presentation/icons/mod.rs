pub mod file_icons;
pub mod git_icons;

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconProperties {
    pub icon: &'static str,
    pub color: &'static str,
}

static ICON_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn is_icon_enabled() -> bool {
    ICON_ENABLED.load(Ordering::Relaxed)
}

pub fn set_nerd_fonts_version(version: &str) {
    if version.is_empty() {
        ICON_ENABLED.store(false, Ordering::Relaxed);
    } else {
        assert!(
            version == "2" || version == "3",
            "Unsupported nerdFontVersion {version}"
        );
        ICON_ENABLED.store(true, Ordering::Relaxed);
    }
}

pub use file_icons::icon_for_file;

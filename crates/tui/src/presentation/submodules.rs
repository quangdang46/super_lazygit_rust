// Ported from ./references/lazygit-master/pkg/gui/presentation/submodules.go

use ratatui::text::Span;

use super_lazygit_core::SubmoduleItem;

use super::icons::file_icons::DEFAULT_SUBMODULE_ICON;
use super::icons::is_icon_enabled;

/// Get display strings for submodule with styling.
pub fn get_submodule_display_strings(submodule: &SubmoduleItem) -> Vec<Span<'static>> {
    let prefix = if submodule.name.contains('/') {
        "  - " // Nested submodule indication
    } else {
        ""
    };

    let icon = if is_icon_enabled() {
        DEFAULT_SUBMODULE_ICON.icon
    } else {
        ""
    };

    vec![
        Span::raw(format!("{}{}", prefix, icon)),
        Span::raw(submodule.name.clone()),
    ]
}
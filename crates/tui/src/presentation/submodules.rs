// Ported from ./references/lazygit-master/pkg/gui/presentation/submodules.go

use ratatui::style::Style;
use ratatui::text::Span;

use super_lazygit_core::SubmoduleItem;

/// Get display strings for submodule with styling.
/// Parity: getSubmoduleDisplayStrings in Go
pub fn get_submodule_display_strings(submodule: &SubmoduleItem) -> Vec<Span<'static>> {
    let name = submodule.name.clone();
    // theme.DefaultTextColor - just return the raw name since Line will apply default styling
    vec![Span::raw(name)]
}
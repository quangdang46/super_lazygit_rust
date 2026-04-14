// Ported from ./references/lazygit-master/pkg/gui/presentation/stash_entries.go

use ratatui::style::{Color, Style};
use ratatui::text::Span;

pub struct StashEntry {
    pub name: String,
    pub recency: String,
}

impl StashEntry {
    pub fn ref_name(&self) -> &str {
        &self.name
    }
}

/// Get display spans for a stash entry with proper styling.
/// Parity: `getStashEntryDisplayStrings` in `presentation/stash_entries.go`.
pub fn get_stash_entry_display_strings(s: &StashEntry, diffed: bool) -> Vec<Span<'static>> {
    let text_color = if diffed { Color::Cyan } else { Color::Reset };

    let mut result = Vec::with_capacity(3);
    // Recency in cyan
    result.push(Span::styled(s.recency.clone(), Style::default().fg(Color::Cyan)));

    // Stash name in diff color
    result.push(Span::styled(s.name.clone(), Style::default().fg(text_color)));

    result
}

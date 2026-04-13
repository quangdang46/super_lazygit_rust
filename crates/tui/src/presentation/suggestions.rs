// Ported from ./references/lazygit-master/pkg/gui/presentation/suggestions.go

use ratatui::text::Span;

/// Suggestion item structure.
pub struct Suggestion {
    pub label: String,
}

/// Get display strings for suggestion with styling.
pub fn get_suggestion_display_strings(suggestion: &Suggestion) -> Vec<Span<'static>> {
    vec![Span::raw(suggestion.label.clone())]
}

/// Get display strings for multiple suggestions.
pub fn get_suggestion_list_display_strings(suggestions: &[Suggestion]) -> Vec<Vec<Span<'static>>> {
    suggestions
        .iter()
        .map(get_suggestion_display_strings)
        .collect()
}
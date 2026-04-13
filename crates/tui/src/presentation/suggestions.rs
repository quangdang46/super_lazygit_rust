// Ported from ./references/lazygit-master/pkg/gui/presentation/suggestions.go

use ratatui::text::Span;

use super_lazygit_core::PromptSuggestion;

/// Get display strings for suggestion with styling.
pub fn get_suggestion_display_strings(suggestion: &PromptSuggestion) -> Vec<Span<'static>> {
    // In Go: []string{suggestion.Label}
    // Label can contain color codes, so we use raw span
    vec![Span::raw(suggestion.label.clone())]
}

/// Get display strings for multiple suggestions.
pub fn get_suggestion_list_display_strings(suggestions: &[PromptSuggestion]) -> Vec<Vec<Span<'static>>> {
    suggestions
        .iter()
        .map(get_suggestion_display_strings)
        .collect()
}
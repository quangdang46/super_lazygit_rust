use super_lazygit_tui::presentation::Suggestion;

pub fn get_suggestion_list_display_strings(suggestions: &[Suggestion]) -> Vec<Vec<String>> {
    suggestions
        .iter()
        .map(|suggestion| get_suggestion_display_strings(suggestion))
        .collect()
}

fn get_suggestion_display_strings(suggestion: &Suggestion) -> Vec<String> {
    vec![suggestion.label.clone()]
}

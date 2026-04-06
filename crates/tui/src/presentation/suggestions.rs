// Ported from ./references/lazygit-master/pkg/gui/presentation/suggestions.go

pub struct Suggestion {
    pub label: String,
}

pub fn get_suggestion_display_strings(suggestion: &Suggestion) -> Vec<String> {
    vec![suggestion.label.clone()]
}

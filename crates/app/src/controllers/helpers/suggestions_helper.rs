// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/suggestions_helper.go

pub struct SuggestionsHelper {
    common: HelperCommon,
}

pub struct HelperCommon;

pub struct Suggestion {
    pub value: String,
    pub label: String,
}

impl SuggestionsHelper {
    pub fn new(common: HelperCommon) -> Self {
        Self { common }
    }

    fn get_remote_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn get_branch_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn get_tag_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn get_remote_branch_names(&self, _separator: &str) -> Vec<String> {
        Vec::new()
    }

    fn get_remote_branch_names_for_remote(&self, _remote_name: &str) -> Vec<String> {
        Vec::new()
    }

    pub fn get_remote_suggestions_func(&self) -> impl Fn(String) -> Vec<Suggestion> {
        move |_input: String| Vec::new()
    }

    pub fn get_branch_name_suggestions_func(&self) -> impl Fn(String) -> Vec<Suggestion> {
        move |_input: String| Vec::new()
    }

    pub fn get_file_path_suggestions_func(&self) -> impl Fn(String) -> Vec<Suggestion> {
        move |_input: String| Vec::new()
    }

    pub fn get_remote_branches_suggestions_func(
        &self,
        _separator: &str,
    ) -> impl Fn(String) -> Vec<Suggestion> {
        move |_input: String| Vec::new()
    }

    pub fn get_remote_branches_for_remote_suggestions_func(
        &self,
        _remote_name: &str,
    ) -> impl Fn(String) -> Vec<Suggestion> {
        move |_input: String| Vec::new()
    }

    pub fn get_tags_suggestions_func(&self) -> impl Fn(String) -> Vec<Suggestion> {
        move |_input: String| Vec::new()
    }

    pub fn get_refs_suggestions_func(&self) -> impl Fn(String) -> Vec<Suggestion> {
        move |_input: String| Vec::new()
    }

    pub fn get_authors_suggestions_func(&self) -> impl Fn(String) -> Vec<Suggestion> {
        move |_input: String| Vec::new()
    }
}

fn matches_to_suggestions(matches: Vec<String>) -> Vec<Suggestion> {
    matches
        .into_iter()
        .map(|m| Suggestion {
            value: m.clone(),
            label: m,
        })
        .collect()
}

pub fn filter_func(
    options: Vec<String>,
    _use_fuzzy_search: bool,
) -> impl Fn(String) -> Vec<Suggestion> {
    move |input: String| {
        let matches = if input.is_empty() {
            options.clone()
        } else {
            options.clone()
        };
        matches_to_suggestions(matches)
    }
}

// Ported from ./references/lazygit-master/pkg/gui/presentation/stash_entries.go

pub struct StashEntry {
    pub name: String,
    pub recency: String,
}

impl StashEntry {
    pub fn ref_name(&self) -> &str {
        &self.name
    }
}

pub fn get_stash_entry_display_strings(s: &StashEntry, diffed: bool) -> Vec<String> {
    let text_style = if diffed { "Cyan" } else { "Default" };

    let mut result = Vec::with_capacity(3);
    result.push("Cyan".to_string());
    result.push(format!("{:?}", text_style));
    result.push(s.name.clone());
    result
}

use crate::style::theme::default_text_color;
use super_lazygit_core::state::SubmoduleItem;

pub fn get_submodule_list_display_strings(submodules: &[SubmoduleItem]) -> Vec<Vec<String>> {
    submodules
        .iter()
        .map(|submodule| get_submodule_display_strings(submodule))
        .collect()
}

fn get_submodule_display_strings(s: &SubmoduleItem) -> Vec<String> {
    let name = s.name.clone();
    vec![default_text_color().sprint(&name)]
}

use super_lazygit_core::state::StashItem;

use crate::presentation::icons::{icon_for_stash, is_icon_enabled};
use crate::style::basic_styles::fg_cyan;
use crate::style::text_style::TextStyle;
use crate::style::theme::{default_text_color, diff_terminal_color};

pub fn get_stash_entry_list_display_strings(
    stash_entries: &[StashItem],
    diff_name: &str,
) -> Vec<Vec<String>> {
    stash_entries
        .iter()
        .map(|stash_entry| {
            let diffed = stash_entry.stash_ref == diff_name;
            get_stash_entry_display_strings(stash_entry, diffed)
        })
        .collect()
}

fn get_stash_entry_display_strings(s: &StashItem, diffed: bool) -> Vec<String> {
    let text_style = if diffed {
        diff_terminal_color()
    } else {
        default_text_color()
    };

    let mut res = Vec::with_capacity(3);
    res.push(fg_cyan().sprint(&s.recency));

    if is_icon_enabled() {
        res.push(text_style.sprint(&icon_for_stash(s)));
    }

    res.push(text_style.sprint(&s.name));
    res
}

use super_lazygit_core::state::RemoteBranchItem;

use crate::presentation::icons::{icon_for_remote_branch, is_icon_enabled};
use crate::style::text_style::TextStyle;
use crate::style::theme::diff_terminal_color;

use crate::presentation::get_branch_text_style;

pub fn get_remote_branch_list_display_strings(
    branches: &[RemoteBranchItem],
    diff_name: &str,
) -> Vec<Vec<String>> {
    branches
        .iter()
        .map(|branch| {
            let diffed = branch.full_name() == diff_name;
            get_remote_branch_display_strings(branch, diffed)
        })
        .collect()
}

fn get_remote_branch_display_strings(b: &RemoteBranchItem, diffed: bool) -> Vec<String> {
    let text_style = get_branch_text_style(&b.name);
    let text_style = if diffed {
        diff_terminal_color()
    } else {
        text_style
    };

    let mut res = Vec::with_capacity(2);
    if is_icon_enabled() {
        res.push(text_style.sprint(&icon_for_remote_branch(b)));
    }
    res.push(text_style.sprint(&b.name));
    res
}

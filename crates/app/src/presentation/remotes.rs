use super_lazygit_core::state::RemoteItem;

use crate::i18n::TranslationSet;
use crate::presentation::icons::{icon_for_remote, is_icon_enabled};
use crate::presentation::item_operation_to_string;
use crate::presentation::loader::{loader, SpinnerConfig};
use crate::style::basic_styles::fg_blue;
use crate::style::text_style::TextStyle;
use crate::style::theme::{default_text_color, diff_terminal_color};
use super_lazygit_tui::presentation::ItemOperation;

pub fn get_remote_list_display_strings(
    remotes: &[RemoteItem],
    diff_name: &str,
    get_item_operation: impl Fn(&dyn crate::types::common::HasUrn) -> ItemOperation,
    tr: &TranslationSet,
) -> Vec<Vec<String>> {
    remotes
        .iter()
        .map(|remote| {
            let diffed = remote.name == diff_name;
            get_remote_display_strings(remote, diffed, get_item_operation(remote), tr)
        })
        .collect()
}

fn get_remote_display_strings(
    r: &RemoteItem,
    diffed: bool,
    item_operation: ItemOperation,
    tr: &TranslationSet,
) -> Vec<String> {
    let branch_count = r.branch_count;

    let text_style = if diffed {
        diff_terminal_color()
    } else {
        default_text_color()
    };

    let mut res = Vec::with_capacity(3);
    if is_icon_enabled() {
        res.push(text_style.sprint(&icon_for_remote(r)));
    }

    let description_str = fg_blue().sprint(&format!("{} branches", branch_count));

    let item_operation_str = item_operation_to_string(item_operation, tr);
    let description_str = if !item_operation_str.is_empty() {
        let spinner_config = SpinnerConfig::default();
        let now = std::time::SystemTime::now();
        let now_millis = now
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let spinner = loader(now_millis, &spinner_config);
        format!(
            "{} {}",
            description_str,
            fg_blue().sprint(&format!("{} {}", item_operation_str, spinner))
        )
    } else {
        description_str
    };

    res.push(text_style.sprint(&r.name));
    res.push(description_str);
    res
}

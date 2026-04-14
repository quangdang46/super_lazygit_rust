use super_lazygit_core::state::TagItem;

use crate::i18n::TranslationSet;
use crate::presentation::icons::{icon_for_tag, is_icon_enabled};
use crate::presentation::item_operation_to_string;
use crate::presentation::loader::{loader, SpinnerConfig};
use crate::style::basic_styles::fg_cyan;
use crate::style::text_style::TextStyle;
use crate::style::theme::{default_text_color, diff_terminal_color};
use super_lazygit_tui::presentation::ItemOperation;

pub fn get_tag_list_display_strings(
    tags: &[TagItem],
    get_item_operation: impl Fn(&dyn crate::types::common::HasUrn) -> ItemOperation,
    diff_name: &str,
    tr: &TranslationSet,
) -> Vec<Vec<String>> {
    tags.iter()
        .map(|tag| {
            let diffed = tag.name == diff_name;
            get_tag_display_strings(tag, get_item_operation(tag), diffed, tr)
        })
        .collect()
}

fn get_tag_display_strings(
    t: &TagItem,
    item_operation: ItemOperation,
    diffed: bool,
    tr: &TranslationSet,
) -> Vec<String> {
    let text_style = if diffed {
        diff_terminal_color()
    } else {
        default_text_color()
    };

    let mut res = Vec::with_capacity(2);
    if is_icon_enabled() {
        res.push(text_style.sprint(&icon_for_tag(t)));
    }

    let description_color = fg_cyan();
    let description_str = description_color.sprint(t.description());

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
            "{} {} {}",
            fg_cyan().sprint(&format!("{} {}", item_operation_str, spinner)),
            description_str
        )
    } else {
        description_str
    };

    res.push(text_style.sprint(&t.name));
    res.push(description_str);
    res
}

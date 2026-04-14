// Ported from ./references/lazygit-master/pkg/gui/presentation/tags.go

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use super::item_operations::ItemOperation;

pub struct Tag {
    pub name: String,
}

impl Tag {
    pub fn description(&self) -> String {
        "tag_description".to_string()
    }
}

pub struct TagDisplayOptions<'a> {
    pub tag: &'a Tag,
    pub item_operation: ItemOperation,
    pub diffed: bool,
}

/// Get styled spans for a tag display.
/// Parity: `GetTagDisplayStrings` in `presentation/tags.go`.
pub fn get_tag_display_strings(opts: TagDisplayOptions<'_>) -> Vec<Span<'static>> {
    let text_color = if opts.diffed { Color::Cyan } else { Color::Reset };

    let mut spans = Vec::with_capacity(2);
    spans.push(Span::styled(opts.tag.name.clone(), Style::default().fg(text_color)));

    // Description in yellow
    let description = opts.tag.description();
    if !description.is_empty() {
        spans.push(Span::styled(
            format!(" {}", description),
            Style::default().fg(Color::Yellow),
        ));
    }

    spans
}

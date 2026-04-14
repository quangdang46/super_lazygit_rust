// Ported from ./references/lazygit-master/pkg/gui/presentation/remotes.go

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use super::item_operations::ItemOperation;

pub struct Remote {
    pub name: String,
    pub branches: Vec<String>,
}

pub struct RemoteDisplayOptions<'a> {
    pub remote: &'a Remote,
    pub diffed: bool,
    pub item_operation: ItemOperation,
    pub branch_count: usize,
}

/// Get styled display strings for a remote.
/// Parity: `GetRemoteDisplayStrings` in `presentation/remotes.go`.
pub fn get_remote_display_strings(opts: RemoteDisplayOptions<'_>) -> Vec<Span<'static>> {
    let text_color = if opts.diffed { Color::Cyan } else { Color::Reset };

    let mut spans = Vec::with_capacity(3);
    spans.push(Span::styled(opts.remote.name.clone(), Style::default().fg(text_color)));

    let description = format!("[{} branches]", opts.branch_count);
    spans.push(Span::styled(description, Style::default().fg(Color::Blue)));

    spans
}

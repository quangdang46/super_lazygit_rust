// Ported from ./references/lazygit-master/pkg/gui/presentation/remote_branches.go

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use super_lazygit_core::RemoteBranchItem;

/// Get display strings for remote branch with styling.
pub fn get_remote_branch_display_strings(branch: &RemoteBranchItem, diffed: bool) -> Vec<Span<'static>> {
    let name_style = if diffed {
        Style::default().fg(Color::Cyan) // DiffTerminalColor
    } else {
        // Use GetBranchTextStyle logic - matches Go's GetBranchTextStyle(b.Name)
        Style::default() // DefaultTextColor
    };

    vec![Span::styled(branch.full_name(), name_style)]
}
// Ported from ./references/lazygit-master/pkg/gui/command_log_panel.go

/// Our UI command log looks like this:
/// Stage File:
/// git add -- 'filename'
/// Unstage File:
/// git reset HEAD 'filename'
///
/// The 'Stage File' and 'Unstage File' lines are actions i.e they group up a set
/// of command logs (typically there's only one command under an action but there may be more).
/// So we call log_action to log the 'Stage File' part and then we call log_command to log the command itself.
/// We pass log_command to our OSCommand struct so that it can handle logging commands for us.
pub struct CommandLogPanel;

impl CommandLogPanel {
    pub fn log_action(action: &str) -> String {
        format!("\n{}", action)
    }

    pub fn log_command(cmd_str: &str, command_line: bool) -> String {
        let text_style = if command_line { "default" } else { "magenta" };
        let indented = format!("  {}", cmd_str.replace("\n", "\n  "));
        format!("\n[{}] {}", text_style, indented)
    }

    pub fn print_header() -> String {
        String::new()
    }
}

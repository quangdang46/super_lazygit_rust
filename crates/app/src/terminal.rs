use std::io::{self, IsTerminal, Stdout};
use std::process::{Command, ExitStatus};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use super_lazygit_core::{Event, InputEvent, KeyPress, TimerEvent, Timestamp};

use crate::runtime::AppRuntime;

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const PERIODIC_REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const PERIODIC_FETCH_INTERVAL: Duration = Duration::from_secs(60);
const TOAST_EXPIRY_INTERVAL: Duration = Duration::from_millis(250);

pub fn run(runtime: &mut AppRuntime) -> Result<()> {
    let mut session = TerminalSession::enter()?;
    let area = session.terminal.size()?;
    runtime.run([Event::Input(InputEvent::Resize {
        width: area.width,
        height: area.height,
    })]);

    let mut last_refresh_tick = Instant::now();
    let mut last_fetch_tick = Instant::now();
    let mut last_toast_tick = Instant::now();

    loop {
        session
            .terminal
            .draw(|frame| runtime.draw_frame(frame))
            .map(|_| ())?;

        if event::poll(EVENT_POLL_INTERVAL)? {
            match event::read()? {
                CrosstermEvent::Key(key)
                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                {
                    if should_exit(key) {
                        break;
                    }

                    if should_suspend(key) {
                        suspend_app_process()?;
                        continue;
                    }

                    if let Some(keypress) = keypress_from_event(key) {
                        runtime.run([Event::Input(InputEvent::KeyPressed(keypress))]);
                    }
                }
                CrosstermEvent::Paste(text) if !text.is_empty() => {
                    runtime.run([Event::Input(InputEvent::Paste(text))]);
                }
                CrosstermEvent::Mouse(mouse) => {
                    if let Some(input) = mouse_input_from_event(mouse.column, mouse.row, mouse.kind)
                    {
                        runtime.run([Event::Input(input)]);
                    }
                }
                CrosstermEvent::Resize(width, height) => {
                    runtime.run([Event::Input(InputEvent::Resize { width, height })]);
                }
                _ => {}
            }
        }

        let now = Instant::now();
        let mut timed_events = Vec::new();

        if now.duration_since(last_refresh_tick) >= PERIODIC_REFRESH_INTERVAL {
            timed_events.push(Event::Timer(TimerEvent::PeriodicRefreshTick));
            last_refresh_tick = now;
        }

        if now.duration_since(last_fetch_tick) >= PERIODIC_FETCH_INTERVAL {
            timed_events.push(Event::Timer(TimerEvent::PeriodicFetchTick));
            last_fetch_tick = now;
        }

        if now.duration_since(last_toast_tick) >= TOAST_EXPIRY_INTERVAL {
            timed_events.push(Event::Timer(TimerEvent::ToastExpiryTick {
                now: current_timestamp(),
            }));
            last_toast_tick = now;
        }

        if !timed_events.is_empty() {
            runtime.run(timed_events);
        }
    }

    Ok(())
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableBracketedPaste,
            EnableMouseCapture,
            Hide
        )?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.hide_cursor()?;
        terminal.clear()?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.terminal.show_cursor();
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            Show,
            DisableBracketedPaste,
            DisableMouseCapture,
            LeaveAlternateScreen
        );
    }
}

pub fn run_external_command(command: &mut Command) -> io::Result<()> {
    run_external_command_named(command, "editor")
}

pub fn run_external_command_named(command: &mut Command, label: &str) -> io::Result<()> {
    run_external_command_named_with_options(command, label, true)
}

pub fn run_external_command_named_with_options(
    command: &mut Command,
    label: &str,
    suspend: bool,
) -> io::Result<()> {
    if !suspend || !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return command
            .status()
            .and_then(|status| require_success(status, label));
    }

    suspend_terminal()?;
    let status = command.status();
    let resume = resume_terminal();
    resume?;
    status.and_then(|status| require_success(status, label))
}

#[cfg(unix)]
pub fn suspend_app_process() -> io::Result<()> {
    let mut command = Command::new("sh");
    command.args(["-lc", "kill -TSTP $PPID"]);
    run_external_command_named(&mut command, "suspend")
}

#[cfg(not(unix))]
pub fn suspend_app_process() -> io::Result<()> {
    Ok(())
}

fn suspend_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        Show,
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen
    )
}

fn resume_terminal() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture,
        Hide
    )
}

fn require_success(status: ExitStatus, label: &str) -> io::Result<()> {
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{label} exited with status {status}"
        )))
    }
}

fn should_exit(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c' | 'd'))
}

fn should_suspend(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('z'))
}

fn keypress_from_event(key: KeyEvent) -> Option<KeyPress> {
    let rendered = match key.code {
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::PageUp => "pageup".to_string(),
        KeyCode::PageDown => "pagedown".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::BackTab => "shift+tab".to_string(),
        KeyCode::Delete => "delete".to_string(),
        KeyCode::Insert => "insert".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Char(character) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                format!("ctrl+{}", character.to_ascii_lowercase())
            } else {
                character.to_string()
            }
        }
        KeyCode::F(number) => format!("f{number}"),
        KeyCode::Null
        | KeyCode::CapsLock
        | KeyCode::ScrollLock
        | KeyCode::NumLock
        | KeyCode::PrintScreen
        | KeyCode::Pause
        | KeyCode::Menu
        | KeyCode::KeypadBegin
        | KeyCode::Media(_)
        | KeyCode::Modifier(_) => return None,
    };

    Some(KeyPress { key: rendered })
}

fn mouse_input_from_event(column: u16, row: u16, kind: MouseEventKind) -> Option<InputEvent> {
    match kind {
        MouseEventKind::Down(MouseButton::Left) => Some(InputEvent::MouseLeft { column, row }),
        MouseEventKind::ScrollUp => Some(InputEvent::MouseWheelUp { column, row }),
        MouseEventKind::ScrollDown => Some(InputEvent::MouseWheelDown { column, row }),
        _ => None,
    }
}

fn current_timestamp() -> Timestamp {
    Timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyEventState, MediaKeyCode, ModifierKeyCode};

    use super::*;

    #[test]
    fn translates_core_navigation_keys_into_existing_router_strings() {
        assert_eq!(
            keypress_from_event(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }),
            Some(KeyPress {
                key: "enter".to_string(),
            })
        );
        assert_eq!(
            keypress_from_event(KeyEvent {
                code: KeyCode::BackTab,
                modifiers: KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }),
            Some(KeyPress {
                key: "shift+tab".to_string(),
            })
        );
        assert_eq!(
            keypress_from_event(KeyEvent {
                code: KeyCode::Char('P'),
                modifiers: KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }),
            Some(KeyPress {
                key: "P".to_string(),
            })
        );
        assert_eq!(
            keypress_from_event(KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }),
            Some(KeyPress {
                key: "ctrl+r".to_string(),
            })
        );
    }

    #[test]
    fn ignores_non_routable_terminal_keys() {
        assert_eq!(
            keypress_from_event(KeyEvent {
                code: KeyCode::Media(MediaKeyCode::Play),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }),
            None
        );
        assert_eq!(
            keypress_from_event(KeyEvent {
                code: KeyCode::Modifier(ModifierKeyCode::LeftShift),
                modifiers: KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }),
            None
        );
    }

    #[test]
    fn ctrl_c_and_ctrl_d_exit_the_session() {
        assert!(should_exit(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }));
        assert!(should_exit(KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }));
        assert!(!should_exit(KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }));
    }

    #[test]
    fn ctrl_z_requests_terminal_suspend() {
        assert!(should_suspend(KeyEvent {
            code: KeyCode::Char('z'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }));
        assert!(!should_suspend(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }));
    }

    #[test]
    fn maps_mouse_click_and_wheel_events_into_core_input_events() {
        assert_eq!(
            mouse_input_from_event(4, 7, MouseEventKind::Down(MouseButton::Left)),
            Some(InputEvent::MouseLeft { column: 4, row: 7 })
        );
        assert_eq!(
            mouse_input_from_event(8, 3, MouseEventKind::ScrollUp),
            Some(InputEvent::MouseWheelUp { column: 8, row: 3 })
        );
        assert_eq!(
            mouse_input_from_event(9, 2, MouseEventKind::ScrollDown),
            Some(InputEvent::MouseWheelDown { column: 9, row: 2 })
        );
        assert_eq!(mouse_input_from_event(1, 1, MouseEventKind::Moved), None);
    }
}

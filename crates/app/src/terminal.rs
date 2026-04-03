use std::io::{self, IsTerminal, Read, Stdout, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
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
use super_lazygit_core::{
    CredentialStrategy, Event, InputEvent, KeyPress, ShellCommandRequest, TimerEvent, Timestamp,
};

use crate::runtime::AppRuntime;

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const PERIODIC_REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const PERIODIC_FETCH_INTERVAL: Duration = Duration::from_secs(60);
const TOAST_EXPIRY_INTERVAL: Duration = Duration::from_millis(250);

pub fn run(runtime: &mut AppRuntime) -> Result<()> {
    #[cfg(windows)]
    update_window_title()?;
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

pub fn run_external_command_named(command: &mut Command, label: &str) -> io::Result<()> {
    run_external_command_named_with_options(command, label, true, None)
}

pub fn run_shell_command_request(request: &ShellCommandRequest) -> io::Result<()> {
    let (program, argv) = request
        .args()
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "shell command missing argv"))?;

    let mut command = Command::new(program);
    command.args(argv);
    command.current_dir(
        request
            .working_dir
            .clone()
            .unwrap_or_else(|| request.repo_id.0.clone().into()),
    );

    for env_var in request.env_vars() {
        let (key, value) = env_var.split_once('=').unwrap_or((env_var.as_str(), ""));
        command.env(key, value);
    }

    if matches!(request.credential_strategy(), CredentialStrategy::Prompt) {
        if request.stdin.is_none() && io::stdin().is_terminal() && io::stdout().is_terminal() {
            return run_external_command_named_with_options(
                &mut command,
                "shell command",
                true,
                None,
            );
        }

        return Err(io::Error::other(
            "credential prompts require an interactive terminal session",
        ));
    }

    if request.should_stream_output()
        && !request.should_suppress_output_unless_error()
        && matches!(request.credential_strategy(), CredentialStrategy::None)
        && request.stdin.is_none()
    {
        return run_external_command_named_with_options(&mut command, "shell command", true, None);
    }

    let result = match request.credential_strategy() {
        CredentialStrategy::Fail => {
            command.env("LANG", "C");
            command.env("LC_ALL", "C");
            command.env("LC_MESSAGES", "C");
            run_command_with_credential_detection(&mut command, request.stdin.as_deref())?
        }
        CredentialStrategy::None | CredentialStrategy::Prompt => {
            run_command_captured(&mut command, request.stdin.as_deref())?
        }
    };

    require_captured_success(result, "shell command", request.should_ignore_empty_error())
}

pub fn run_external_command_named_with_options(
    command: &mut Command,
    label: &str,
    suspend: bool,
    stdin: Option<&str>,
) -> io::Result<()> {
    if !suspend || !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return run_command_with_optional_stdin(command, stdin)
            .and_then(|status| require_success(status, label));
    }

    suspend_terminal()?;
    let status = run_command_with_optional_stdin(command, stdin);
    let resume = resume_terminal();
    resume?;
    status.and_then(|status| require_success(status, label))
}

fn run_command_with_optional_stdin(
    command: &mut Command,
    stdin: Option<&str>,
) -> io::Result<ExitStatus> {
    match stdin {
        Some(input) => {
            command.stdin(Stdio::piped());
            let mut child = command.spawn()?;
            if let Some(mut child_stdin) = child.stdin.take() {
                child_stdin.write_all(input.as_bytes())?;
            }
            child.wait()
        }
        None => command.status(),
    }
}

#[derive(Debug)]
struct CapturedCommandResult {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    credential_prompt_detected: bool,
}

fn run_command_captured(
    command: &mut Command,
    stdin: Option<&str>,
) -> io::Result<CapturedCommandResult> {
    match stdin {
        Some(input) => {
            command.stdin(Stdio::piped());
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());
            let mut child = command.spawn()?;
            if let Some(mut child_stdin) = child.stdin.take() {
                child_stdin.write_all(input.as_bytes())?;
            }
            let output = child.wait_with_output()?;
            Ok(CapturedCommandResult {
                status: output.status,
                stdout: output.stdout,
                stderr: output.stderr,
                credential_prompt_detected: false,
            })
        }
        None => {
            let output = command.output()?;
            Ok(CapturedCommandResult {
                status: output.status,
                stdout: output.stdout,
                stderr: output.stderr,
                credential_prompt_detected: false,
            })
        }
    }
}

fn run_command_with_credential_detection(
    command: &mut Command,
    stdin: Option<&str>,
) -> io::Result<CapturedCommandResult> {
    #[cfg(unix)]
    command.process_group(0);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn()?;
    if let Some(input) = stdin {
        if let Some(mut child_stdin) = child.stdin.take() {
            child_stdin.write_all(input.as_bytes())?;
        }
    } else {
        let _ = child.stdin.take();
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("failed to open child stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("failed to open child stderr"))?;
    let prompt_detected = Arc::new(AtomicBool::new(false));
    let stdout_thread = spawn_pipe_reader(stdout, Arc::clone(&prompt_detected));
    let stderr_thread = spawn_pipe_reader(stderr, Arc::clone(&prompt_detected));

    loop {
        if prompt_detected.load(Ordering::SeqCst) {
            let _ = terminate_child(&mut child);
            break;
        }
        if child.try_wait()?.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    let status = child.wait()?;
    let stdout = join_pipe_reader(stdout_thread)?;
    let stderr = join_pipe_reader(stderr_thread)?;

    Ok(CapturedCommandResult {
        status,
        stdout,
        stderr,
        credential_prompt_detected: prompt_detected.load(Ordering::SeqCst),
    })
}

fn spawn_pipe_reader<R>(
    mut reader: R,
    prompt_detected: Arc<AtomicBool>,
) -> thread::JoinHandle<io::Result<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut output = Vec::new();
        let mut detector = CredentialPromptDetector::default();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            let chunk = &buffer[..read];
            output.extend_from_slice(chunk);
            if detector.push(chunk) {
                prompt_detected.store(true, Ordering::SeqCst);
            }
        }
        Ok(output)
    })
}

fn join_pipe_reader(handle: thread::JoinHandle<io::Result<Vec<u8>>>) -> io::Result<Vec<u8>> {
    handle
        .join()
        .map_err(|_| io::Error::other("pipe reader panicked"))?
}

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[cfg(unix)]
fn terminate_child(child: &mut std::process::Child) -> io::Result<()> {
    const SIGKILL: i32 = 9;
    let result = unsafe { kill(-(child.id() as i32), SIGKILL) };
    if result == 0 {
        Ok(())
    } else {
        child.kill()
    }
}

#[cfg(not(unix))]
fn terminate_child(child: &mut std::process::Child) -> io::Result<()> {
    child.kill()
}

fn require_captured_success(
    result: CapturedCommandResult,
    label: &str,
    ignore_empty_error: bool,
) -> io::Result<()> {
    if result.status.success() {
        return Ok(());
    }

    if result.credential_prompt_detected {
        return Err(io::Error::other(format!(
            "{label} requested credentials and was terminated"
        )));
    }

    let stdout = String::from_utf8_lossy(&result.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
    if ignore_empty_error && stdout.is_empty() && stderr.is_empty() {
        return Ok(());
    }

    if !stderr.is_empty() {
        return Err(io::Error::other(stderr));
    }
    if !stdout.is_empty() {
        return Err(io::Error::other(stdout));
    }

    require_success(result.status, label)
}

#[derive(Default)]
struct CredentialPromptDetector {
    buffer: String,
}

impl CredentialPromptDetector {
    fn push(&mut self, bytes: &[u8]) -> bool {
        let chunk = String::from_utf8_lossy(bytes).to_lowercase();
        self.buffer.push_str(&chunk);
        if self.buffer.len() > 512 {
            let truncate_to = self.buffer.len() - 512;
            self.buffer.drain(..truncate_to);
        }

        if self
            .buffer
            .split('\n')
            .next_back()
            .is_some_and(is_credential_prompt)
        {
            self.buffer.clear();
            return true;
        }

        false
    }
}

fn is_credential_prompt(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains("password:")
        || trimmed.contains("'s password:")
        || trimmed.contains("password for '")
        || trimmed.contains("username for '")
        || trimmed.contains("enter passphrase for key '")
        || trimmed.contains("enter pin for")
        || trimmed.contains("2fa token")
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

#[cfg(windows)]
fn update_window_title() -> io::Result<()> {
    use crossterm::terminal::SetTitle;

    let cwd = std::env::current_dir()?;
    execute!(io::stdout(), SetTitle(window_title_for_path(&cwd)))
}

#[cfg_attr(not(windows), allow(dead_code))]
fn window_title_for_path(path: &Path) -> String {
    let base = path
        .file_name()
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    format!("{base} - Lazygit")
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crossterm::event::{KeyEventState, MediaKeyCode, ModifierKeyCode};
    use super_lazygit_core::{JobId, RepoId, ShellCommandRequest};

    use super::*;

    #[derive(Clone)]
    struct ShellCommandMatcher {
        description: String,
        test: Arc<dyn Fn(&ShellCommandRequest) -> bool + Send + Sync>,
        output: String,
        err: Option<String>,
    }

    #[derive(Clone, Default)]
    struct FakeShellCommandRunner {
        state: Arc<Mutex<FakeShellCommandRunnerState>>,
    }

    #[derive(Default)]
    struct FakeShellCommandRunnerState {
        expected: Vec<ShellCommandMatcher>,
        invoked_indexes: Vec<usize>,
    }

    impl FakeShellCommandRunner {
        fn expect_func<F>(
            &self,
            description: impl Into<String>,
            matcher: F,
            output: impl Into<String>,
            err: Option<&str>,
        ) -> &Self
        where
            F: Fn(&ShellCommandRequest) -> bool + Send + Sync + 'static,
        {
            let mut state = self.state.lock().expect("fake shell runner state");
            state.expected.push(ShellCommandMatcher {
                description: description.into(),
                test: Arc::new(matcher),
                output: output.into(),
                err: err.map(str::to_string),
            });
            self
        }

        fn expect_args(
            &self,
            expected_args: &[&str],
            output: impl Into<String>,
            err: Option<&str>,
        ) -> &Self {
            let expected = expected_args
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>();
            let description = format!("matches args {}", expected.join(" "));
            self.expect_func(
                description,
                move |request| request.args() == expected.as_slice(),
                output,
                err,
            )
        }

        fn expect_shell_args(
            &self,
            expected_shell_body: &str,
            output: impl Into<String>,
            err: Option<&str>,
        ) -> &Self {
            let expected = expected_shell_body.to_string();
            self.expect_func(
                format!("matches shell body {expected}"),
                move |request| request.command == expected,
                output,
                err,
            )
        }

        fn remaining_expected_descriptions(&self) -> Vec<String> {
            let state = self.state.lock().expect("fake shell runner state");
            state
                .expected
                .iter()
                .enumerate()
                .filter(|(index, _)| !state.invoked_indexes.contains(index))
                .map(|(_, matcher)| matcher.description.clone())
                .collect()
        }

        fn run_with_output(&self, request: &ShellCommandRequest) -> io::Result<String> {
            let mut state = self.state.lock().expect("fake shell runner state");
            let matched_index = state
                .expected
                .iter()
                .enumerate()
                .find_map(|(index, matcher)| {
                    if state.invoked_indexes.contains(&index) {
                        return None;
                    }
                    if (matcher.test)(request) {
                        Some(index)
                    } else {
                        None
                    }
                });

            if let Some(index) = matched_index {
                let matcher = state.expected[index].clone();
                state.invoked_indexes.push(index);
                return match matcher.err {
                    Some(message) => Err(io::Error::other(message)),
                    None => Ok(matcher.output),
                };
            }

            Err(io::Error::other(format!(
                "unexpected command: {}",
                request.command
            )))
        }

        fn run(&self, request: &ShellCommandRequest) -> io::Result<()> {
            self.run_with_output(request).map(|_| ())
        }

        fn run_and_process_lines<F>(
            &self,
            request: &ShellCommandRequest,
            mut on_line: F,
        ) -> io::Result<()>
        where
            F: FnMut(&str) -> io::Result<bool>,
        {
            let output = self.run_with_output(request)?;
            for line in output.lines() {
                if on_line(line)? {
                    break;
                }
            }
            Ok(())
        }

        fn check_for_missing_calls(&self) -> io::Result<()> {
            let remaining = self.remaining_expected_descriptions();
            if remaining.is_empty() {
                Ok(())
            } else {
                Err(io::Error::other(format!(
                    "expected {} more command(s) to be run. Remaining commands:\n{}",
                    remaining.len(),
                    remaining.join("\n")
                )))
            }
        }
    }

    fn dummy_shell_request(command: &str) -> ShellCommandRequest {
        ShellCommandRequest::new(
            JobId::new("shell:/tmp/repo:fake-runner"),
            RepoId::new("/tmp/repo"),
            command,
        )
    }

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

    #[test]
    fn window_title_for_path_uses_leaf_directory_name() {
        assert_eq!(
            window_title_for_path(Path::new("/tmp/workspace/repo-a")),
            "repo-a - Lazygit"
        );
        assert_eq!(window_title_for_path(Path::new("/")), "/ - Lazygit");
    }

    #[test]
    fn fake_shell_runner_matches_unordered_expectations_and_returns_output() {
        let runner = FakeShellCommandRunner::default();
        runner
            .expect_args(&["sh", "-lc", "printf second"], "second\n", None)
            .expect_args(&["sh", "-lc", "printf first"], "first\n", None);

        let second = ShellCommandRequest::from_args(
            JobId::new("shell:/tmp/repo:second"),
            RepoId::new("/tmp/repo"),
            "sh",
            ["-lc", "printf second"],
        );
        let first = ShellCommandRequest::from_args(
            JobId::new("shell:/tmp/repo:first"),
            RepoId::new("/tmp/repo"),
            "sh",
            ["-lc", "printf first"],
        );

        assert_eq!(
            runner.run_with_output(&second).expect("second output"),
            "second\n"
        );
        assert_eq!(
            runner.run_with_output(&first).expect("first output"),
            "first\n"
        );
        runner
            .check_for_missing_calls()
            .expect("all calls consumed");
    }

    #[test]
    fn fake_shell_runner_replays_lines_and_stops_when_callback_requests() {
        let runner = FakeShellCommandRunner::default();
        runner.expect_shell_args(
            "printf line1\\nline2\\nline3",
            "line1\nline2\nline3\n",
            None,
        );
        let request = dummy_shell_request("printf line1\\nline2\\nline3");
        let mut seen = Vec::new();

        runner
            .run_and_process_lines(&request, |line| {
                seen.push(line.to_string());
                Ok(line == "line2")
            })
            .expect("line processing");

        assert_eq!(seen, vec!["line1".to_string(), "line2".to_string()]);
        runner
            .check_for_missing_calls()
            .expect("all calls consumed");
    }

    #[test]
    fn fake_shell_runner_reports_errors_and_missing_calls() {
        let runner = FakeShellCommandRunner::default();
        runner
            .expect_args(&["git", "status"], "", Some("status failed"))
            .expect_shell_args("printf later", "later\n", None);

        let failing = ShellCommandRequest::from_args(
            JobId::new("shell:/tmp/repo:status"),
            RepoId::new("/tmp/repo"),
            "git",
            ["status"],
        )
        .fail_on_credential_request();

        let error = runner
            .run_with_output(&failing)
            .expect_err("configured failure");
        assert_eq!(error.to_string(), "status failed");

        let missing = runner
            .check_for_missing_calls()
            .expect_err("missing call should fail");
        assert!(missing.to_string().contains("expected 1 more command(s)"));
        assert!(missing
            .to_string()
            .contains("matches shell body printf later"));
    }

    #[test]
    fn fake_shell_runner_run_uses_registered_expectation_without_output() {
        let runner = FakeShellCommandRunner::default();
        runner.expect_shell_args("printf ok", "ok\n", None);

        runner
            .run(&dummy_shell_request("printf ok"))
            .expect("runner should accept matching command");
        runner
            .check_for_missing_calls()
            .expect("all calls consumed");
    }
}

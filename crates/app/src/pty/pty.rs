// Ported from ./references/lazygit-master/pkg/gui/pty.go

pub struct Winsize {
    pub cols: u16,
    pub rows: u16,
}

pub fn desired_pty_size(width: u16, height: u16) -> Winsize {
    Winsize {
        cols: width,
        rows: height,
    }
}

pub fn remove_existing_term_env_vars(env: &[String]) -> Vec<String> {
    env.iter()
        .filter(|v| !is_term_env_var(v))
        .cloned()
        .collect()
}

pub fn is_term_env_var(env_var: &str) -> bool {
    env_var.starts_with("TERM=")
        || env_var.starts_with("TERM_PROGRAM=")
        || env_var.starts_with("TERM_PROGRAM_VERSION=")
        || env_var.starts_with("TERMINAL_EMULATOR=")
        || env_var.starts_with("TERMINAL_NAME=")
        || env_var.starts_with("TERMINAL_VERSION_")
}

// Windows-specific stub (pty_windows.go parity)
pub fn windows_pty_resize(_width: u16, _height: u16) -> bool {
    true
}

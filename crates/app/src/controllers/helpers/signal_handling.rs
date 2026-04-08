// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/signal_handling.go


#[cfg(not(windows))]
pub fn can_suspend_app() -> bool {
    true
}

#[cfg(not(windows))]
pub fn send_stop_signal() -> Result<(), String> {
    Err("SIGSTOP not supported".to_string())
}

#[cfg(not(windows))]
pub fn set_foreground_pgrp() -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn handle_resume_signal<F>(on_resume: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    on_resume()
}

#[cfg(not(windows))]
pub fn install_resume_signal_handler<F>(_on_resume: F)
where
    F: FnOnce() -> Result<(), String> + Send + 'static,
{
}

#[cfg(windows)]
pub fn can_suspend_app() -> bool {
    false
}

#[cfg(windows)]
pub fn send_stop_signal() -> Result<(), String> {
    Err("SIGSTOP not supported on Windows".to_string())
}

#[cfg(windows)]
pub fn set_foreground_pgrp() -> Result<(), String> {
    Err("Not supported on Windows".to_string())
}

#[cfg(windows)]
pub fn handle_resume_signal<F>(on_resume: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    on_resume()
}

#[cfg(windows)]
pub fn install_resume_signal_handler<F>(on_resume: F)
where
    F: FnOnce() -> Result<(), String> + Send + 'static,
{
}

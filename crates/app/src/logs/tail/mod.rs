pub mod tail;

#[cfg(target_os = "windows")]
pub mod logs_windows;

#[cfg(not(target_os = "windows"))]
pub mod logs_default;

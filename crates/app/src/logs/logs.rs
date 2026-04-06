// Ported from ./references/lazygit-master/pkg/logs/logs.go

pub struct Logger;

impl Logger {
    pub fn new_production_logger() -> Self {
        Self
    }

    pub fn new_development_logger(_log_path: &str) -> Self {
        Self
    }
}

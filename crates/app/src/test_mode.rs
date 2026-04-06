// Ported from ./references/lazygit-master/pkg/gui/test_mode.go

pub struct IntegrationTest;

pub fn handle_test_mode(_test: Option<&IntegrationTest>) -> bool {
    if std::env::var("SANDBOX_VAR").ok().is_some() {
        return false;
    }
    false
}

pub fn headless() -> bool {
    std::env::var("LAZYGIT_HEADLESS").is_ok()
}

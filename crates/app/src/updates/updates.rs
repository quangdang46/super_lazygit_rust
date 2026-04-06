// Ported from ./references/lazygit-master/pkg/updates/updates.go

pub struct Updater;

impl Updater {
    pub fn new() -> Self {
        Self
    }

    pub fn check_for_new_update(&self) -> Result<String, String> {
        Ok(String::new())
    }

    pub fn update(&self, _new_version: &str) -> Result<(), String> {
        Ok(())
    }
}

impl Default for Updater {
    fn default() -> Self {
        Self::new()
    }
}

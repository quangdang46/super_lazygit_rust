// Ported from ./references/lazygit-master/pkg/common/common.go

use std::sync::Mutex;

use super_lazygit_config::AppConfig;
use super_lazygit_core::state::AppState;

use crate::i18n::TranslationSet;

pub struct Common {
    pub log: Box<dyn log::Log>,
    pub tr: TranslationSet,
    user_config: Mutex<Box<AppConfig>>,
    pub app_state: AppState,
    pub debug: bool,
}

impl Common {
    pub fn user_config(&self) -> AppConfig {
        (*self.user_config.lock().unwrap()).clone()
    }

    pub fn set_user_config(&self, user_config: AppConfig) {
        *self.user_config.lock().unwrap() = Box::new(user_config);
    }
}

// Ported from ./references/lazygit-master/pkg/common/dummies.go

use std::sync::Mutex;

use super_lazygit_config::AppConfig;
use super_lazygit_core::state::AppState;

use crate::common::Common;
use crate::i18n::TranslationSet;

pub fn new_dummy_common() -> Common {
    let tr = TranslationSet::english();
    Common {
        log: Box::new(DummyLogger),
        tr,
        user_config: Mutex::new(Box::new(AppConfig::default())),
        app_state: AppState::default(),
        debug: false,
    }
}

pub fn new_dummy_common_with_user_config_and_app_state(
    user_config: AppConfig,
    app_state: AppState,
) -> Common {
    let tr = TranslationSet::english();
    Common {
        log: Box::new(DummyLogger),
        tr,
        user_config: Mutex::new(Box::new(user_config)),
        app_state,
        debug: false,
    }
}

struct DummyLogger;

impl log::Log for DummyLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        false
    }

    fn log(&self, _record: &log::Record) {}

    fn flush(&self) {}
}

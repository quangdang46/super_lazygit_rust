use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct CustomCommandsSubmenuWithSpecialKeybindingsTest;

impl CustomCommandsSubmenuWithSpecialKeybindingsTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Using custom commands from a custom commands menu with keybindings that conflict with builtin ones".to_string(),
            extra_cmd_args: vec![],
            skip: false,
            setup_repo: Some(Box::new(|_shell| {})),
            setup_config: Some(Box::new(|_cfg| {})),
            run: Some(Box::new(|_t, _keys| {})),
            extra_env_vars: std::collections::HashMap::new(),
            git_version: Default::default(),
            width: 0,
            height: 0,
            is_demo: false,
        })
    }
}

pub const CUSTOM_COMMANDS_SUBMENU_WITH_SPECIAL_KEYBINDINGS:
    CustomCommandsSubmenuWithSpecialKeybindingsTest =
    CustomCommandsSubmenuWithSpecialKeybindingsTest;

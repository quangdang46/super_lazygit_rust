use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct FilterMenuWithNoKeybindingsTest;

impl FilterMenuWithNoKeybindingsTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description:
                "Filtering the keybindings menu so that only entries without keybinding are left"
                    .to_string(),
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

pub const FILTER_MENU_WITH_NO_KEYBINDINGS: FilterMenuWithNoKeybindingsTest =
    FilterMenuWithNoKeybindingsTest;

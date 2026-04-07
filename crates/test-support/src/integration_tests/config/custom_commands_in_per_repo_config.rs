use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct CustomCommandsInPerRepoConfigTest;

impl CustomCommandsInPerRepoConfigTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Custom commands in per-repo config add to the global ones instead of replacing them".to_string(),
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

pub const CUSTOM_COMMANDS_IN_PER_REPO_CONFIG: CustomCommandsInPerRepoConfigTest =
    CustomCommandsInPerRepoConfigTest;

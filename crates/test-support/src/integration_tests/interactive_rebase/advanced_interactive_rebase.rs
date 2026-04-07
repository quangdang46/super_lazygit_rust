use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct AdvancedInteractiveRebaseTest;

impl AdvancedInteractiveRebaseTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "It begins an interactive rebase and verifies to have the possibility of editing the commits of the branch before proceeding with the actual rebase".to_string(),
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

pub const ADVANCED_INTERACTIVE_REBASE: AdvancedInteractiveRebaseTest =
    AdvancedInteractiveRebaseTest;

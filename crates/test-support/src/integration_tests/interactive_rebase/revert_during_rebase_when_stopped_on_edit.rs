use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct RevertDuringRebaseWhenStoppedOnEditTest;

impl RevertDuringRebaseWhenStoppedOnEditTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Revert a series of commits while stopped in a rebase".to_string(),
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

pub const REVERT_DURING_REBASE_WHEN_STOPPED_ON_EDIT: RevertDuringRebaseWhenStoppedOnEditTest =
    RevertDuringRebaseWhenStoppedOnEditTest;

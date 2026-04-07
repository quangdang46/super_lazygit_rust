use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct MoveInRebaseTest;

impl MoveInRebaseTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Via a single interactive rebase move a commit all the way up then back down then slightly back up again and apply the change".to_string(),
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

pub const MOVE_IN_REBASE: MoveInRebaseTest = MoveInRebaseTest;

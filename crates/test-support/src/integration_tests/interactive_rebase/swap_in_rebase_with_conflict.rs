use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct SwapInRebaseWithConflictTest;

impl SwapInRebaseWithConflictTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Via an edit-triggered rebase, swap two commits, causing a conflict. Then resolve the conflict and continue".to_string(),
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

pub const SWAP_IN_REBASE_WITH_CONFLICT: SwapInRebaseWithConflictTest = SwapInRebaseWithConflictTest;

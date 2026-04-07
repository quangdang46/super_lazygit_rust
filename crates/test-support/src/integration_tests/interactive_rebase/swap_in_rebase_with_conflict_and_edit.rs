use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct SwapInRebaseWithConflictAndEditTest;

impl SwapInRebaseWithConflictAndEditTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Via an edit-triggered rebase, swap two commits, causing a conflict, then edit the commit that will conflict.".to_string(),
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

pub const SWAP_IN_REBASE_WITH_CONFLICT_AND_EDIT: SwapInRebaseWithConflictAndEditTest =
    SwapInRebaseWithConflictAndEditTest;

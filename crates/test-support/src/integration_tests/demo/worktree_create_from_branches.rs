use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct WorktreeCreateFromBranchesTest;

impl WorktreeCreateFromBranchesTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Create a worktree from the branches view".to_string(),
            extra_cmd_args: vec![],
            skip: false,
            setup_repo: Some(Box::new(|_shell| {})),
            setup_config: Some(Box::new(|_cfg| {})),
            run: Some(Box::new(|_t, _keys| {})),
            extra_env_vars: std::collections::HashMap::new(),
            git_version: Default::default(),
            width: 0,
            height: 0,
            is_demo: true,
        })
    }
}

pub const WORKTREE_CREATE_FROM_BRANCHES: WorktreeCreateFromBranchesTest =
    WorktreeCreateFromBranchesTest;

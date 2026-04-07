use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct ForceRemoveWorktreeTest;

impl ForceRemoveWorktreeTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Force remove a dirty worktree".to_string(),
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

pub const FORCE_REMOVE_WORKTREE: ForceRemoveWorktreeTest = ForceRemoveWorktreeTest;

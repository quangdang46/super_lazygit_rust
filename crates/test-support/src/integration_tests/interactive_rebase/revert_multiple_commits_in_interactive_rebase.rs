use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct RevertMultipleCommitsInInteractiveRebaseTest;

impl RevertMultipleCommitsInInteractiveRebaseTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Reverts a range of commits, the first of which conflicts, in the middle of an interactive rebase".to_string(),
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

pub const REVERT_MULTIPLE_COMMITS_IN_INTERACTIVE_REBASE:
    RevertMultipleCommitsInInteractiveRebaseTest = RevertMultipleCommitsInInteractiveRebaseTest;

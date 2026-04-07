use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct RevertSingleCommitInInteractiveRebaseTest;

impl RevertSingleCommitInInteractiveRebaseTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Reverts a commit that conflicts in the middle of an interactive rebase"
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

pub const REVERT_SINGLE_COMMIT_IN_INTERACTIVE_REBASE: RevertSingleCommitInInteractiveRebaseTest =
    RevertSingleCommitInInteractiveRebaseTest;

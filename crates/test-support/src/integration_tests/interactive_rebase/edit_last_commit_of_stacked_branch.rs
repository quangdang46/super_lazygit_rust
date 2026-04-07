use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct EditLastCommitOfStackedBranchTest;

impl EditLastCommitOfStackedBranchTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Edit and amend the last commit of a branch in a stack of branches, and ensure that it doesn't break the stack".to_string(),
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

pub const EDIT_LAST_COMMIT_OF_STACKED_BRANCH: EditLastCommitOfStackedBranchTest =
    EditLastCommitOfStackedBranchTest;

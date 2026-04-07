use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct CreateFixupCommitInBranchStackTest;

impl CreateFixupCommitInBranchStackTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Create a fixup commit in a stack of branches, verify that it is created at the end of the branch it belongs to".to_string(),
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

pub const CREATE_FIXUP_COMMIT_IN_BRANCH_STACK: CreateFixupCommitInBranchStackTest =
    CreateFixupCommitInBranchStackTest;

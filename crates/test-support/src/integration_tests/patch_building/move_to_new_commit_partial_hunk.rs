use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct MoveToNewCommitPartialHunkTest;

impl MoveToNewCommitPartialHunkTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description:
                "Move a patch from a commit to a new commit, with only parts of a hunk in the patch"
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

pub const MOVE_TO_NEW_COMMIT_PARTIAL_HUNK: MoveToNewCommitPartialHunkTest =
    MoveToNewCommitPartialHunkTest;

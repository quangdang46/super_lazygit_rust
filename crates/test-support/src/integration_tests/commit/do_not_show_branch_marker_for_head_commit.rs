use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct DoNotShowBranchMarkerForHeadCommitTest;

impl DoNotShowBranchMarkerForHeadCommitTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Verify that no branch heads are shown for the branch head if there is a tag with the same name as the branch".to_string(),
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

pub const DO_NOT_SHOW_BRANCH_MARKER_FOR_HEAD_COMMIT: DoNotShowBranchMarkerForHeadCommitTest =
    DoNotShowBranchMarkerForHeadCommitTest;

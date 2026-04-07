use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct EditRangeSelectDownToMergeOutsideRebaseTest;

impl EditRangeSelectDownToMergeOutsideRebaseTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Select a range of commits (the last one being a merge commit) to edit outside of a rebase".to_string(),
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

pub const EDIT_RANGE_SELECT_DOWN_TO_MERGE_OUTSIDE_REBASE:
    EditRangeSelectDownToMergeOutsideRebaseTest = EditRangeSelectDownToMergeOutsideRebaseTest;

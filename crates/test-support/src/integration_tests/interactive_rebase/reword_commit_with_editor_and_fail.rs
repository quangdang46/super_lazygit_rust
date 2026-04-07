use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct RewordCommitWithEditorAndFailTest;

impl RewordCommitWithEditorAndFailTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description:
                "Rewords a commit with editor, and fails because an empty commit message is given"
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

pub const REWORD_COMMIT_WITH_EDITOR_AND_FAIL: RewordCommitWithEditorAndFailTest =
    RewordCommitWithEditorAndFailTest;

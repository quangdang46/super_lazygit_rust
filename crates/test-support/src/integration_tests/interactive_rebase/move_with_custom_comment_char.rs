use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct MoveWithCustomCommentCharTest;

impl MoveWithCustomCommentCharTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Directly moves a commit down and back up with the 'core.commentChar' option set to a custom character".to_string(),
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

pub const MOVE_WITH_CUSTOM_COMMENT_CHAR: MoveWithCustomCommentCharTest =
    MoveWithCustomCommentCharTest;

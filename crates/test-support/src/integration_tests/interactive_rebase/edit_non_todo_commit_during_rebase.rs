use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct EditNonTodoCommitDuringRebaseTest;

impl EditNonTodoCommitDuringRebaseTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Tries to edit a non-todo commit while already rebasing, resulting in an error message".to_string(),
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

pub const EDIT_NON_TODO_COMMIT_DURING_REBASE: EditNonTodoCommitDuringRebaseTest =
    EditNonTodoCommitDuringRebaseTest;

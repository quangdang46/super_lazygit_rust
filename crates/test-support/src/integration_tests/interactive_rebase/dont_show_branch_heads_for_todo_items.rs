use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct DontShowBranchHeadsForTodoItemsTest;

impl DontShowBranchHeadsForTodoItemsTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Check that branch heads are shown for normal commits during interactive rebase, but not for todo items".to_string(),
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

pub const DONT_SHOW_BRANCH_HEADS_FOR_TODO_ITEMS: DontShowBranchHeadsForTodoItemsTest =
    DontShowBranchHeadsForTodoItemsTest;

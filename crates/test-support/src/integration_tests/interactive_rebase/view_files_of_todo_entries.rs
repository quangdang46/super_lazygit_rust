use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct ViewFilesOfTodoEntriesTest;

impl ViewFilesOfTodoEntriesTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Check that files of a pick todo can be viewed, but files of an update-ref todo can't".to_string(),
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

pub const VIEW_FILES_OF_TODO_ENTRIES: ViewFilesOfTodoEntriesTest = ViewFilesOfTodoEntriesTest;

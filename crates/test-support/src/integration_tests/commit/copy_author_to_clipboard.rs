use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct CopyAuthorToClipboardTest;

impl CopyAuthorToClipboardTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Copy a commit author name to the clipboard".to_string(),
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

pub const COPY_AUTHOR_TO_CLIPBOARD: CopyAuthorToClipboardTest = CopyAuthorToClipboardTest;

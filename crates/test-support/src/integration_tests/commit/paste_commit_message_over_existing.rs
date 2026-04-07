use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct PasteCommitMessageOverExistingTest;

impl PasteCommitMessageOverExistingTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Paste a commit message into the commit message panel when there is already text in the panel, causing a confirmation".to_string(),
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

pub const PASTE_COMMIT_MESSAGE_OVER_EXISTING: PasteCommitMessageOverExistingTest =
    PasteCommitMessageOverExistingTest;

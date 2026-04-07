//! Daemon functionality for lazygit.
//!
//! Sometimes lazygit will be invoked in daemon mode from a parent lazygit process.
//! We do this when git lets us supply a program to run within a git command.
//! For example, if we want to ensure that a git command doesn't hang due to
//! waiting for an editor to save a commit message, we can tell git to invoke lazygit
//! as the editor via 'GIT_EDITOR=lazygit', and use the env var
//! 'LAZYGIT_DAEMON_KIND=1' (exit immediately) to specify that we want to run lazygit
//! as a daemon which simply exits immediately.
//!
//! 'Daemon' is not the best name for this, because it's not a persistent background
//! process, but it's close enough.

use serde::{Deserialize, Serialize};
use std::env;
use std::process::Command;

/// Environment variable key for daemon kind
pub const DAEMON_KIND_ENV_KEY: &str = "LAZYGIT_DAEMON_KIND";

/// Environment variable key for daemon instruction JSON data
pub const DAEMON_INSTRUCTION_ENV_KEY: &str = "LAZYGIT_DAEMON_INSTRUCTION";

/// DaemonKind represents the type of daemon instruction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonKind {
    /// For when we fail to parse the daemon kind
    Unknown = 0,
    /// Exit immediately
    ExitImmediately = 1,
    /// Remove update refs for copied branch
    RemoveUpdateRefsForCopiedBranch = 2,
    /// Move todos up
    MoveTodosUp = 3,
    /// Move todos down
    MoveTodosDown = 4,
    /// Insert break
    InsertBreak = 5,
    /// Change todo actions
    ChangeTodoActions = 6,
    /// Drop merge commit
    DropMergeCommit = 7,
    /// Move fixup commit down
    MoveFixupCommitDown = 8,
    /// Write rebase todo
    WriteRebaseTodo = 9,
}

impl DaemonKind {
    /// Parse daemon kind from environment variable value
    fn from_env_value(value: &str) -> Self {
        match value.parse::<i32>().ok() {
            Some(1) => DaemonKind::ExitImmediately,
            Some(2) => DaemonKind::RemoveUpdateRefsForCopiedBranch,
            Some(3) => DaemonKind::MoveTodosUp,
            Some(4) => DaemonKind::MoveTodosDown,
            Some(5) => DaemonKind::InsertBreak,
            Some(6) => DaemonKind::ChangeTodoActions,
            Some(7) => DaemonKind::DropMergeCommit,
            Some(8) => DaemonKind::MoveFixupCommitDown,
            Some(9) => DaemonKind::WriteRebaseTodo,
            _ => DaemonKind::Unknown,
        }
    }
}

/// Get the current daemon kind from environment
pub fn get_daemon_kind() -> DaemonKind {
    DaemonKind::from_env_value(&env::var(DAEMON_KIND_ENV_KEY).unwrap_or_default())
}

/// Check if we're running in daemon mode
pub fn in_daemon_mode() -> bool {
    get_daemon_kind() != DaemonKind::Unknown
}

/// Instruction is a command to be run by lazygit in daemon mode.
/// It is serialized to json and passed to lazygit via environment variables.
pub trait Instruction: Send {
    /// Returns the daemon kind for this instruction
    fn kind(&self) -> DaemonKind;

    /// Returns the serialized instruction data as JSON
    fn serialized_instructions(&self) -> String;

    /// Runs the instruction
    fn run(&self) -> anyhow::Result<()>;
}

/// ExitImmediatelyInstruction - exits immediately without doing anything
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitImmediatelyInstruction;

impl ExitImmediatelyInstruction {
    pub fn new() -> Self {
        Self
    }
}

impl Instruction for ExitImmediatelyInstruction {
    fn kind(&self) -> DaemonKind {
        DaemonKind::ExitImmediately
    }

    fn serialized_instructions(&self) -> String {
        serialize_instruction(self)
    }

    fn run(&self) -> anyhow::Result<()> {
        // No-op: exit immediately
        Ok(())
    }
}

/// RemoveUpdateRefsForCopiedBranchInstruction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveUpdateRefsForCopiedBranchInstruction;

impl RemoveUpdateRefsForCopiedBranchInstruction {
    pub fn new() -> Self {
        Self
    }
}

impl Instruction for RemoveUpdateRefsForCopiedBranchInstruction {
    fn kind(&self) -> DaemonKind {
        DaemonKind::RemoveUpdateRefsForCopiedBranch
    }

    fn serialized_instructions(&self) -> String {
        serialize_instruction(self)
    }

    fn run(&self) -> anyhow::Result<()> {
        // TODO: Implement handle_interactive_rebase
        Ok(())
    }
}

/// ChangeTodoActionsInstruction - changes todo actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeTodoActionsInstruction {
    pub changes: Vec<ChangeTodoAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeTodoAction {
    pub hash: String,
    pub new_action: String,
    pub flag: String,
}

impl ChangeTodoActionsInstruction {
    pub fn new(changes: Vec<ChangeTodoAction>) -> Self {
        Self { changes }
    }
}

impl Instruction for ChangeTodoActionsInstruction {
    fn kind(&self) -> DaemonKind {
        DaemonKind::ChangeTodoActions
    }

    fn serialized_instructions(&self) -> String {
        serialize_instruction(self)
    }

    fn run(&self) -> anyhow::Result<()> {
        // TODO: Implement handle_interactive_rebase with changes
        Ok(())
    }
}

/// DropMergeCommitInstruction - drops a merge commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropMergeCommitInstruction {
    pub hash: String,
}

impl DropMergeCommitInstruction {
    pub fn new(hash: String) -> Self {
        Self { hash }
    }
}

impl Instruction for DropMergeCommitInstruction {
    fn kind(&self) -> DaemonKind {
        DaemonKind::DropMergeCommit
    }

    fn serialized_instructions(&self) -> String {
        serialize_instruction(self)
    }

    fn run(&self) -> anyhow::Result<()> {
        // TODO: Implement drop merge commit
        Ok(())
    }
}

/// MoveFixupCommitDownInstruction - moves a fixup commit down to right after the original commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveFixupCommitDownInstruction {
    pub original_hash: String,
    pub fixup_hash: String,
    pub change_to_fixup: bool,
}

impl MoveFixupCommitDownInstruction {
    pub fn new(original_hash: String, fixup_hash: String, change_to_fixup: bool) -> Self {
        Self {
            original_hash,
            fixup_hash,
            change_to_fixup,
        }
    }
}

impl Instruction for MoveFixupCommitDownInstruction {
    fn kind(&self) -> DaemonKind {
        DaemonKind::MoveFixupCommitDown
    }

    fn serialized_instructions(&self) -> String {
        serialize_instruction(self)
    }

    fn run(&self) -> anyhow::Result<()> {
        // TODO: Implement move fixup commit down
        Ok(())
    }
}

/// MoveTodosUpInstruction - moves todos up in the rebase todo list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveTodosUpInstruction {
    pub hashes: Vec<String>,
}

impl MoveTodosUpInstruction {
    pub fn new(hashes: Vec<String>) -> Self {
        Self { hashes }
    }
}

impl Instruction for MoveTodosUpInstruction {
    fn kind(&self) -> DaemonKind {
        DaemonKind::MoveTodosUp
    }

    fn serialized_instructions(&self) -> String {
        serialize_instruction(self)
    }

    fn run(&self) -> anyhow::Result<()> {
        // TODO: Implement move todos up
        Ok(())
    }
}

/// MoveTodosDownInstruction - moves todos down in the rebase todo list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveTodosDownInstruction {
    pub hashes: Vec<String>,
}

impl MoveTodosDownInstruction {
    pub fn new(hashes: Vec<String>) -> Self {
        Self { hashes }
    }
}

impl Instruction for MoveTodosDownInstruction {
    fn kind(&self) -> DaemonKind {
        DaemonKind::MoveTodosDown
    }

    fn serialized_instructions(&self) -> String {
        serialize_instruction(self)
    }

    fn run(&self) -> anyhow::Result<()> {
        // TODO: Implement move todos down
        Ok(())
    }
}

/// InsertBreakInstruction - inserts a break in the rebase todo list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertBreakInstruction;

impl InsertBreakInstruction {
    pub fn new() -> Self {
        Self
    }
}

impl Instruction for InsertBreakInstruction {
    fn kind(&self) -> DaemonKind {
        DaemonKind::InsertBreak
    }

    fn serialized_instructions(&self) -> String {
        serialize_instruction(self)
    }

    fn run(&self) -> anyhow::Result<()> {
        // TODO: Implement insert break
        Ok(())
    }
}

/// WriteRebaseTodoInstruction - writes content to the rebase todo file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteRebaseTodoInstruction {
    pub todos_file_content: Vec<u8>,
}

impl WriteRebaseTodoInstruction {
    pub fn new(todos_file_content: Vec<u8>) -> Self {
        Self {
            todos_file_content,
        }
    }
}

impl Instruction for WriteRebaseTodoInstruction {
    fn kind(&self) -> DaemonKind {
        DaemonKind::WriteRebaseTodo
    }

    fn serialized_instructions(&self) -> String {
        serialize_instruction(self)
    }

    fn run(&self) -> anyhow::Result<()> {
        // TODO: Implement write rebase todo
        Ok(())
    }
}

/// Serialize an instruction to JSON string
fn serialize_instruction<T: Serialize>(instruction: &T) -> String {
    serde_json::to_string(instruction).expect("serialize instruction to JSON")
}

/// Deserialize an instruction from JSON string
fn deserialize_instruction<'a, T: Deserialize<'a>>(json_data: &'a str) -> T {
    serde_json::from_str(json_data).expect("deserialize instruction from JSON")
}

/// Get the comment character for git config
fn get_comment_char() -> char {
    let output = Command::new("git")
        .args(["config", "--get", "--null", "core.commentChar"])
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.len() == 2 {
            return stdout.chars().next().unwrap_or('#');
        }
    }

    '#'
}

/// Convert instruction to environment variables for passing to a subprocess
pub fn to_env_vars(instruction: &dyn Instruction) -> Vec<(String, String)> {
    vec![
        (DAEMON_KIND_ENV_KEY.to_string(), (instruction.kind() as i32).to_string()),
        (
            DAEMON_INSTRUCTION_ENV_KEY.to_string(),
            instruction.serialized_instructions(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_kind_parsing() {
        assert_eq!(DaemonKind::from_env_value("1"), DaemonKind::ExitImmediately);
        assert_eq!(DaemonKind::from_env_value("2"), DaemonKind::RemoveUpdateRefsForCopiedBranch);
        assert_eq!(DaemonKind::from_env_value("3"), DaemonKind::MoveTodosUp);
        assert_eq!(DaemonKind::from_env_value("4"), DaemonKind::MoveTodosDown);
        assert_eq!(DaemonKind::from_env_value("5"), DaemonKind::InsertBreak);
        assert_eq!(DaemonKind::from_env_value("6"), DaemonKind::ChangeTodoActions);
        assert_eq!(DaemonKind::from_env_value("7"), DaemonKind::DropMergeCommit);
        assert_eq!(DaemonKind::from_env_value("8"), DaemonKind::MoveFixupCommitDown);
        assert_eq!(DaemonKind::from_env_value("9"), DaemonKind::WriteRebaseTodo);
        assert_eq!(DaemonKind::from_env_value("0"), DaemonKind::Unknown);
        assert_eq!(DaemonKind::from_env_value("invalid"), DaemonKind::Unknown);
    }

    #[test]
    fn test_exit_immediately_instruction() {
        let instruction = ExitImmediatelyInstruction::new();
        assert_eq!(instruction.kind(), DaemonKind::ExitImmediately);
        assert!(instruction.serialized_instructions().contains("ExitImmediately"));
    }

    #[test]
    fn test_move_todos_up_instruction() {
        let hashes = vec!["abc123".to_string(), "def456".to_string()];
        let instruction = MoveTodosUpInstruction::new(hashes.clone());
        assert_eq!(instruction.kind(), DaemonKind::MoveTodosUp);
        assert!(instruction.serialized_instructions().contains("abc123"));
    }

    #[test]
    fn test_change_todo_actions_instruction() {
        let changes = vec![
            ChangeTodoAction {
                hash: "abc123".to_string(),
                new_action: "pick".to_string(),
                flag: "".to_string(),
            },
        ];
        let instruction = ChangeTodoActionsInstruction::new(changes);
        assert_eq!(instruction.kind(), DaemonKind::ChangeTodoActions);
    }
}

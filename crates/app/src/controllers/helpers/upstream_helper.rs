// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/upstream_helper.go

pub struct UpstreamHelper {
    common: HelperCommon,
    get_remote_branches_suggestions_func: fn(String) -> fn(String) -> Vec<Suggestion>,
}

pub struct HelperCommon;

pub struct Suggestion {
    pub value: String,
    pub label: String,
}

pub struct Branch;
pub struct Remote;

impl UpstreamHelper {
    pub fn new(
        common: HelperCommon,
        get_remote_branches_suggestions_func: fn(String) -> fn(String) -> Vec<Suggestion>,
    ) -> Self {
        Self {
            common,
            get_remote_branches_suggestions_func,
        }
    }

    pub fn parse_upstream(&self, upstream: &str) -> Result<(String, String), String> {
        let split: Vec<&str> = upstream.split_whitespace().collect();
        if split.len() != 2 {
            return Err("Invalid upstream format".to_string());
        }
        Ok((split[0].to_string(), split[1].to_string()))
    }

    fn prompt_for_upstream(
        &self,
        _initial_content: &str,
        _on_confirm: fn(String) -> Result<(), String>,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn prompt_for_upstream_with_initial_content(
        &self,
        _current_branch: &Branch,
        _on_confirm: fn(String) -> Result<(), String>,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn prompt_for_upstream_without_initial_content(
        &self,
        _current_branch: &Branch,
        _on_confirm: fn(String) -> Result<(), String>,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn get_suggested_remote(&self) -> String {
        "origin".to_string()
    }
}

fn get_suggested_remote(_remotes: &[Remote]) -> String {
    "origin".to_string()
}

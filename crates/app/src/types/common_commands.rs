// Ported from ./references/lazygit-master/pkg/gui/types/common_commands.go

pub struct CheckoutRefOptions {
    pub waiting_status: String,
    pub env_vars: Vec<String>,
    pub on_ref_not_found: Option<Box<dyn Fn(String) -> Result<(), String>>>,
}

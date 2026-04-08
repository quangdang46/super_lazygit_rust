// Ported from ./references/lazygit-master/pkg/gui/presentation/item_operations.go

#[derive(Default)]
pub enum ItemOperation {
    #[default]
    None,
    Pushing,
    Pulling,
    FastForwarding,
    Deleting,
    Fetching,
    CheckingOut,
}


pub fn item_operation_to_string(operation: ItemOperation, tr: &TranslationSet) -> String {
    match operation {
        ItemOperation::None => String::new(),
        ItemOperation::Pushing => tr.pushing_status.clone(),
        ItemOperation::Pulling => tr.pulling_status.clone(),
        ItemOperation::FastForwarding => tr.fast_forwarding.clone(),
        ItemOperation::Deleting => tr.deleting_status.clone(),
        ItemOperation::Fetching => tr.fetching_status.clone(),
        ItemOperation::CheckingOut => tr.checking_out_status.clone(),
    }
}

pub struct TranslationSet {
    pub pushing_status: String,
    pub pulling_status: String,
    pub fast_forwarding: String,
    pub deleting_status: String,
    pub fetching_status: String,
    pub checking_out_status: String,
}

impl Default for TranslationSet {
    fn default() -> Self {
        Self {
            pushing_status: "Pushing".to_string(),
            pulling_status: "Pulling".to_string(),
            fast_forwarding: "Fast-forwarding".to_string(),
            deleting_status: "Deleting".to_string(),
            fetching_status: "Fetching".to_string(),
            checking_out_status: "Checking out".to_string(),
        }
    }
}

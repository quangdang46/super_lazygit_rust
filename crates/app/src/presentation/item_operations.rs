use crate::i18n::TranslationSet;

pub fn item_operation_to_string(
    item_operation: super_lazygit_tui::presentation::ItemOperation,
    tr: &TranslationSet,
) -> String {
    match item_operation {
        super_lazygit_tui::presentation::ItemOperation::None => String::new(),
        super_lazygit_tui::presentation::ItemOperation::Pushing => tr.pushing_status.clone(),
        super_lazygit_tui::presentation::ItemOperation::Pulling => tr.pulling_status.clone(),
        super_lazygit_tui::presentation::ItemOperation::FastForwarding => tr.fast_forwarding.clone(),
        super_lazygit_tui::presentation::ItemOperation::Deleting => tr.deleting_status.clone(),
        super_lazygit_tui::presentation::ItemOperation::Fetching => tr.fetching_status.clone(),
        super_lazygit_tui::presentation::ItemOperation::CheckingOut => tr.checking_out_status.clone(),
    }
}

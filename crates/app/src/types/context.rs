// Ported from ./references/lazygit-master/pkg/gui/types/context.go

use crate::types::common::HasUrn;

#[derive(Clone, Copy)]
pub enum ContextKind {
    SideContext,
    MainContext,
    PersistentPopup,
    TemporaryPopup,
    ExtrasContext,
    GlobalContext,
    DisplayContext,
}

pub trait ParentContexter {
    fn set_parent_context(&mut self, ctx: ());
    fn get_parent_context(&self) -> Option<()>;
}

#[derive(Clone, Copy)]
pub enum NeedsRerenderOnWidthChangeLevel {
    None,
    WhenWidthChanges,
    WhenScreenModeChanges,
}

pub trait IBaseContext: ParentContexter {
    fn get_kind(&self) -> ContextKind;
    fn get_view_name(&self) -> &str;
    fn get_window_name(&self) -> &str;
    fn set_window_name(&mut self, name: &str);
    fn get_key(&self) -> ContextKey;
    fn is_focusable(&self) -> bool;
    fn is_transient(&self) -> bool;
    fn has_controlled_bounds(&self) -> bool;
    fn total_content_height(&self) -> i32;
    fn needs_rerender_on_width_change(&self) -> NeedsRerenderOnWidthChangeLevel;
    fn needs_rerender_on_height_change(&self) -> bool;
    fn title(&self) -> &str;
}

pub trait Context: IBaseContext {
    fn handle_focus(&mut self, opts: OnFocusOpts);
    fn handle_focus_lost(&mut self, opts: OnFocusLostOpts);
    fn focus_line(&mut self, scroll_into_view: bool);
    fn handle_render(&mut self);
}

pub trait ISearchHistoryContext: Context {
    fn get_search_history(&self) -> &SearchHistoryBuffer;
}

pub trait IFilterableContext: Context {
    fn set_filter(&mut self, filter: &str, _: bool);
    fn get_filter(&self) -> &str;
    fn clear_filter(&mut self);
    fn reapply_filter(&mut self, _: bool);
    fn is_filtering(&self) -> bool;
}

pub trait ISearchableContext: Context {
    fn set_search_string(&mut self, search: &str);
    fn get_search_string(&self) -> &str;
    fn clear_search_string(&mut self);
    fn is_searching(&self) -> bool;
}

pub trait DiffableContext: Context {
    fn get_diff_terminals(&self) -> Vec<String>;
    fn ref_for_adjusting_line_number_in_diff(&self) -> String;
}

pub trait IListContext: Context {
    fn get_selected_item_id(&self) -> String;
    fn get_selected_item_ids(&self) -> (Vec<String>, usize, usize);
    fn is_item_visible(&self, item: &dyn HasUrn) -> bool;
    fn get_list(&self) -> &dyn IList;
    fn view_index_to_model_index(&self, idx: i32) -> i32;
    fn model_index_to_view_index(&self, idx: i32) -> i32;
    fn is_list_context(&self);
    fn range_select_enabled(&self) -> bool;
    fn render_only_visible_lines(&self) -> bool;
}

pub trait IPatchExplorerContext: Context {
    fn get_state(&self) -> &PatchExplorerState;
    fn set_state(&mut self, state: PatchExplorerState);
    fn get_included_line_indices(&self) -> Vec<i32>;
}

pub trait IViewTrait {
    fn focus_point(&mut self, y_idx: i32, scroll_into_view: bool);
    fn set_range_select_start(&mut self, y_idx: i32);
    fn cancel_range_select(&mut self);
    fn set_view_port_content(&mut self, content: &str);
    fn set_content(&mut self, content: &str);
    fn set_footer(&mut self, value: &str);
    fn scroll_left(&mut self);
    fn scroll_right(&mut self);
    fn scroll_up(&mut self, value: i32);
    fn scroll_down(&mut self, value: i32);
    fn selected_line_idx(&self) -> i32;
}

pub struct OnFocusOpts {
    pub clicked_window_name: String,
    pub clicked_view_line_idx: i32,
    pub scroll_selection_into_view: bool,
}

pub struct OnFocusLostOpts {
    pub new_context_key: ContextKey,
}

pub struct ContextKey(pub String);

pub struct KeybindingsOpts;

pub type KeybindingsFn = Box<dyn Fn(KeybindingsOpts) -> Vec<Binding>>;
pub type MouseKeybindingsFn = Box<dyn Fn(KeybindingsOpts) -> Vec<ViewMouseBinding>>;

pub trait HasKeybindings {
    fn get_keybindings(&self, opts: KeybindingsOpts) -> Vec<Binding>;
    fn get_mouse_keybindings(&self, opts: KeybindingsOpts) -> Vec<ViewMouseBinding>;
    fn get_on_double_click(&self) -> Option<Box<dyn Fn() -> Result<(), String>>>;
    fn get_on_click(&self) -> Option<Box<dyn Fn(ViewMouseBindingOpts) -> Result<(), String>>>;
}

pub trait IController: HasKeybindings {
    fn context(&self) -> &dyn Context;
    fn get_on_render_to_main(&self) -> Option<Box<dyn Fn()>>;
    fn get_on_focus(&self) -> Option<Box<dyn Fn(OnFocusOpts)>>;
    fn get_on_focus_lost(&self) -> Option<Box<dyn Fn(OnFocusLostOpts)>>;
}

pub trait IList {
    fn len(&self) -> usize;
    fn get_item(&self, index: usize) -> &dyn HasUrn;
}

pub trait IListCursor {
    fn get_selected_line_idx(&self) -> i32;
    fn set_selected_line_idx(&mut self, value: i32);
    fn set_selection(&mut self, value: i32);
    fn move_selected_line(&mut self, delta: i32);
    fn clamp_selection(&mut self);
    fn cancel_range_select(&mut self);
    fn get_range_start_idx(&self) -> (i32, bool);
    fn get_selection_range(&self) -> (i32, i32);
    fn is_selecting_range(&self) -> bool;
    fn are_multiple_items_selected(&self) -> bool;
}

pub trait IListPanelState {
    fn set_selected_line_idx(&mut self, idx: i32);
    fn set_selection(&mut self, value: i32);
    fn get_selected_line_idx(&self) -> i32;
}

pub trait ListItem {
    fn id(&self) -> String;
    fn description(&self) -> String;
}

pub trait IContextMgr {
    fn push(&mut self, context: (), opts: OnFocusOpts);
    fn pop(&mut self);
    fn replace(&mut self, context: ());
    fn activate(&mut self, context: (), opts: OnFocusOpts);
    fn current(&self) -> ();
}

pub struct Binding;
pub struct ViewMouseBinding;
pub struct ViewMouseBindingOpts;
pub struct SearchHistoryBuffer;
pub struct PatchExplorerState;

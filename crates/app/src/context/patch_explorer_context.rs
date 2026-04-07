// Ported from ./references/lazygit-master/pkg/gui/context/patch_explorer_context.go

use crate::types::common::HasUrn;
use crate::types::context::{
    Context, ContextKind, HasKeybindings, IBaseContext, IList, IListContext, IListPanelState,
    NeedsRerenderOnWidthChangeLevel, OnFocusLostOpts, OnFocusOpts,
};

pub struct PatchExplorerContext {
    pub key: String,
    state: PatchExplorerState,
    view_trait: ViewTrait,
    get_included_line_indices: fn() -> Vec<i32>,
    c: ContextCommon,
    mutex: Mutex,
    handle_render_func: Option<fn()>,
    in_on_select_item_callback: bool,
    on_focus_fns: Vec<fn(OnFocusOpts)>,
    on_focus_lost_fns: Vec<fn(OnFocusLostOpts)>,
    on_render_to_main_fn: Option<fn()>,
    window_name: String,
    kind: ContextKind,
    focusable: bool,
    transient: bool,
    has_controlled_bounds: bool,
    needs_rerender_on_width_change: NeedsRerenderOnWidthChangeLevel,
    needs_rerender_on_height_change: bool,
}

pub struct PatchExplorerState;
pub struct ViewTrait;
pub struct ContextCommon;
pub struct Mutex;

impl PatchExplorerContext {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
            state: PatchExplorerState,
            view_trait: ViewTrait,
            get_included_line_indices: || vec![],
            c: ContextCommon,
            mutex: Mutex,
            handle_render_func: None,
            in_on_select_item_callback: false,
            on_focus_fns: vec![],
            on_focus_lost_fns: vec![],
            on_render_to_main_fn: None,
            window_name: String::new(),
            kind: ContextKind::MainContext,
            focusable: true,
            transient: false,
            has_controlled_bounds: false,
            needs_rerender_on_width_change: NeedsRerenderOnWidthChangeLevel::WhenWidthChanges,
            needs_rerender_on_height_change: false,
        }
    }

    pub fn get_state(&self) -> &PatchExplorerState {
        &self.state
    }

    pub fn set_state(&mut self, state: PatchExplorerState) {
        self.state = state;
    }

    pub fn get_view_trait(&self) -> &ViewTrait {
        &self.view_trait
    }

    pub fn get_included_line_indices(&self) -> Vec<i32> {
        (self.get_included_line_indices)()
    }

    pub fn mutex(&self) -> &Mutex {
        &self.mutex
    }
}

impl IBaseContext for PatchExplorerContext {
    fn get_kind(&self) -> ContextKind {
        self.kind
    }

    fn get_view_name(&self) -> &str {
        ""
    }

    fn get_window_name(&self) -> &str {
        &self.window_name
    }

    fn set_window_name(&mut self, name: &str) {
        self.window_name = name.to_string();
    }

    fn get_key(&self) -> crate::types::context::ContextKey {
        crate::types::context::ContextKey(self.key.clone())
    }

    fn is_focusable(&self) -> bool {
        self.focusable
    }

    fn is_transient(&self) -> bool {
        self.transient
    }

    fn has_controlled_bounds(&self) -> bool {
        self.has_controlled_bounds
    }

    fn total_content_height(&self) -> i32 {
        0
    }

    fn needs_rerender_on_width_change(&self) -> NeedsRerenderOnWidthChangeLevel {
        self.needs_rerender_on_width_change
    }

    fn needs_rerender_on_height_change(&self) -> bool {
        self.needs_rerender_on_height_change
    }

    fn title(&self) -> &str {
        ""
    }
}

impl Context for PatchExplorerContext {
    fn handle_focus(&mut self, opts: OnFocusOpts) {
        for f in &self.on_focus_fns {
            f(opts.clone());
        }
        if let Some(f) = self.on_render_to_main_fn {
            f();
        }
    }

    fn handle_focus_lost(&mut self, opts: OnFocusLostOpts) {
        for f in &self.on_focus_lost_fns {
            f(opts.clone());
        }
    }

    fn focus_line(&mut self, _scroll_into_view: bool) {}

    fn handle_render(&mut self) {
        if let Some(f) = self.handle_render_func {
            f();
        }
    }
}

impl IListContext for PatchExplorerContext {
    fn get_selected_item_id(&self) -> String {
        String::new()
    }

    fn get_selected_item_ids(&self) -> (Vec<String>, usize, usize) {
        (vec![], 0, 0)
    }

    fn is_item_visible(&self, _item: &dyn HasUrn) -> bool {
        true
    }

    fn get_list(&self) -> &dyn IList {
        self
    }

    fn view_index_to_model_index(&self, _idx: i32) -> i32 {
        0
    }

    fn model_index_to_view_index(&self, _idx: i32) -> i32 {
        0
    }

    fn is_list_context(&self) {}

    fn range_select_enabled(&self) -> bool {
        true
    }

    fn render_only_visible_lines(&self) -> bool {
        false
    }
}

impl IListPanelState for PatchExplorerContext {
    fn set_selected_line_idx(&mut self, _idx: i32) {}

    fn set_selection(&mut self, _value: i32) {}

    fn get_selected_line_idx(&self) -> i32 {
        0
    }
}

impl IList for PatchExplorerContext {
    fn len(&self) -> usize {
        0
    }

    fn get_item(&self, _index: usize) -> &dyn HasUrn {
        self
    }
}

impl HasKeybindings for PatchExplorerContext {
    fn get_keybindings(&self, _opts: crate::types::context::KeybindingsOpts) -> Vec<crate::types::context::Binding> {
        vec![]
    }

    fn get_mouse_keybindings(&self, _opts: crate::types::context::KeybindingsOpts) -> Vec<crate::types::context::ViewMouseBinding> {
        vec![]
    }

    fn get_on_double_click(&self) -> Option<Box<dyn Fn() -> Result<(), String>>> {
        None
    }

    fn get_on_click(&self) -> Option<Box<dyn Fn(crate::types::context::ViewMouseBindingOpts) -> Result<(), String>>> {
        None
    }
}

impl HasUrn for PatchExplorerContext {
    fn urn(&self) -> String {
        self.key.clone()
    }
}

impl crate::types::context::ParentContexter for PatchExplorerContext {
    fn set_parent_context(&mut self, _ctx: ()) {}
    fn get_parent_context(&self) -> Option<()> {
        None
    }
}

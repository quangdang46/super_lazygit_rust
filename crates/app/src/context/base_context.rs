// Ported from ./references/lazygit-master/pkg/gui/context/base_context.go

use crate::types::context::{
    Binding, Context, ContextKey, ContextKind, HasKeybindings, IBaseContext, IViewTrait,
    KeybindingsOpts, MouseKeybindingsFn, NeedsRerenderOnWidthChangeLevel, OnFocusLostOpts,
    OnFocusOpts, ViewMouseBinding, ViewMouseBindingOpts,
};

pub struct BaseContext {
    kind: ContextKind,
    key: ContextKey,
    view_name: String,
    window_name: String,
    on_get_options_map: Option<fn() -> Vec<(String, String)>>,
    view_trait: Option<Box<dyn IViewTrait>>,

    keybindings_fns: Vec<fn(KeybindingsOpts) -> Vec<Binding>>,
    mouse_keybindings_fns: Vec<MouseKeybindingsFn>,
    on_double_click_fn: Option<fn() -> Result<(), String>>,
    on_click_fn: Option<fn(ViewMouseBindingOpts) -> Result<(), String>>,
    on_click_focused_main_view_fn:
        Option<fn(main_view_name: &str, clicked_line_idx: i32) -> Result<(), String>>,
    on_render_to_main_fn: Option<fn()>,
    on_focus_fns: Vec<fn(OnFocusOpts)>,
    on_focus_lost_fns: Vec<fn(OnFocusLostOpts)>,

    focusable: bool,
    transient: bool,
    has_controlled_bounds: bool,
    needs_rerender_on_width_change: NeedsRerenderOnWidthChangeLevel,
    needs_rerender_on_height_change: bool,
    highlight_on_focus: bool,

    parent_context: Option<()>,
}

pub struct NewBaseContextOpts {
    pub kind: ContextKind,
    pub key: String,
    pub view_name: String,
    pub window_name: String,
    pub focusable: bool,
    pub transient: bool,
    pub has_uncontrolled_bounds: bool,
    pub highlight_on_focus: bool,
    pub needs_rerender_on_width_change: NeedsRerenderOnWidthChangeLevel,
    pub needs_rerender_on_height_change: bool,
    pub on_get_options_map: Option<fn() -> Vec<(String, String)>>,
}

impl BaseContext {
    pub fn new(opts: NewBaseContextOpts) -> Self {
        let has_controlled_bounds = !opts.has_uncontrolled_bounds;

        Self {
            kind: opts.kind,
            key: ContextKey(opts.key),
            view_name: opts.view_name,
            window_name: opts.window_name,
            on_get_options_map: opts.on_get_options_map,
            view_trait: None,
            keybindings_fns: vec![],
            mouse_keybindings_fns: vec![],
            on_double_click_fn: None,
            on_click_fn: None,
            on_click_focused_main_view_fn: None,
            on_render_to_main_fn: None,
            on_focus_fns: vec![],
            on_focus_lost_fns: vec![],
            focusable: opts.focusable,
            transient: opts.transient,
            has_controlled_bounds,
            needs_rerender_on_width_change: opts.needs_rerender_on_width_change,
            needs_rerender_on_height_change: opts.needs_rerender_on_height_change,
            highlight_on_focus: opts.highlight_on_focus,
            parent_context: None,
        }
    }

    pub fn get_options_map(&self) -> Option<Vec<(String, String)>> {
        self.on_get_options_map.map(|f| f())
    }

    pub fn set_window_name(&mut self, window_name: &str) {
        self.window_name = window_name.to_string();
    }

    pub fn get_window_name(&self) -> &str {
        &self.window_name
    }

    pub fn get_view_name(&self) -> &str {
        if self.view_name.is_empty() {
            ""
        } else {
            &self.view_name
        }
    }

    pub fn get_view_trait(&self) -> Option<&dyn IViewTrait> {
        self.view_trait.as_deref()
    }

    pub fn get_kind(&self) -> ContextKind {
        self.kind
    }

    pub fn get_key(&self) -> ContextKey {
        self.key.clone()
    }

    pub fn get_keybindings(&self, opts: &KeybindingsOpts) -> Vec<Binding> {
        let mut bindings = vec![];
        for i in 0..self.keybindings_fns.len() {
            let idx = self.keybindings_fns.len() - 1 - i;
            bindings.extend(self.keybindings_fns[idx](KeybindingsOpts));
        }
        bindings
    }

    pub fn add_keybindings_fn(&mut self, f: fn(KeybindingsOpts) -> Vec<Binding>) {
        self.keybindings_fns.push(f);
    }

    pub fn add_mouse_keybindings_fn(&mut self, f: MouseKeybindingsFn) {
        self.mouse_keybindings_fns.push(f);
    }

    pub fn clear_all_attached_controller_functions(&mut self) {
        self.keybindings_fns.clear();
        self.mouse_keybindings_fns.clear();
        self.on_focus_fns.clear();
        self.on_focus_lost_fns.clear();
        self.on_double_click_fn = None;
        self.on_click_fn = None;
        self.on_click_focused_main_view_fn = None;
        self.on_render_to_main_fn = None;
    }

    pub fn add_on_double_click_fn(&mut self, f: fn() -> Result<(), String>) {
        if self.on_double_click_fn.is_some() {
            panic!("only one controller is allowed to set an onDoubleClickFn");
        }
        self.on_double_click_fn = Some(f);
    }

    pub fn add_on_click_fn(&mut self, f: fn(ViewMouseBindingOpts) -> Result<(), String>) {
        if self.on_click_fn.is_some() {
            panic!("only one controller is allowed to set an onClickFn");
        }
        self.on_click_fn = Some(f);
    }

    pub fn add_on_click_focused_main_view_fn(
        &mut self,
        f: fn(main_view_name: &str, clicked_line_idx: i32) -> Result<(), String>,
    ) {
        if self.on_click_focused_main_view_fn.is_some() {
            panic!("only one controller is allowed to set an onClickFocusedMainViewFn");
        }
        self.on_click_focused_main_view_fn = Some(f);
    }

    pub fn get_on_double_click(&self) -> Option<fn() -> Result<(), String>> {
        self.on_double_click_fn
    }

    pub fn get_on_click(&self) -> Option<fn(ViewMouseBindingOpts) -> Result<(), String>> {
        self.on_click_fn
    }

    pub fn get_on_click_focused_main_view(&self) -> Option<fn(&str, i32) -> Result<(), String>> {
        self.on_click_focused_main_view_fn
    }

    pub fn add_on_render_to_main_fn(&mut self, f: fn()) {
        if self.on_render_to_main_fn.is_some() {
            panic!("only one controller is allowed to set an onRenderToMainFn");
        }
        self.on_render_to_main_fn = Some(f);
    }

    pub fn add_on_focus_fn(&mut self, f: fn(OnFocusOpts)) {
        self.on_focus_fns.push(f);
    }

    pub fn add_on_focus_lost_fn(&mut self, f: fn(OnFocusLostOpts)) {
        self.on_focus_lost_fns.push(f);
    }

    pub fn get_mouse_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<ViewMouseBinding> {
        let mut bindings = vec![];
        for i in 0..self.mouse_keybindings_fns.len() {
            let idx = self.mouse_keybindings_fns.len() - 1 - i;
            bindings.extend(self.mouse_keybindings_fns[idx](KeybindingsOpts));
        }
        bindings
    }

    pub fn is_focusable(&self) -> bool {
        self.focusable
    }

    pub fn is_transient(&self) -> bool {
        self.transient
    }

    pub fn has_controlled_bounds(&self) -> bool {
        self.has_controlled_bounds
    }

    pub fn needs_rerender_on_width_change(&self) -> NeedsRerenderOnWidthChangeLevel {
        self.needs_rerender_on_width_change
    }

    pub fn needs_rerender_on_height_change(&self) -> bool {
        self.needs_rerender_on_height_change
    }

    pub fn title(&self) -> &str {
        ""
    }

    pub fn total_content_height(&self) -> i32 {
        0
    }
}

impl IBaseContext for BaseContext {
    fn get_kind(&self) -> ContextKind {
        self.kind
    }

    fn get_view_name(&self) -> &str {
        if self.view_name.is_empty() {
            ""
        } else {
            &self.view_name
        }
    }

    fn get_window_name(&self) -> &str {
        &self.window_name
    }

    fn set_window_name(&mut self, name: &str) {
        self.window_name = name.to_string();
    }

    fn get_key(&self) -> ContextKey {
        self.key.clone()
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

impl Context for BaseContext {
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

    fn handle_render(&mut self) {}
}

impl HasKeybindings for BaseContext {
    fn get_keybindings(&self, _opts: KeybindingsOpts) -> Vec<Binding> {
        let mut bindings = vec![];
        for i in 0..self.keybindings_fns.len() {
            let idx = self.keybindings_fns.len() - 1 - i;
            bindings.extend(self.keybindings_fns[idx](KeybindingsOpts));
        }
        bindings
    }

    fn get_mouse_keybindings(&self, _opts: KeybindingsOpts) -> Vec<ViewMouseBinding> {
        let mut bindings = vec![];
        for i in 0..self.mouse_keybindings_fns.len() {
            let idx = self.mouse_keybindings_fns.len() - 1 - i;
            bindings.extend(self.mouse_keybindings_fns[idx](KeybindingsOpts));
        }
        bindings
    }

    fn get_on_double_click(&self) -> Option<Box<dyn Fn() -> Result<(), String>>> {
        self.on_double_click_fn
            .map(|f| Box::new(f) as Box<dyn Fn() -> Result<(), String>>)
    }

    fn get_on_click(&self) -> Option<Box<dyn Fn(ViewMouseBindingOpts) -> Result<(), String>>> {
        self.on_click_fn
            .map(|f| Box::new(f) as Box<dyn Fn(ViewMouseBindingOpts) -> Result<(), String>>)
    }
}

impl crate::types::context::ParentContexter for BaseContext {
    fn set_parent_context(&mut self, _ctx: ()) {
        self.parent_context = Some(());
    }

    fn get_parent_context(&self) -> Option<()> {
        self.parent_context
    }
}

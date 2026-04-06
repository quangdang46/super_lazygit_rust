// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/window_helper.go

pub struct WindowHelper {
    common: HelperCommon,
    view_helper: ViewHelper,
}

pub struct HelperCommon;
pub struct ViewHelper;
pub struct Context;

impl Context {
    pub fn get_window_name(&self) -> String {
        String::new()
    }
    pub fn get_view_name(&self) -> String {
        String::new()
    }
    pub fn is_transient(&self) -> bool {
        false
    }
    pub fn get_key(&self) -> String {
        String::new()
    }
    pub fn get_view(&self) -> Option<View> {
        None
    }
}

pub struct View;

impl View {
    pub fn name(&self) -> String {
        String::new()
    }
}

pub struct ThreadSafeMap<K, V> {
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> ThreadSafeMap<K, V> {
    pub fn get(&self, _key: &K) -> Option<V> {
        None
    }
    pub fn set(&self, _key: K, _value: V) {}
    pub fn keys(&self) -> Vec<K> {
        Vec::new()
    }
}

impl WindowHelper {
    pub fn new(common: HelperCommon, view_helper: ViewHelper) -> Self {
        Self {
            common,
            view_helper,
        }
    }

    pub fn get_viewName_for_window(&self, _window: &str) -> String {
        String::new()
    }

    pub fn get_context_for_window(&self, _window: &str) -> Context {
        Context
    }

    pub fn set_window_context(&self, _context: &Context) {}

    pub fn current_window(&self) -> String {
        String::new()
    }

    pub fn move_to_top_of_window(&self, _context: &Context) {}

    pub fn top_view_in_window(
        &self,
        _window_name: &str,
        _include_invisible_views: bool,
    ) -> Option<View> {
        None
    }

    pub fn window_for_view(&self, _view_name: &str) -> String {
        String::new()
    }

    pub fn side_windows(&self) -> Vec<String> {
        vec![
            "status".to_string(),
            "files".to_string(),
            "branches".to_string(),
            "commits".to_string(),
            "stash".to_string(),
        ]
    }
}

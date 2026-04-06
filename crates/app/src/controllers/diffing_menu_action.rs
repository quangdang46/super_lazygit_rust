// Ported from ./references/lazygit-master/pkg/gui/controllers/diffing_menu_action.go

pub struct DiffingMenuAction {
    context: ControllerCommon,
}

pub struct ControllerCommon {
    helpers: HelperCommon,
}

pub struct HelperCommon {
    diff_helper: DiffHelper,
    tr: TrStrings,
}

pub struct DiffHelper;

impl DiffHelper {
    pub fn current_diff_terminals(&self) -> Vec<String> {
        Vec::new()
    }
}

impl DiffingMenuAction {
    pub fn new(context: ControllerCommon) -> Self {
        Self { context }
    }

    pub fn call(&self) -> Result<(), String> {
        let names = self.context.helpers.diff_helper.current_diff_terminals();
        let mut menu_items: Vec<MenuItem> = Vec::new();

        for name in &names {
            let label = format!("{} {}", self.context.tr.diff, name);
            let menu_item = MenuItem {
                label,
                on_press: Box::new(|| Ok(())),
            };
            menu_items.push(menu_item);
        }

        let enter_ref_menu_item = MenuItem {
            label: self.context.tr.enter_ref_to_diff.clone(),
            on_press: Box::new(|| Ok(())),
        };
        menu_items.push(enter_ref_menu_item);

        if self.is_diffing_active() {
            let swap_menu_item = MenuItem {
                label: self.context.tr.swap_diff.clone(),
                on_press: Box::new(|| Ok(())),
            };
            menu_items.push(swap_menu_item);

            let exit_menu_item = MenuItem {
                label: self.context.tr.exit_diff_mode.clone(),
                on_press: Box::new(|| Ok(())),
            };
            menu_items.push(exit_menu_item);
        }

        let options = CreateMenuOptions {
            title: self.context.tr.diffing_menu_title.clone(),
            items: menu_items,
            ..Default::default()
        };

        self.context.menu(options)
    }

    fn is_diffing_active(&self) -> bool {
        false
    }
}

pub struct TrStrings {
    pub diff: String,
    pub enter_ref_to_diff: String,
    pub enter_ref_name: String,
    pub diffing_menu_title: String,
    pub swap_diff: String,
    pub exit_diff_mode: String,
}

impl TrStrings {
    pub fn new() -> Self {
        Self {
            diff: "Diff".to_string(),
            enter_ref_to_diff: "Enter ref to diff".to_string(),
            enter_ref_name: "Enter ref name".to_string(),
            diffing_menu_title: "Diffing options".to_string(),
            swap_diff: "Swap diff".to_string(),
            exit_diff_mode: "Exit diff mode".to_string(),
        }
    }
}

impl Default for TrStrings {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MenuItem {
    pub label: String,
    pub on_press: Box<dyn Fn() -> Result<(), String>>,
}

#[derive(Debug, Clone)]
pub enum RefreshMode {
    Sync,
    Async,
}

pub struct RefreshOptions {
    pub mode: RefreshMode,
}

#[derive(Debug, Clone)]
pub struct CreateMenuOptions {
    pub title: String,
    pub prompt: Option<String>,
    pub items: Vec<MenuItem>,
    pub hide_cancel: bool,
    pub column_alignment: Vec<String>,
    pub allow_filtering_keybindings: bool,
    pub keep_conflicting_keybindings: bool,
}

impl Default for CreateMenuOptions {
    fn default() -> Self {
        Self {
            title: String::new(),
            prompt: None,
            items: Vec::new(),
            hide_cancel: false,
            column_alignment: Vec::new(),
            allow_filtering_keybindings: false,
            keep_conflicting_keybindings: false,
        }
    }
}

impl ControllerCommon {
    pub fn menu(&self, _options: CreateMenuOptions) -> Result<(), String> {
        Ok(())
    }
}

impl HelperCommon {
    pub fn new() -> Self {
        Self {
            diff_helper: DiffHelper,
            tr: TrStrings::new(),
        }
    }
}

impl Default for HelperCommon {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffingMenuAction {
    pub fn new() -> Self {
        Self {
            context: ControllerCommon {
                helpers: HelperCommon::new(),
            },
        }
    }
}

impl Default for DiffingMenuAction {
    fn default() -> Self {
        Self::new()
    }
}

// Ported from ./references/lazygit-master/pkg/gui/controllers/screen_mode_actions.go
use crate::controllers::ControllerCommon;

pub struct ScreenModeActions {
    common: ControllerCommon,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ScreenMode {
    Normal,
    Half,
    Full,
}

impl ScreenModeActions {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn next(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn prev(&self) -> Result<(), String> {
        Ok(())
    }

    fn rerender_views_with_screen_mode_dependent_content(&self) {}

    fn rerender_view(&self, _view: &View) {}
}

pub struct View;

fn next_in_cycle(sl: &[ScreenMode], current: &ScreenMode) -> ScreenMode {
    for (i, val) in sl.iter().enumerate() {
        if val == current {
            if i == sl.len() - 1 {
                return sl[0].clone();
            }
            return sl[i + 1].clone();
        }
    }
    sl[0].clone()
}

fn prev_in_cycle(sl: &[ScreenMode], current: &ScreenMode) -> ScreenMode {
    for (i, val) in sl.iter().enumerate() {
        if val == current {
            if i > 0 {
                return sl[i - 1].clone();
            }
            return sl[sl.len() - 1].clone();
        }
    }
    sl[sl.len() - 1].clone()
}

// Ported from ./references/lazygit-master/pkg/gui/types/rendering.go

pub struct MainContextPair {
    pub main: Context,
    pub secondary: Context,
}

pub struct MainViewPairs {
    pub normal: MainContextPair,
    pub merge_conflicts: MainContextPair,
    pub staging: MainContextPair,
    pub patch_building: MainContextPair,
}

pub struct ViewUpdateOpts {
    pub title: String,
    pub sub_title: String,
    pub task: Box<dyn UpdateTask>,
}

pub struct RefreshMainOpts {
    pub pair: MainContextPair,
    pub main: Option<ViewUpdateOpts>,
    pub secondary: Option<ViewUpdateOpts>,
}

pub trait UpdateTask {
    fn is_update_task(&self);
}

pub struct RenderStringTask {
    pub str: String,
}

impl UpdateTask for RenderStringTask {
    fn is_update_task(&self) {}
}

pub struct RenderStringWithoutScrollTask {
    pub str: String,
}

impl UpdateTask for RenderStringWithoutScrollTask {
    fn is_update_task(&self) {}
}

pub struct RenderStringWithScrollTask {
    pub str: String,
    pub origin_x: i32,
    pub origin_y: i32,
}

impl UpdateTask for RenderStringWithScrollTask {
    fn is_update_task(&self) {}
}

pub struct RunCommandTask {
    pub cmd: Command,
    pub prefix: String,
}

impl UpdateTask for RunCommandTask {
    fn is_update_task(&self) {}
}

pub struct RunPtyTask {
    pub cmd: Command,
    pub prefix: String,
}

impl UpdateTask for RunPtyTask {
    fn is_update_task(&self) {}
}

pub struct Command;

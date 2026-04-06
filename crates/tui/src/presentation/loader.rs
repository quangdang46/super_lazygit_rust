// Ported from ./references/lazygit-master/pkg/gui/presentation/loader.go

pub struct SpinnerConfig {
    pub rate: i64,
    pub frames: Vec<String>,
}

impl Default for SpinnerConfig {
    fn default() -> Self {
        Self {
            rate: 100,
            frames: vec![
                "⠋".to_string(),
                "⠙".to_string(),
                "⠹".to_string(),
                "⠸".to_string(),
                "⠼".to_string(),
                "⠴".to_string(),
                "⠦".to_string(),
                "⠧".to_string(),
                "⠇".to_string(),
                "⠏".to_string(),
            ],
        }
    }
}

pub fn loader(now_millis: i64, config: &SpinnerConfig) -> String {
    let index = (now_millis / config.rate) % config.frames.len() as i64;
    config.frames[index as usize].clone()
}

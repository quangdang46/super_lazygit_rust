// Ported from ./references/lazygit-master/pkg/gui/style/color.go

use ratatui::style::Color;

#[derive(Clone, Copy)]
pub struct AppColor {
    rgb: Option<(u8, u8, u8)>,
    basic: Option<Color>,
}

impl AppColor {
    pub fn new_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            rgb: Some((r, g, b)),
            basic: None,
        }
    }

    pub fn new_basic(color: Color) -> Self {
        Self {
            rgb: None,
            basic: Some(color),
        }
    }

    pub fn is_rgb(&self) -> bool {
        self.rgb.is_some()
    }

    pub fn to_ratatui_color(&self, is_bg: bool) -> Color {
        if let Some((r, g, b)) = self.rgb {
            return Color::Rgb(r, g, b);
        }

        if let Some(c) = self.basic {
            if is_bg {
                if let Some(rgb) = c.into_rgb() {
                    return Color::Rgb(rgb.0, rgb.1, rgb.2);
                }
            } else {
                if let Some(rgb) = c.into_rgb() {
                    return Color::Rgb(rgb.0, rgb.1, rgb.2);
                }
            }
        }
        Color::Reset
    }
}

impl From<Color> for AppColor {
    fn from(color: Color) -> Self {
        Self::new_basic(color)
    }
}

impl From<(u8, u8, u8)> for AppColor {
    fn from(rgb: (u8, u8, u8)) -> Self {
        Self::new_rgb(rgb.0, rgb.1, rgb.2)
    }
}

impl AppColor {
    pub fn default_foreground() -> Self {
        Self::new_basic(Color::Reset)
    }

    pub fn default_background() -> Self {
        Self::new_basic(Color::Reset)
    }
}

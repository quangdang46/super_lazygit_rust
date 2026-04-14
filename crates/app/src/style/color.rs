// Ported from ./references/lazygit-master/pkg/gui/style/color.go

use ratatui::style::Color;

/// ColorScheme represents the user's color scheme preference.
/// System variant defers to the terminal's detected color scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorScheme {
    Dark,
    Light,
    #[default]
    System,
}

impl ColorScheme {
    /// Returns true if this is the System variant (i.e., follow terminal preference)
    pub fn is_system(&self) -> bool {
        matches!(self, ColorScheme::System)
    }
}

/// Global color scheme state with user override capability.
/// Uses OnceLock for lazy initialization and RwLock for safe mutation.
static COLOR_SCHEME_STATE: std::sync::OnceLock<std::sync::RwLock<ColorScheme>> =
    std::sync::OnceLock::new();

/// Returns the current active color scheme.
/// If the user has overridden it, returns the override; otherwise returns the given terminal scheme.
pub fn color_scheme(terminal_color_scheme: ColorScheme) -> ColorScheme {
    let state = COLOR_SCHEME_STATE.get_or_init(|| std::sync::RwLock::new(ColorScheme::System));
    let guard = state.read().unwrap();
    let user_override = *guard;
    if user_override.is_system() {
        terminal_color_scheme
    } else {
        user_override
    }
}

/// Sets a user override for the color scheme.
/// Calling this with a non-System value forces that scheme regardless of terminal detection.
pub fn set_color_scheme(scheme: ColorScheme) {
    let state = COLOR_SCHEME_STATE.get_or_init(|| std::sync::RwLock::new(ColorScheme::System));
    let mut guard = state.write().unwrap();
    *guard = scheme;
}

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

    pub fn get_rgb(&self) -> Option<(u8, u8, u8)> {
        self.rgb
    }

    pub fn to_ratatui_color(&self, _is_bg: bool) -> Color {
        if let Some((r, g, b)) = self.rgb {
            return Color::Rgb(r, g, b);
        }

        if let Some(c) = self.basic {
            return c;
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

/// Maps a color name to an AppColor based on the active color scheme.
/// Returns different RGB values for light vs dark schemes for better readability.
pub fn scheme_aware_color(name: &str, scheme: ColorScheme) -> AppColor {
    // Determine effective scheme (resolve System to Dark/Light)
    let effective_scheme = match scheme {
        ColorScheme::System => ColorScheme::Dark, // Default fallback
        other => other,
    };

    match (name, effective_scheme) {
        // Selection colors
        ("selection", ColorScheme::Dark) => AppColor::new_rgb(0, 0, 170), // Blue for dark
        ("selection", ColorScheme::Light) => AppColor::new_rgb(170, 170, 255), // Lighter blue for light

        // Active border colors
        ("active_border", ColorScheme::Dark) => AppColor::new_rgb(0, 180, 0), // Bright green for dark
        ("active_border", ColorScheme::Light) => AppColor::new_rgb(0, 100, 0), // Darker green for light

        // Inactive border colors
        ("inactive_border", ColorScheme::Dark) => AppColor::new_rgb(128, 128, 128), // Gray for dark
        ("inactive_border", ColorScheme::Light) => AppColor::new_rgb(100, 100, 100), // Darker gray for light

        // Options/bars
        ("options", ColorScheme::Dark) => AppColor::new_rgb(80, 80, 80),
        ("options", ColorScheme::Light) => AppColor::new_rgb(200, 200, 200),

        // Text colors
        ("text", ColorScheme::Dark) => AppColor::new_rgb(200, 200, 200),
        ("text", ColorScheme::Light) => AppColor::new_rgb(50, 50, 50),

        ("text_highlight", ColorScheme::Dark) => AppColor::new_rgb(255, 255, 255),
        ("text_highlight", ColorScheme::Light) => AppColor::new_rgb(0, 0, 0),

        // Primary/accent colors
        ("primary", ColorScheme::Dark) => AppColor::new_rgb(100, 180, 255),
        ("primary", ColorScheme::Light) => AppColor::new_rgb(0, 100, 180),

        // Success/green
        ("success", ColorScheme::Dark) => AppColor::new_rgb(0, 255, 0),
        ("success", ColorScheme::Light) => AppColor::new_rgb(0, 150, 0),

        // Danger/red
        ("danger", ColorScheme::Dark) => AppColor::new_rgb(255, 80, 80),
        ("danger", ColorScheme::Light) => AppColor::new_rgb(200, 0, 0),

        // Warning/yellow
        ("warning", ColorScheme::Dark) => AppColor::new_rgb(255, 200, 0),
        ("warning", ColorScheme::Light) => AppColor::new_rgb(180, 140, 0),

        // Hidden/invisible
        ("hidden", ColorScheme::Dark) => AppColor::new_rgb(90, 90, 90),
        ("hidden", ColorScheme::Light) => AppColor::new_rgb(160, 160, 160),

        // Box Title
        ("box_title", ColorScheme::Dark) => AppColor::new_rgb(255, 255, 255),
        ("box_title", ColorScheme::Light) => AppColor::new_rgb(0, 0, 0),

        // Selected text (inverse)
        ("selected_text", ColorScheme::Dark) => AppColor::new_rgb(200, 200, 200),
        ("selected_text", ColorScheme::Light) => AppColor::new_rgb(50, 50, 50),

        // Ghost text (muted)
        ("ghost", ColorScheme::Dark) => AppColor::new_rgb(100, 100, 100),
        ("ghost", ColorScheme::Light) => AppColor::new_rgb(150, 150, 150),

        // Focus indicator
        ("focus", ColorScheme::Dark) => AppColor::new_rgb(255, 215, 0),
        ("focus", ColorScheme::Light) => AppColor::new_rgb(180, 140, 0),

        // Not found/default - return a neutral color that works on both
        _ => AppColor::new_rgb(180, 180, 180),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_scheme_variants() {
        assert!(!ColorScheme::Dark.is_system());
        assert!(!ColorScheme::Light.is_system());
        assert!(ColorScheme::System.is_system());
    }

    #[test]
    fn test_color_scheme_default() {
        assert_eq!(ColorScheme::default(), ColorScheme::System);
    }

    #[test]
    fn test_color_scheme_function_with_system() {
        let terminal = ColorScheme::Light;
        let result = color_scheme(terminal);
        assert_eq!(result, ColorScheme::Light);
    }

    #[test]
    fn test_color_scheme_function_with_dark_override() {
        set_color_scheme(ColorScheme::Dark);
        let result = color_scheme(ColorScheme::Light);
        assert_eq!(result, ColorScheme::Dark);
        set_color_scheme(ColorScheme::System);
    }

    #[test]
    fn test_color_scheme_function_with_light_override() {
        set_color_scheme(ColorScheme::Light);
        let result = color_scheme(ColorScheme::Dark);
        assert_eq!(result, ColorScheme::Light);
        set_color_scheme(ColorScheme::System);
    }

    #[test]
    fn test_scheme_aware_color_selection_dark() {
        let color = scheme_aware_color("selection", ColorScheme::Dark);
        assert_eq!(color.rgb.unwrap(), (0, 0, 170));
    }

    #[test]
    fn test_scheme_aware_color_selection_light() {
        let color = scheme_aware_color("selection", ColorScheme::Light);
        assert_eq!(color.rgb.unwrap(), (170, 170, 255));
    }

    #[test]
    fn test_scheme_aware_color_active_border_dark() {
        let color = scheme_aware_color("active_border", ColorScheme::Dark);
        assert_eq!(color.rgb.unwrap(), (0, 180, 0));
    }

    #[test]
    fn test_scheme_aware_color_active_border_light() {
        let color = scheme_aware_color("active_border", ColorScheme::Light);
        assert_eq!(color.rgb.unwrap(), (0, 100, 0));
    }

    #[test]
    fn test_scheme_aware_color_text_dark() {
        let color = scheme_aware_color("text", ColorScheme::Dark);
        assert_eq!(color.rgb.unwrap(), (200, 200, 200));
    }

    #[test]
    fn test_scheme_aware_color_text_light() {
        let color = scheme_aware_color("text", ColorScheme::Light);
        assert_eq!(color.rgb.unwrap(), (50, 50, 50));
    }

    #[test]
    fn test_scheme_aware_color_unknown() {
        let color = scheme_aware_color("unknown_color", ColorScheme::Dark);
        assert_eq!(color.rgb.unwrap(), (180, 180, 180));
    }

    #[test]
    fn test_scheme_aware_color_system_defaults_to_dark() {
        let color = scheme_aware_color("selection", ColorScheme::System);
        assert_eq!(color.rgb.unwrap(), (0, 0, 170));
    }
}

// Ported from ./references/lazygit-master/pkg/theme/gocui.go

use ratatui::style::Color;

/// GetGocuiAttribute gets the ratatui color attribute from the string
pub fn get_gocui_attribute(key: &str) -> Color {
    if is_valid_hex_value(key) {
        return hex_to_color(key);
    }

    match key {
        "default" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "bold" => Color::Reset,      // Attributes handled separately
        "reverse" => Color::Reset,   // Attributes handled separately
        "underline" => Color::Reset, // Attributes handled separately
        _ => Color::White,
    }
}

/// GetGocuiStyle converts a list of attribute keys into a Color with modifiers applied
/// Note: In ratatui, colors and attributes are separate, so this returns the base Color
/// and the caller should apply modifiers separately
pub fn get_gocui_style(keys: &[&str]) -> Color {
    let mut color = Color::White;
    for key in keys {
        let attr = get_gocui_attribute(key);
        // Use the last non-attribute color
        if !matches!(attr, Color::Reset) || *key == "default" {
            color = attr;
        }
    }
    color
}

/// is_valid_hex_value checks if a string is a valid hex color (e.g., "#FF0000" or "FF0000")
fn is_valid_hex_value(key: &str) -> bool {
    let hex = key.trim_start_matches('#');
    hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// hex_to_color converts a hex string to a ratatui Color
fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Color::White;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);

    Color::Rgb(r, g, b)
}

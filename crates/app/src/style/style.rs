// Ported from ./references/lazygit-master/pkg/theme/style.go

use std::collections::HashMap;

use ratatui::style::Color;

use super::basic_styles::{color_map, ColorMapEntry};
use super::color::AppColor;
use super::text_style::TextStyle;

/// GetTextStyle creates a TextStyle from a list of style keys
pub fn get_text_style(keys: &[&str], background: bool) -> TextStyle {
    let mut style = TextStyle::new();

    for key in keys {
        match *key {
            "bold" => {
                style = style.set_bold();
            }
            "reverse" => {
                style = style.set_reverse();
            }
            "underline" => {
                style = style.set_underline();
            }
            "strikethrough" => {
                style = style.set_strikethrough();
            }
            _ => {
                if let Some(entry) = color_map().get(*key) {
                    let color_style = if background {
                        entry.background
                    } else {
                        entry.foreground
                    };
                    style = style.merge_style(color_style);
                } else if is_valid_hex_value(key) {
                    let color = hex_to_app_color(key, background);
                    if background {
                        style = style.set_bg(color);
                    } else {
                        style = style.set_fg(color);
                    }
                }
            }
        }
    }

    style
}

/// is_valid_hex_value checks if a string is a valid hex color
fn is_valid_hex_value(key: &str) -> bool {
    let hex = key.trim_start_matches('#');
    hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// hex_to_app_color converts a hex string to an AppColor
fn hex_to_app_color(hex: &str, _background: bool) -> AppColor {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return AppColor::new_basic(Color::White);
    }

    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);

    AppColor::new_rgb(r, g, b)
}

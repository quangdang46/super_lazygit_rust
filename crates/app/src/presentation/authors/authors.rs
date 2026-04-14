use std::collections::HashMap;
use std::sync::RwLock;

use crate::style::text_style::TextStyle;

static AUTHOR_INITIAL_CACHE: RwLock<HashMap<String, String>> = RwLock::new(HashMap::new());
static AUTHOR_NAME_CACHE: RwLock<HashMap<(String, i32), String>> = RwLock::new(HashMap::new());
static AUTHOR_STYLE_CACHE: RwLock<HashMap<String, TextStyle>> = RwLock::new(HashMap::new());

const AUTHOR_NAME_WILDCARD: &str = "*";

pub fn short_author(author_name: &str) -> String {
    {
        let cache = AUTHOR_INITIAL_CACHE.read().unwrap();
        if let Some(value) = cache.get(author_name) {
            return value.clone();
        }
    }

    let initials = get_initials(author_name);
    if initials.is_empty() {
        return String::new();
    }

    let value = author_style(author_name).sprint(&initials);
    {
        let mut cache = AUTHOR_INITIAL_CACHE.write().unwrap();
        cache.insert(author_name.to_string(), value.clone());
    }

    value
}

pub fn long_author(author_name: &str, length: i32) -> String {
    let cache_key = (author_name.to_string(), length);
    {
        let cache = AUTHOR_NAME_CACHE.read().unwrap();
        if let Some(value) = cache.get(&cache_key) {
            return value.clone();
        }
    }

    let truncated_name = truncate_with_ellipsis(author_name, length);
    let value = author_style(author_name).sprint(&truncated_name);
    {
        let mut cache = AUTHOR_NAME_CACHE.write().unwrap();
        cache.insert(cache_key, value.clone());
    }

    value
}

pub fn author_with_length(author_name: &str, length: i32) -> String {
    if length < 2 {
        return String::new();
    }

    if length == 2 {
        return short_author(author_name);
    }

    long_author(author_name, length)
}

pub fn author_style(author_name: &str) -> TextStyle {
    {
        let cache = AUTHOR_STYLE_CACHE.read().unwrap();
        if let Some(value) = cache.get(author_name) {
            return *value;
        }
    }

    let value = if let Some(wildcard_style) = {
        let cache = AUTHOR_STYLE_CACHE.read().unwrap();
        cache.get(AUTHOR_NAME_WILDCARD).copied()
    } {
        wildcard_style
    } else {
        true_color_style(author_name)
    };

    {
        let mut cache = AUTHOR_STYLE_CACHE.read().unwrap();
        cache.insert(author_name.to_string(), value);
    }

    author_style(author_name)
}

fn true_color_style(str: &str) -> TextStyle {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    str.hash(&mut hasher);
    let hash = hasher.finish();

    let h = ((hash >> 0) & 0xFF) as f64 / 255.0 * 360.0;
    let s = 0.6 + 0.4 * ((hash >> 8) & 0xFF) as f64 / 255.0;
    let l = 0.4 + ((hash >> 16) & 0xFF) as f64 / 255.0 * 0.2;

    let (r, g, b) = hsl_to_rgb(h, s, l);

    let color = crate::style::color::AppColor::Rgb(r, g, b);
    TextStyle::new().set_fg(color)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = match h as i32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

fn get_initials(author_name: &str) -> String {
    if author_name.is_empty() {
        return String::new();
    }

    let mut chars = author_name.chars();
    if let Some(first) = chars.next() {
        if chars.next().is_some() {
            let split: Vec<&str> = author_name.split_whitespace().collect();
            if split.len() == 1 {
                return limit_str(author_name, 2);
            }
            return format!(
                "{}{}",
                limit_str(split[0], 1),
                limit_str(split[1], 1)
            );
        }
    }

    limit_str(author_name, 2)
}

fn limit_str(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().take(max_chars).collect();
    chars.into_iter().collect()
}

fn truncate_with_ellipsis(s: &str, max_length: i32) -> String {
    if max_length < 0 {
        return s.to_string();
    }
    let max_length = max_length as usize;
    if s.len() <= max_length {
        return s.to_string();
    }

    let mut result = String::new();
    let mut char_count = 0;
    for c in s.chars() {
        let c_width = if c.is_ascii() { 1 } else { 2 };
        if char_count + c_width > max_length - 1 {
            break;
        }
        result.push(c);
        char_count += c_width;
    }
    result.push('…');
    result
}

pub fn set_custom_authors(custom_author_colors: HashMap<String, String>) {
    let mut cache = AUTHOR_STYLE_CACHE.write().unwrap();
    *cache = custom_author_colors
        .into_iter()
        .map(|(name, color| {
            let style = parse_color_string(&color);
            (name, style)
        })
        .collect();
}

fn parse_color_string(color: &str) -> TextStyle {
    if color.starts_with('#') && color.len() == 7 {
        if let Ok(r) = u8::from_str_radix(&color[1..3], 16) {
            if let Ok(g) = u8::from_str_radix(&color[3..5], 16) {
                if let Ok(b) = u8::from_str_radix(&color[5..7], 16) {
                    let app_color = crate::style::color::AppColor::Rgb(r, g, b);
                    return TextStyle::new().set_fg(app_color);
                }
            }
        }
    }
    TextStyle::new()
}

//! Author styling for commit presentation.
//!
//! Ports `lazygit/pkg/gui/presentation/authors/authors.go` to Rust with lazygit parity.
//! Generates deterministic colors from author names and provides length-aware display.

use ratatui::style::{Color, Style};

/// Generate a deterministic RGB color from a string using hash-based HSL.
///
/// Parity: `authors.trueColorStyle` in Go uses MD5 + colorful.Hsl.
/// This uses a simple FNV-1a-inspired hash for deterministic color generation
/// without requiring external crypto/hash dependencies.
fn author_color(name: &str) -> Color {
    let hash = hash_bytes(name.as_bytes());
    let h = (hash[0] as f64 / 255.0) * 360.0;
    let s = 0.6 + 0.4 * (hash[1] as f64 / 255.0);
    let l = 0.4 + (hash[2] as f64 / 255.0) * 0.2;
    let (r, g, b) = hsl_to_rgb(h, s, l);
    Color::Rgb(r, g, b)
}

/// Simple deterministic hash producing 4 bytes from input.
/// Uses FNV-1a-style mixing for each byte position.
fn hash_bytes(data: &[u8]) -> [u8; 4] {
    let mut result = [0u8; 4];
    for (i, &byte) in data.iter().enumerate() {
        result[i % 4] = result[i % 4]
            .wrapping_mul(31)
            .wrapping_add(byte)
            .wrapping_add(result[(i + 1) % 4]);
    }
    // Extra mixing pass for better distribution
    for i in 0..4 {
        result[i] = result[i].wrapping_mul(97).wrapping_add(result[(i + 2) % 4]);
    }
    result
}

/// Convert HSL color to RGB.
///
/// Parity: matches `colorful.Hsl` in Go with same H→RGB algorithm.
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

/// Extract author initials for short display.
///
/// Parity: `authors.getInitials` in Go.
/// - For wide (CJK) graphemes, returns the first grapheme.
/// - For single-word names, returns first two characters.
/// - For multi-word names, returns first letter of first two words.
pub fn author_initials(author_name: &str) -> String {
    if author_name.is_empty() {
        return String::new();
    }

    let mut chars = author_name.chars();
    let first_char = chars.next().unwrap();

    // Wide character (CJK etc.) - just use first character
    if first_char.len_utf8() > 1 || first_char as u32 > 0x7F {
        return first_char.to_string();
    }

    let parts: Vec<&str> = author_name.split_whitespace().collect();
    if parts.len() == 1 {
        let cs: Vec<char> = parts[0].chars().collect();
        cs.into_iter().take(2).collect()
    } else {
        let left: String = parts[0].chars().take(1).collect();
        let right: String = parts[1].chars().take(1).collect();
        format!("{left}{right}")
    }
}

/// Return an author representation that fits into a given maximum length.
///
/// Parity: `authors.AuthorWithLength` in Go.
/// - length < 2: empty string
/// - length == 2: initials
/// - length > 2: truncated full name
pub fn author_with_length(author_name: &str, length: usize) -> String {
    if length < 2 || author_name.is_empty() {
        return String::new();
    }
    if length == 2 {
        return author_initials(author_name);
    }

    let count = author_name.chars().count();
    if count <= length {
        return author_name.to_string();
    }

    let mut truncated: String = author_name.chars().take(length - 1).collect();
    truncated.push('\u{2026}'); // ellipsis
    truncated
}

/// Get a styled Span for the author name with deterministic color.
///
/// Parity: combines `authors.AuthorWithLength` and `authors.AuthorStyle`.
pub fn author_span(author_name: &str, length: usize) -> ratatui::text::Span<'static> {
    let text = author_with_length(author_name, length);
    let style = Style::default().fg(author_color(author_name));
    ratatui::text::Span::styled(text, style)
}

/// Get the author style (deterministic color) for a given author name.
///
/// Parity: `authors.AuthorStyle` in Go.
pub fn author_style(author_name: &str) -> Style {
    Style::default().fg(author_color(author_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_initials_extracts_from_multi_word_name() {
        assert_eq!(author_initials("John Doe"), "JD");
        assert_eq!(author_initials("Alice Bob Carol"), "AB");
    }

    #[test]
    fn author_initials_single_word_returns_first_two_chars() {
        assert_eq!(author_initials("john"), "jo");
    }

    #[test]
    fn author_initials_empty_returns_empty() {
        assert!(author_initials("").is_empty());
    }

    #[test]
    fn author_initials_wide_char_returns_first_char() {
        assert_eq!(author_initials("太郎"), "太");
    }

    #[test]
    fn author_with_length_returns_empty_for_short_length() {
        assert!(author_with_length("John", 0).is_empty());
        assert!(author_with_length("John", 1).is_empty());
    }

    #[test]
    fn author_with_length_returns_initials_for_length_two() {
        assert_eq!(author_with_length("John Doe", 2), "JD");
    }

    #[test]
    fn author_with_length_truncates_long_names() {
        let result = author_with_length("VeryLongAuthorName", 8);
        assert_eq!(result.chars().count(), 8);
        assert!(result.ends_with('\u{2026}'));
    }

    #[test]
    fn author_with_length_keeps_short_names() {
        assert_eq!(author_with_length("Al", 5), "Al");
    }

    #[test]
    fn author_color_is_deterministic() {
        let c1 = author_color("John Doe");
        let c2 = author_color("John Doe");
        assert_eq!(c1, c2);

        // Different names should (very likely) produce different colors
        let c3 = author_color("Jane Smith");
        assert_ne!(c1, c3);
    }

    #[test]
    fn hsl_to_rgb_produces_valid_rgb() {
        let (r, g, b) = hsl_to_rgb(0.0, 1.0, 0.5);
        assert_eq!(r, 255); // Pure red
        assert_eq!(g, 0);
        assert_eq!(b, 0);

        let (r, g, b) = hsl_to_rgb(120.0, 1.0, 0.5);
        assert_eq!(r, 0);
        assert_eq!(g, 255); // Pure green
        assert_eq!(b, 0);

        let (r, g, b) = hsl_to_rgb(240.0, 1.0, 0.5);
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 255); // Pure blue
    }
}

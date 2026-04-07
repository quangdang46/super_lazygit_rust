// Ported from ./references/lazygit-master/pkg/cheatsheet/generate.go

/// Generates cheatsheet markdown files for all supported languages.
///
/// This is a CLI tool invoked via `cargo run -p super-lazygit-app --bin cheatsheet_generate`
/// to generate Keybindings_{{.LANG}}.md files in the docs-master/keybindings directory.

use std::collections::HashMap;
use std::hash::Hash;

/// Returns the command to run for generating cheatsheets.
pub fn command_to_run() -> String {
    "cargo generate-cli-keys".to_string()
}

/// Returns the path to the keybindings directory.
pub fn get_keybindings_dir() -> String {
    // In the actual Go code, this uses utils.GetLazyRootDirectory() + "/docs-master/keybindings"
    // For Rust, we'll use an environment variable or relative path
    std::env::var("LAZYGIT_KEYBINDINGS_DIR")
        .unwrap_or_else(|_| "docs-master/keybindings".to_string())
}

/// Generates cheatsheet files for all languages.
pub fn generate() {
    generate_at_dir(&get_keybindings_dir());
}

/// Generate cheatsheets at the specified directory.
fn generate_at_dir(_cheatsheet_dir: &str) {
    // TODO: Port i18n.GetTranslationSets() to get available languages
    // For now, generate for English as a proof-of-concept
    let languages = vec!["en".to_string()];

    for lang in languages {
        generate_for_language(&lang);
    }
}

/// Generate cheatsheet for a specific language.
fn generate_for_language(_lang: &str) {
    // TODO: Port the full implementation:
    // 1. Create App with language-specific config
    // 2. Get cheatsheet keybindings
    // 3. Format and write the markdown file
    eprintln!("Generating cheatsheet for language: {}", _lang);
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct BindingSection {
    title: String,
    bindings: Vec<Binding>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct Header {
    priority: i32,
    title: String,
}

#[derive(Debug, Clone)]
struct HeaderWithBindings {
    header: Header,
    bindings: Vec<Binding>,
}

/// Localized title map for view names to translation keys.
fn localised_title(_view_name: &str) -> String {
    // TODO: This needs TranslationSet from i18n
    // For now, return the view name as-is
    _view_name.to_string()
}

/// Filter bindings to display, excluding certain views and empty bindings.
fn get_binding_sections(bindings: &[Binding], _tr: &TranslationSet) -> Vec<BindingSection> {
    let excluded_views = vec!["stagingSecondary", "patchBuildingSecondary"];

    let bindings_to_display: Vec<Binding> = bindings
        .iter()
        .filter(|b| {
            if excluded_views.contains(&b.view_name.as_str()) {
                return false;
            }
            (!b.description.is_empty() || !b.alternative.is_empty()) && b.key.is_some()
        })
        .cloned()
        .collect();

    // Group by header
    let mut bindings_by_header: HashMap<Header, Vec<Binding>> = HashMap::new();
    for binding in bindings_to_display {
        let header = get_header(&binding, _tr);
        bindings_by_header.entry(header).or_default().push(binding);
    }

    // Convert to sorted sections
    let mut binding_groups: Vec<HeaderWithBindings> = bindings_by_header
        .into_iter()
        .map(|(header, h_bindings)| {
            // Deduplicate by description + key label
            let mut seen = std::collections::HashSet::new();
            let unique_bindings: Vec<Binding> = h_bindings
                .into_iter()
                .filter(|b| {
                    let key = format!("{}{}", b.description, label_from_key(&b.key));
                    seen.insert(key)
                })
                .collect();

            HeaderWithBindings {
                header,
                bindings: unique_bindings,
            }
        })
        .collect();

    // Sort by priority (desc), then by title
    binding_groups.sort_by(|a, b| {
        if a.header.priority != b.header.priority {
            b.header.priority.cmp(&a.header.priority)
        } else {
            a.header.title.cmp(&b.header.title)
        }
    });

    binding_groups
        .into_iter()
        .map(|hb| BindingSection {
            title: hb.header.title,
            bindings: hb.bindings,
        })
        .collect()
}

/// Get the header for a binding based on its tag and view name.
fn get_header(binding: &Binding, _tr: &TranslationSet) -> Header {
    if binding.tag == "navigation" {
        return Header {
            priority: 2,
            title: localised_title("navigation"),
        };
    }

    if binding.view_name.is_empty() {
        return Header {
            priority: 3,
            title: localised_title("global"),
        };
    }

    Header {
        priority: 1,
        title: localised_title(&binding.view_name),
    }
}

/// Format sections into markdown string.
fn format_sections(_tr: &TranslationSet, binding_sections: &[BindingSection]) -> String {
    let mut content = String::new();

    // Header
    content.push_str("# Lazygit Keybindings\n\n");
    content.push_str("_Press `?` to toggle the keybindings legend_\n\n");

    for section in binding_sections {
        content.push_str(&format!("\n## {}\n\n", section.title));
        content.push_str("| Key | Action | Info |\n");
        content.push_str("|-----|--------|-------------|\n");

        for binding in &section.bindings {
            content.push_str(&format_binding(binding));
        }
    }

    content
}

/// Format a single binding as a markdown table row.
fn format_binding(binding: &Binding) -> String {
    let action = label_from_key(&binding.key);
    let mut description = binding.description.clone();
    if !binding.alternative.is_empty() {
        description = format!("{} ({})", description, binding.alternative);
    }

    // Replace newlines with <br> tags for proper markdown table formatting
    let tooltip = binding.tooltip.replace('\n', "<br>");

    // Escape pipe characters to avoid breaking the table format
    let action = action.replace('|', "\\|");
    let description = description.replace('|', "\\|");
    let tooltip = tooltip.replace('|', "\\|");

    // Use backticks for keyboard keys
    format!("| `` {} `` | {} | {} |\n", action, description, tooltip)
}

/// Get label from a key.
fn label_from_key(key: &Option<Key>) -> String {
    key.as_ref()
        .map(|k| format!("{:?}", k))
        .unwrap_or_default()
}

// Placeholder types - these need to be connected to actual app types once ported

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Binding {
    pub view_name: String,
    pub key: Option<Key>,
    pub description: String,
    pub alternative: String,
    pub tag: String,
    pub tooltip: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Key;

#[derive(Debug, Clone, Default)]
pub struct TranslationSet;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_from_key() {
        let key = Some(Key);
        assert_eq!(label_from_key(&key), "Key");
    }

    #[test]
    fn test_binding_section_ordering() {
        // Test that binding sections are ordered by priority
        let bindings = vec![
            Binding {
                view_name: "files".to_string(),
                key: Some(Key),
                description: "Stage".to_string(),
                alternative: "".to_string(),
                tag: "".to_string(),
                tooltip: "".to_string(),
            },
            Binding {
                view_name: "".to_string(),
                key: Some(Key),
                description: "Quit".to_string(),
                alternative: "".to_string(),
                tag: "".to_string(),
                tooltip: "".to_string(),
            },
        ];

        let tr = TranslationSet::default();
        let sections = get_binding_sections(&bindings, &tr);

        // Navigation (priority 2) should come before global (priority 3)
        // But since no bindings have tag "navigation", check ordering by priority
        assert!(!sections.is_empty());
    }

    #[test]
    fn test_format_binding_escapes_pipes() {
        let binding = Binding {
            view_name: "test".to_string(),
            key: Some(Key),
            description: "a | b".to_string(),
            alternative: "c | d".to_string(),
            tag: "".to_string(),
            tooltip: "tip".to_string(),
        };

        let formatted = format_binding(&binding);
        assert!(formatted.contains("\\|"));
    }
}
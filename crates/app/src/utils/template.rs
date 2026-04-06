use std::collections::HashMap;

pub fn resolve_placeholder_string(s: &str, arguments: &HashMap<String, String>) -> String {
    let mut old_news = Vec::with_capacity(arguments.len() * 4);
    for (key, value) in arguments {
        old_news.push(format!("{{{{{key}}}}"));
        old_news.push(value.clone());
        old_news.push(format!("{{.{key}}}"));
        old_news.push(value.clone());
    }

    let replacer = old_news.chunks(2).filter_map(|chunk| {
        if chunk.len() == 2 {
            Some((chunk[0].as_str(), chunk[1].as_str()))
        } else {
            None
        }
    });

    let mut result = s.to_string();
    for (from, to) in replacer {
        result = result.replace(from, to);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_placeholder_string() {
        let mut args = HashMap::new();
        args.insert("name".to_string(), "Alice".to_string());
        args.insert("age".to_string(), "30".to_string());

        let template = "Hello {{name}}, you are {{.age}} years old";
        let result = resolve_placeholder_string(template, &args);
        assert_eq!(result, "Hello Alice, you are 30 years old");
    }

    #[test]
    fn test_resolve_placeholder_string_no_match() {
        let args = HashMap::new();
        let template = "Hello {{name}}";
        let result = resolve_placeholder_string(template, &args);
        assert_eq!(result, "Hello {{name}}");
    }

    #[test]
    fn test_resolve_placeholder_string_empty_args() {
        let args = HashMap::new();
        let template = "Hello {{name}}";
        let result = resolve_placeholder_string(template, &args);
        assert_eq!(result, "Hello {{name}}");
    }
}

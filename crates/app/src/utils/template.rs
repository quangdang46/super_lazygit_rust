use std::collections::HashMap;

pub fn resolve_placeholder_string(s: &str, arguments: &HashMap<String, String>) -> String {
    let mut replacements = Vec::with_capacity(arguments.len() * 6);
    for (key, value) in arguments {
        replacements.push((format!("{{{{{key}}}}}", key = key), value.clone()));
        replacements.push((format!("{{.{key}}}"), value.clone()));
        replacements.push((format!("{{{key}}}"), value.clone()));
    }

    let mut result = s.to_string();
    for (from, to) in replacements {
        result = result.replace(&from, &to);
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

        let template = "Hello {{name}}, you are {age} years old";
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

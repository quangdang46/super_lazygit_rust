use serde_yaml::Value;

pub fn lookup_key<'a>(node: &'a Value, key: &str) -> Option<(&'a str, &'a Value)> {
    if let Some(mapping) = node.as_mapping() {
        for (k, v) in mapping.iter() {
            if let Some(k_str) = k.as_str() {
                if k_str == key {
                    return Some((k_str, v));
                }
            }
        }
    }
    None
}

pub fn remove_key(node: &mut Value, key: &str) -> Option<Value> {
    if let Some(mapping) = node.as_mapping_mut() {
        if let Some((k, v)) = mapping.iter().find(|(k, _)| k.as_str() == Some(key)) {
            let removed = v.clone();
            mapping.remove(k);
            return Some(removed);
        }
    }
    None
}

pub fn rename_yaml_key(root: &mut Value, path: &[String], new_key: &str) -> Result<bool, String> {
    if path.is_empty() {
        return Ok(false);
    }

    let mut current = root;
    for (i, key) in path.iter().enumerate() {
        if i == path.len() - 1 {
            if let Some(mapping) = current.as_mapping_mut() {
                if mapping.contains_key(&Value::String(new_key.to_string())) {
                    return Err(format!("new key '{}' already exists", new_key));
                }
                if let Some((k, _)) = mapping
                    .iter()
                    .find(|(k, _)| k.as_str() == Some(key.as_str()))
                {
                    let v = mapping.remove(k).unwrap();
                    mapping.insert(Value::String(new_key.to_string()), v);
                    return Ok(true);
                }
            }
        } else {
            if let Some(mapping) = current.as_mapping() {
                if let Some((_, next)) = lookup_key(mapping, key) {
                    current = next;
                } else {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }
    }
    Ok(false)
}

pub fn walk<F>(node: &Value, path: &str, callback: &mut F) -> Result<(), String>
where
    F: FnMut(&Value, &str),
{
    callback(node, path);

    match node {
        Value::Mapping(mapping) => {
            for (k, v) in mapping.iter() {
                let child_path = if path.is_empty() {
                    k.as_str().unwrap_or("?").to_string()
                } else {
                    format!(".{}.{}", path, k.as_str().unwrap_or("?"))
                };
                walk(v, &child_path, callback)?;
            }
        }
        Value::Sequence(seq) => {
            for (i, v) in seq.iter().enumerate() {
                let child_path = format!("{}[{}]", path, i);
                walk(v, &child_path, callback)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn yaml_marshal(node: &Value) -> Result<String, String> {
    serde_yaml::to_string(node).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml::Mapping;

    #[test]
    fn test_lookup_key() {
        let yaml = serde_yaml::from_str::<Value>("foo: bar\nbaz: qux").unwrap();
        let (k, v) = lookup_key(&yaml, "foo").unwrap();
        assert_eq!(k, "foo");
        assert_eq!(v.as_str(), Some("bar"));
    }

    #[test]
    fn test_lookup_key_not_found() {
        let yaml = serde_yaml::from_str::<Value>("foo: bar").unwrap();
        assert!(lookup_key(&yaml, "missing").is_none());
    }

    #[test]
    fn test_remove_key() {
        let mut yaml = serde_yaml::from_str::<Value>("foo: bar\nbaz: qux").unwrap();
        let removed = remove_key(&mut yaml, "foo");
        assert!(removed.is_some());
        assert!(lookup_key(&yaml, "foo").is_none());
    }

    #[test]
    fn test_rename_yaml_key() {
        let mut yaml = serde_yaml::from_str::<Value>("foo: bar").unwrap();
        let result = rename_yaml_key(&mut yaml, &["foo".to_string()], "new_foo");
        assert!(result.is_ok());
        assert!(lookup_key(&yaml, "new_foo").is_some());
        assert!(lookup_key(&yaml, "foo").is_none());
    }

    #[test]
    fn test_yaml_marshal() {
        let yaml = serde_yaml::from_str::<Value>("foo: bar").unwrap();
        let result = yaml_marshal(&yaml);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("foo: bar"));
    }
}

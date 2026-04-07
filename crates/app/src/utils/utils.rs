use regex::Regex;

pub fn sort_range(x: i32, y: i32) -> (i32, i32) {
    if x < y {
        (x, y)
    } else {
        (y, x)
    }
}

pub fn as_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

pub fn modulo_with_wrap(n: i32, max: i32) -> i32 {
    if max == 0 {
        return 0;
    }

    if n >= max {
        n % max
    } else if n < 0 {
        max + n
    } else {
        n
    }
}

pub fn find_string_submatch<'a>(s: &'a str, regexp_str: &str) -> (bool, Vec<&'a str>) {
    let re = Regex::new(regexp_str).unwrap();
    let match_result = re.find(s);
    match match_result {
        Some(m) => {
            let matched: Vec<&'a str> = re
                .captures(s)
                .map(|c| {
                    c.iter()
                        .map(|opt| opt.map(|m| m.as_str()).unwrap_or(""))
                        .collect()
                })
                .unwrap_or_default();
            (true, matched)
        }
        None => (false, vec![]),
    }
}

pub fn must_convert_to_int(s: &str) -> i32 {
    s.parse::<i32>().expect("Failed to parse int")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_sort_range() {
        assert_eq!(sort_range(1, 2), (1, 2));
        assert_eq!(sort_range(2, 1), (1, 2));
        assert_eq!(sort_range(1, 1), (1, 1));
    }

    #[test]
    fn test_as_json() {
        let mut map = HashMap::new();
        map.insert("key", "value");
        let json = as_json(&map);
        assert!(json.contains("key"));
        assert!(json.contains("value"));
    }

    #[test]
    fn test_modulo_with_wrap() {
        assert_eq!(modulo_with_wrap(5, 3), 2);
        assert_eq!(modulo_with_wrap(7, 5), 2);
        assert_eq!(modulo_with_wrap(-1, 5), 4);
        assert_eq!(modulo_with_wrap(-6, 5), 4);
        assert_eq!(modulo_with_wrap(0, 3), 0);
        assert_eq!(modulo_with_wrap(5, 0), 0);
    }

    #[test]
    fn test_find_string_submatch() {
        let (matched, groups) = find_string_submatch("hello world", r"(\w+)");
        assert!(matched);
        assert!(!groups.is_empty());
    }

    #[test]
    fn test_find_string_submatch_no_match() {
        let (matched, _) = find_string_submatch("hello", r"\d+");
        assert!(!matched);
    }

    #[test]
    fn test_must_convert_to_int() {
        assert_eq!(must_convert_to_int("42"), 42);
        assert_eq!(must_convert_to_int("-10"), -10);
    }

    #[test]
    #[should_panic]
    fn test_must_convert_to_int_panic() {
        must_convert_to_int("not_a_number");
    }
}

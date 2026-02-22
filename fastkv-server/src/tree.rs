pub fn build_tree(items: &[(String, String)]) -> serde_json::Value {
    let mut root = serde_json::Map::new();
    for (key, value) in items {
        // Parse value as JSON, fallback to string if invalid
        let parsed_value = serde_json::from_str(value)
            .unwrap_or_else(|_| serde_json::Value::String(value.clone()));

        // Split key by '/' and nest
        let parts: Vec<&str> = key.split('/').collect();
        insert_nested(&mut root, &parts, parsed_value);
    }
    serde_json::Value::Object(root)
}

fn insert_nested(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    parts: &[&str],
    value: serde_json::Value,
) {
    if parts.is_empty() {
        return;
    }
    if parts.len() == 1 {
        obj.insert(parts[0].to_string(), value);
    } else {
        let entry = obj
            .entry(parts[0].to_string())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if let serde_json::Value::Object(ref mut nested) = entry {
            insert_nested(nested, &parts[1..], value);
        } else {
            tracing::warn!(
                target: "fastkv-server",
                key = parts[0],
                "tree path conflict: cannot nest under a scalar value"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_tree_basic() {
        let items = vec![
            ("profile/name".to_string(), "\"Alice\"".to_string()),
            (
                "profile/image/url".to_string(),
                "\"https://example.com\"".to_string(),
            ),
        ];
        let tree = build_tree(&items);
        assert_eq!(tree["profile"]["name"], "Alice");
        assert_eq!(tree["profile"]["image"]["url"], "https://example.com");
    }

    #[test]
    fn test_build_tree_non_json_value() {
        let items = vec![("key".to_string(), "plain text".to_string())];
        let tree = build_tree(&items);
        assert_eq!(tree["key"], "plain text");
    }

    #[test]
    fn test_build_tree_json_number() {
        let items = vec![("count".to_string(), "42".to_string())];
        let tree = build_tree(&items);
        assert_eq!(tree["count"], 42);
    }

    #[test]
    fn test_build_tree_empty() {
        let items: Vec<(String, String)> = vec![];
        let tree = build_tree(&items);
        assert_eq!(tree, serde_json::json!({}));
    }

    #[test]
    fn test_build_tree_single_key_no_slash() {
        let items = vec![("name".to_string(), "\"Bob\"".to_string())];
        let tree = build_tree(&items);
        assert_eq!(tree["name"], "Bob");
    }

    #[test]
    fn test_build_tree_deep_nesting() {
        let items = vec![("a/b/c/d".to_string(), "\"deep\"".to_string())];
        let tree = build_tree(&items);
        assert_eq!(tree["a"]["b"]["c"]["d"], "deep");
    }

    #[test]
    fn test_build_tree_conflict_value_then_object() {
        // If a key is set as a leaf value, a later key trying to nest under it
        // should not panic — the nested insert is simply skipped.
        let items = vec![
            ("a/b".to_string(), "\"leaf\"".to_string()),
            ("a/b/c".to_string(), "\"nested\"".to_string()),
        ];
        let tree = build_tree(&items);
        // "a/b" was set first as a leaf, so "a/b/c" can't nest under it
        assert_eq!(tree["a"]["b"], "leaf");
    }
}

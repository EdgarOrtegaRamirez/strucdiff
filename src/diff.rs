use serde_json::{Map, Value};
use std::collections::BTreeSet;

/// A single difference entry in a diff result
#[derive(Debug, Clone, PartialEq)]
pub struct DiffEntry {
    /// JSON path to the value (e.g., "config.host", "items[0].name")
    pub path: String,
    /// The kind of change
    pub kind: DiffKind,
    /// The old value (present for Changed and Removed)
    pub old_value: Option<String>,
    /// The new value (present for Changed and Added)
    pub new_value: Option<String>,
}

/// The kind of change at a path
#[derive(Debug, Clone, PartialEq)]
pub enum DiffKind {
    /// A value was added
    Added,
    /// A value was removed
    Removed,
    /// A value changed
    Changed,
}

/// Result of comparing two structured values
#[derive(Debug, Clone, PartialEq)]
pub struct DiffResult {
    /// Individual diff entries
    pub entries: Vec<DiffEntry>,
    /// Whether the two values are identical
    pub identical: bool,
}

impl DiffResult {
    pub fn new() -> Self {
        DiffResult {
            entries: Vec::new(),
            identical: true,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Compare two serde_json::Value trees and produce a DiffResult
pub fn diff_values(left: &Value, right: &Value, path: &str) -> DiffResult {
    let mut result = DiffResult::new();

    match (left, right) {
        (Value::Object(left_map), Value::Object(right_map)) => {
            diff_objects(left_map, right_map, path, &mut result);
        }
        (Value::Array(left_arr), Value::Array(right_arr)) => {
            diff_arrays(left_arr, right_arr, path, &mut result);
        }
        _ => {
            if left != right {
                result.entries.push(DiffEntry {
                    path: path.to_string(),
                    kind: DiffKind::Changed,
                    old_value: Some(format_value(left)),
                    new_value: Some(format_value(right)),
                });
                result.identical = false;
            }
        }
    }

    result
}

fn diff_objects(
    left: &Map<String, Value>,
    right: &Map<String, Value>,
    base_path: &str,
    result: &mut DiffResult,
) {
    let left_keys: BTreeSet<&String> = left.keys().collect();
    let right_keys: BTreeSet<&String> = right.keys().collect();

    // Keys in both — compare values
    for key in left_keys.intersection(&right_keys) {
        let child_path = if base_path.is_empty() {
            key.to_string()
        } else {
            format!("{}.{}", base_path, key)
        };

        let sub = diff_values(&left[*key], &right[*key], &child_path);
        if !sub.identical {
            result.identical = false;
            result.entries.extend(sub.entries);
        }
    }

    // Keys only in left — removed
    for key in left_keys.difference(&right_keys) {
        let child_path = if base_path.is_empty() {
            (*key).to_string()
        } else {
            format!("{}.{}", base_path, key)
        };
        result.entries.push(DiffEntry {
            path: child_path,
            kind: DiffKind::Removed,
            old_value: Some(format_value(&left[*key])),
            new_value: None,
        });
        result.identical = false;
    }

    // Keys only in right — added
    for key in right_keys.difference(&left_keys) {
        let child_path = if base_path.is_empty() {
            (*key).to_string()
        } else {
            format!("{}.{}", base_path, key)
        };
        result.entries.push(DiffEntry {
            path: child_path,
            kind: DiffKind::Added,
            old_value: None,
            new_value: Some(format_value(&right[*key])),
        });
        result.identical = false;
    }
}

fn diff_arrays(
    left: &[Value],
    right: &[Value],
    base_path: &str,
    result: &mut DiffResult,
) {
    let max_len = left.len().max(right.len());

    for i in 0..max_len {
        let child_path = format!("{}[{}]", base_path, i);

        match (left.get(i), right.get(i)) {
            (Some(lv), Some(rv)) => {
                let sub = diff_values(lv, rv, &child_path);
                if !sub.identical {
                    result.identical = false;
                    result.entries.extend(sub.entries);
                }
            }
            (Some(lv), None) => {
                result.entries.push(DiffEntry {
                    path: child_path,
                    kind: DiffKind::Removed,
                    old_value: Some(format_value(lv)),
                    new_value: None,
                });
                result.identical = false;
            }
            (None, Some(rv)) => {
                result.entries.push(DiffEntry {
                    path: child_path,
                    kind: DiffKind::Added,
                    old_value: None,
                    new_value: Some(format_value(rv)),
                });
                result.identical = false;
            }
            (None, None) => unreachable!(),
        }
    }
}

/// Format a serde_json::Value to a human-readable string
fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            if s.len() > 80 {
                format!("\"{}...\"", &s[..77])
            } else {
                format!("\"{}\"", s)
            }
        }
        Value::Array(arr) => {
            if arr.len() > 5 {
                format!("[{} items]", arr.len())
            } else {
                let items: Vec<String> = arr.iter().map(format_value).collect();
                format!("[{}]", items.join(", "))
            }
        }
        Value::Object(obj) => {
            format!("{{{}}}", obj.len())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;

    #[test]
    fn test_identical_primitives() {
        let result = diff_values(&Value::Bool(true), &Value::Bool(true), "");
        assert!(result.identical);
        assert!(result.entries.is_empty());
    }

    #[test]
    fn test_different_primitives() {
        let result = diff_values(&Value::from(1), &Value::from(2), "root");
        assert!(!result.identical);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].kind, DiffKind::Changed);
        assert_eq!(result.entries[0].path, "root");
    }

    #[test]
    fn test_added_key() {
        let mut left = Map::new();
        left.insert("a".to_string(), Value::from(1));
        let mut right = Map::new();
        right.insert("a".to_string(), Value::from(1));
        right.insert("b".to_string(), Value::from(2));

        let left_val = Value::Object(left);
        let right_val = Value::Object(right);
        let result = diff_values(&left_val, &right_val, "");

        assert!(!result.identical);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].kind, DiffKind::Added);
        assert_eq!(result.entries[0].path, "b");
    }

    #[test]
    fn test_removed_key() {
        let mut left = Map::new();
        left.insert("a".to_string(), Value::from(1));
        left.insert("b".to_string(), Value::from(2));
        let mut right = Map::new();
        right.insert("a".to_string(), Value::from(1));

        let left_val = Value::Object(left);
        let right_val = Value::Object(right);
        let result = diff_values(&left_val, &right_val, "");

        assert!(!result.identical);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].kind, DiffKind::Removed);
        assert_eq!(result.entries[0].path, "b");
    }

    #[test]
    fn test_nested_object_diff() {
        let left_val: Value = serde_json::from_str(r#"{"a": {"b": 1, "c": 2}}"#).unwrap();
        let right_val: Value = serde_json::from_str(r#"{"a": {"b": 1, "c": 3}}"#).unwrap();
        let result = diff_values(&left_val, &right_val, "");

        assert!(!result.identical);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].path, "a.c");
        assert_eq!(result.entries[0].kind, DiffKind::Changed);
    }

    #[test]
    fn test_array_diff() {
        let left_val: Value = serde_json::from_str(r#"[1, 2, 3]"#).unwrap();
        let right_val: Value = serde_json::from_str(r#"[1, 5, 3, 4]"#).unwrap();
        let result = diff_values(&left_val, &right_val, "");

        assert!(!result.identical);
        // [1] changed, [3] added
        assert_eq!(result.entries.len(), 2);
    }

    #[test]
    fn test_identical_deep_objects() {
        let left_val: Value = serde_json::from_str(
            r#"{"name": "test", "config": {"host": "localhost", "port": 8080}}"#,
        )
        .unwrap();
        let result = diff_values(&left_val, &left_val, "");
        assert!(result.identical);
    }

    #[test]
    fn test_null_values() {
        let left_val: Value = serde_json::json!({"a": null});
        let right_val: Value = serde_json::json!({"a": "not null"});
        let result = diff_values(&left_val, &right_val, "");
        assert!(!result.identical);
        assert_eq!(result.entries[0].path, "a");
        assert_eq!(result.entries[0].kind, DiffKind::Changed);
    }

    #[test]
    fn test_empty_vs_nonempty() {
        let left = Value::Object(Map::new());
        let mut right_map = Map::new();
        right_map.insert("key".to_string(), Value::from("val"));
        let right = Value::Object(right_map);
        let result = diff_values(&left, &right, "");
        assert!(!result.identical);
        assert_eq!(result.entries.len(), 1);
    }

    #[test]
    fn test_format_null() {
        assert_eq!(format_value(&Value::Null), "null");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_value(&Value::from(42)), "42");
    }

    #[test]
    fn test_format_string_truncated() {
        let long = "x".repeat(100);
        let formatted = format_value(&Value::String(long));
        assert!(formatted.len() < 90);
        assert!(formatted.ends_with("...\""));
    }
}
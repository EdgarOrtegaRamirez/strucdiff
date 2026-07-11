use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

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
    /// For Renamed entries: the original key name
    pub old_key: Option<String>,
    /// For Renamed entries: the new key name
    pub new_key: Option<String>,
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
    /// A key was renamed (old_key → new_key)
    Renamed,
}

/// Configuration for diff behavior
#[derive(Debug, Clone)]
pub struct DiffConfig {
    /// Enable rename detection for object keys
    pub rename_detection: bool,
    /// Similarity threshold for rename detection (0.0–1.0)
    pub rename_threshold: f64,
}

impl Default for DiffConfig {
    fn default() -> Self {
        DiffConfig {
            rename_detection: true,
            rename_threshold: 0.6,
        }
    }
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

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Compare two serde_json::Value trees and produce a DiffResult (with default config)
#[allow(dead_code)]
pub fn diff_values(left: &Value, right: &Value, path: &str) -> DiffResult {
    diff_values_with_config(left, right, path, &DiffConfig::default())
}

/// Compare two serde_json::Value trees with explicit configuration
pub fn diff_values_with_config(
    left: &Value,
    right: &Value,
    path: &str,
    config: &DiffConfig,
) -> DiffResult {
    let mut result = DiffResult::new();

    match (left, right) {
        (Value::Object(left_map), Value::Object(right_map)) => {
            diff_objects(left_map, right_map, path, config, &mut result);
        }
        (Value::Array(left_arr), Value::Array(right_arr)) => {
            diff_arrays(left_arr, right_arr, path, config, &mut result);
        }
        _ => {
            if left != right {
                result.entries.push(DiffEntry {
                    path: path.to_string(),
                    kind: DiffKind::Changed,
                    old_value: Some(format_value(left)),
                    new_value: Some(format_value(right)),
                    old_key: None,
                    new_key: None,
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
    config: &DiffConfig,
    result: &mut DiffResult,
) {
    let left_keys: BTreeSet<&String> = left.keys().collect();
    let right_keys: BTreeSet<&String> = right.keys().collect();

    // Detect renames: keys only in left matched to keys only in right by similarity
    let renames = if config.rename_detection {
        find_renames(&left_keys, &right_keys, config.rename_threshold)
    } else {
        BTreeMap::new()
    };

    // Track which keys have been handled by rename detection
    let mut handled_left: BTreeSet<&String> = BTreeSet::new();
    let mut handled_right: BTreeSet<&String> = BTreeSet::new();

    // Handle renamed keys first
    for (old_key, new_key) in &renames {
        let child_path = if base_path.is_empty() {
            (**new_key).to_string()
        } else {
            format!("{}.{}", base_path, new_key)
        };

        let sub = diff_values_with_config(&left[old_key], &right[new_key], &child_path, config);

        if !sub.identical {
            // Value also changed — report sub-entries as part of the rename
            result.identical = false;
            result.entries.extend(sub.entries);
        }

        // Always report the rename itself
        result.entries.push(DiffEntry {
            path: child_path,
            kind: DiffKind::Renamed,
            old_value: Some(format_value(&left[old_key])),
            new_value: Some(format_value(&right[new_key])),
            old_key: Some((**old_key).to_string()),
            new_key: Some((**new_key).to_string()),
        });
        result.identical = false;

        handled_left.insert(old_key);
        handled_right.insert(new_key);
    }

    // Keys in both — compare values (skip if handled by rename)
    for key in left_keys.intersection(&right_keys) {
        if handled_left.contains(key) || handled_right.contains(key) {
            continue;
        }

        let child_path = if base_path.is_empty() {
            (**key).to_string()
        } else {
            format!("{}.{}", base_path, key)
        };

        let sub = diff_values_with_config(&left[*key], &right[*key], &child_path, config);
        if !sub.identical {
            result.identical = false;
            result.entries.extend(sub.entries);
        }
    }

    // Keys only in left — removed (skip if handled by rename)
    for key in left_keys.difference(&right_keys) {
        if handled_left.contains(key) {
            continue;
        }

        let child_path = if base_path.is_empty() {
            (**key).to_string()
        } else {
            format!("{}.{}", base_path, key)
        };
        result.entries.push(DiffEntry {
            path: child_path,
            kind: DiffKind::Removed,
            old_value: Some(format_value(&left[*key])),
            new_value: None,
            old_key: None,
            new_key: None,
        });
        result.identical = false;
    }

    // Keys only in right — added (skip if handled by rename)
    for key in right_keys.difference(&left_keys) {
        if handled_right.contains(key) {
            continue;
        }

        let child_path = if base_path.is_empty() {
            (**key).to_string()
        } else {
            format!("{}.{}", base_path, key)
        };
        result.entries.push(DiffEntry {
            path: child_path,
            kind: DiffKind::Added,
            old_value: None,
            new_value: Some(format_value(&right[*key])),
            old_key: None,
            new_key: None,
        });
        result.identical = false;
    }
}

fn diff_arrays(
    left: &[Value],
    right: &[Value],
    base_path: &str,
    config: &DiffConfig,
    result: &mut DiffResult,
) {
    let max_len = left.len().max(right.len());

    for i in 0..max_len {
        let child_path = format!("{}[{}]", base_path, i);

        match (left.get(i), right.get(i)) {
            (Some(lv), Some(rv)) => {
                let sub = diff_values_with_config(lv, rv, &child_path, config);
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
                    old_key: None,
                    new_key: None,
                });
                result.identical = false;
            }
            (None, Some(rv)) => {
                result.entries.push(DiffEntry {
                    path: child_path,
                    kind: DiffKind::Added,
                    old_value: None,
                    new_value: Some(format_value(rv)),
                    old_key: None,
                    new_key: None,
                });
                result.identical = false;
            }
            (None, None) => unreachable!(),
        }
    }
}

/// Find renamed keys by matching keys-only-in-left to keys-only-in-right by similarity.
/// Returns a map of old_key → new_key.
fn find_renames(
    left_keys: &BTreeSet<&String>,
    right_keys: &BTreeSet<&String>,
    threshold: f64,
) -> BTreeMap<String, String> {
    let mut renames: BTreeMap<String, String> = BTreeMap::new();

    // Keys only in left (candidates for removal/rename)
    let left_only: Vec<&String> = left_keys.difference(right_keys).cloned().collect();
    // Keys only in right (candidates for addition/rename)
    let right_only: Vec<&String> = right_keys.difference(left_keys).cloned().collect();

    let mut used_right: BTreeSet<String> = BTreeSet::new();

    for lk in &left_only {
        let mut best_match: Option<&String> = None;
        let mut best_score = 0.0_f64;

        for rk in &right_only {
            if used_right.contains(*rk) {
                continue;
            }

            let score = key_similarity(lk, rk);
            if score > best_score && score > threshold {
                best_score = score;
                best_match = Some(*rk);
            }
        }

        if let Some(rk) = best_match {
            renames.insert((*lk).clone(), rk.clone());
            used_right.insert(rk.clone());
        }
    }

    renames
}

/// Compute similarity between two key names (0.0–1.0).
/// Normalizes keys (lowercase, separator unification) before Levenshtein.
fn key_similarity(a: &str, b: &str) -> f64 {
    let na = normalize_key(a);
    let nb = normalize_key(b);

    if na == nb {
        return 1.0;
    }

    let lev = levenshtein(&na, &nb);
    let max_len = na.len().max(nb.len());

    if max_len == 0 {
        return 0.0;
    }

    1.0 - (lev as f64) / (max_len as f64)
}

/// Normalize a key: lowercase, unify separators (-, _, ., space) to a single dash
fn normalize_key(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            result.push(c.to_ascii_lowercase());
        } else if c == '-' || c == '_' || c == '.' || c == ' ' {
            result.push('-');
        } else {
            result.push(c);
        }
    }
    result
}

/// Compute Levenshtein edit distance between two strings
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let la = a_chars.len();
    let lb = b_chars.len();

    if la == 0 {
        return lb;
    }
    if lb == 0 {
        return la;
    }

    // Use two rows for space efficiency
    let mut prev: Vec<usize> = (0..=lb).collect();
    let mut curr: Vec<usize> = vec![0; lb + 1];

    for i in 1..=la {
        curr[0] = i;
        for j in 1..=lb {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[lb]
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

    // ===== Rename detection tests =====

    #[test]
    fn test_rename_detection_basic() {
        // "userName" removed, "username" added — should be detected as rename
        let left_val: Value = serde_json::json!({"userName": "alice", "age": 30});
        let right_val: Value = serde_json::json!({"username": "alice", "age": 30});
        let result = diff_values(&left_val, &right_val, "");

        assert!(!result.identical);
        let renames: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.kind == DiffKind::Renamed)
            .collect();
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].old_key.as_deref(), Some("userName"));
        assert_eq!(renames[0].new_key.as_deref(), Some("username"));
    }

    #[test]
    fn test_rename_detection_separator() {
        // "user_name" → "user-name" should be detected as rename (same after normalization)
        let left_val: Value = serde_json::json!({"user_name": 42});
        let right_val: Value = serde_json::json!({"user-name": 42});
        let result = diff_values(&left_val, &right_val, "");

        assert!(!result.identical);
        let renames: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.kind == DiffKind::Renamed)
            .collect();
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].old_key.as_deref(), Some("user_name"));
        assert_eq!(renames[0].new_key.as_deref(), Some("user-name"));
    }

    #[test]
    fn test_rename_detection_disabled() {
        let config = DiffConfig {
            rename_detection: false,
            rename_threshold: 0.6,
        };
        let left_val: Value = serde_json::json!({"userName": "alice"});
        let right_val: Value = serde_json::json!({"username": "alice"});
        let result = diff_values_with_config(&left_val, &right_val, "", &config);

        // Should be separate add/remove, not rename
        let renames: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.kind == DiffKind::Renamed)
            .collect();
        assert!(renames.is_empty());
        assert!(result.entries.iter().any(|e| e.kind == DiffKind::Removed));
        assert!(result.entries.iter().any(|e| e.kind == DiffKind::Added));
    }

    #[test]
    fn test_rename_with_value_change() {
        // Key renamed AND value changed
        let left_val: Value = serde_json::json!({"userName": "alice"});
        let right_val: Value = serde_json::json!({"username": "bob"});
        let result = diff_values(&left_val, &right_val, "");

        let renames: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.kind == DiffKind::Renamed)
            .collect();
        assert_eq!(renames.len(), 1);
        // Should also report the value change
        assert!(result.entries.iter().any(|e| e.kind == DiffKind::Changed));
    }

    #[test]
    fn test_no_false_rename() {
        // Completely different keys should NOT be detected as renames
        let left_val: Value = serde_json::json!({"xyz": 1});
        let right_val: Value = serde_json::json!({"abc": 2});
        let result = diff_values(&left_val, &right_val, "");

        let renames: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.kind == DiffKind::Renamed)
            .collect();
        assert!(renames.is_empty());
        assert!(result.entries.iter().any(|e| e.kind == DiffKind::Removed));
        assert!(result.entries.iter().any(|e| e.kind == DiffKind::Added));
    }

    #[test]
    fn test_rename_in_nested_object() {
        let left_val: Value = serde_json::json!({"config": {"dataBaseHost": "localhost"}});
        let right_val: Value = serde_json::json!({"config": {"database_host": "localhost"}});
        let result = diff_values(&left_val, &right_val, "");

        let renames: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.kind == DiffKind::Renamed)
            .collect();
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].path, "config.database_host");
    }

    // ===== Levenshtein and key similarity tests =====

    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein("hello", "hello"), 0);
    }

    #[test]
    fn test_levenshtein_empty() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
    }

    #[test]
    fn test_levenshtein_substitution() {
        assert_eq!(levenshtein("cat", "bat"), 1);
    }

    #[test]
    fn test_levenshtein_insertion() {
        assert_eq!(levenshtein("cat", "cats"), 1);
    }

    #[test]
    fn test_key_similarity_identical() {
        assert_eq!(key_similarity("foo", "foo"), 1.0);
    }

    #[test]
    fn test_key_similarity_case_insensitive() {
        assert_eq!(key_similarity("FooBar", "foobar"), 1.0);
    }

    #[test]
    fn test_key_similarity_separator_unified() {
        assert_eq!(key_similarity("user_name", "user-name"), 1.0);
        assert_eq!(key_similarity("user.name", "user name"), 1.0);
    }

    #[test]
    fn test_key_similarity_different() {
        let score = key_similarity("abc", "xyz");
        assert!(score < 0.6); // Below threshold
    }

    #[test]
    fn test_normalize_key() {
        assert_eq!(normalize_key("FooBar"), "foobar");
        assert_eq!(normalize_key("user_name"), "user-name");
        assert_eq!(normalize_key("user.name"), "user-name");
        assert_eq!(normalize_key("user name"), "user-name");
        assert_eq!(normalize_key("USER-NAME"), "user-name");
    }
}

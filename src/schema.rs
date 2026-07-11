use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A node in an inferred schema tree
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaNode {
    /// JSON Schema type: "object", "array", "string", "number", "integer", "boolean", "null"
    pub schema_type: String,
    /// Path to this node (dot-notation, e.g., "config.host")
    pub path: String,
    /// For objects: child field schemas keyed by field name
    pub children: BTreeMap<String, SchemaNode>,
    /// For arrays: schema of the items (merged from all elements)
    pub items: Option<Box<SchemaNode>>,
    /// For objects: which fields are required (present in all observed instances)
    pub required: Vec<String>,
}

impl SchemaNode {
    /// Create a new schema node with the given type and path
    pub fn new(schema_type: &str, path: &str) -> Self {
        SchemaNode {
            schema_type: schema_type.to_string(),
            path: path.to_string(),
            children: BTreeMap::new(),
            items: None,
            required: Vec::new(),
        }
    }
}

/// Infer a schema from a serde_json::Value
pub fn infer_schema(value: &Value, path: &str) -> SchemaNode {
    match value {
        Value::Null => SchemaNode::new("null", path),
        Value::Bool(_) => SchemaNode::new("boolean", path),
        Value::Number(n) => {
            if n.is_i64() {
                SchemaNode::new("integer", path)
            } else {
                SchemaNode::new("number", path)
            }
        }
        Value::String(_) => SchemaNode::new("string", path),
        Value::Array(arr) => {
            let mut node = SchemaNode::new("array", path);
            if !arr.is_empty() {
                // Merge schemas from all array items
                let mut merged = infer_schema(&arr[0], &format!("{}[]", path));
                for item in &arr[1..] {
                    let item_schema = infer_schema(item, &format!("{}[]", path));
                    merged = merge_schemas(&merged, &item_schema);
                }
                node.items = Some(Box::new(merged));
            }
            node
        }
        Value::Object(obj) => {
            let mut node = SchemaNode::new("object", path);
            for (key, child_val) in obj {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };
                let child_schema = infer_schema(child_val, &child_path);
                node.children.insert(key.clone(), child_schema);
                node.required.push(key.clone());
            }
            node
        }
    }
}

/// Merge two schemas into one (used for array items with heterogeneous types)
fn merge_schemas(a: &SchemaNode, b: &SchemaNode) -> SchemaNode {
    if a.schema_type == b.schema_type {
        let mut merged = SchemaNode::new(&a.schema_type, &a.path);

        match a.schema_type.as_str() {
            "object" => {
                // Merge children; required = intersection of required
                for (key, child_a) in &a.children {
                    if let Some(child_b) = b.children.get(key) {
                        merged
                            .children
                            .insert(key.clone(), merge_schemas(child_a, child_b));
                        merged.required.push(key.clone());
                    }
                }
            }
            "array" => {
                if let (Some(items_a), Some(items_b)) = (&a.items, &b.items) {
                    merged.items = Some(Box::new(merge_schemas(items_a, items_b)));
                } else if let Some(items_a) = &a.items {
                    merged.items = Some(Box::new((**items_a).clone()));
                } else if let Some(items_b) = &b.items {
                    merged.items = Some(Box::new((**items_b).clone()));
                }
            }
            _ => {}
        }

        merged
    } else {
        // Different types — use a union type (represented as "anyOf" in JSON Schema)
        // For simplicity, we'll just use the first type but note the conflict
        SchemaNode::new(&a.schema_type, &a.path)
    }
}

/// Render a SchemaNode as a JSON Schema (Draft 2020-12) Value
pub fn to_json_schema(node: &SchemaNode) -> Value {
    let mut schema = Map::new();
    schema.insert(
        "$schema".to_string(),
        Value::String("https://json-schema.org/draft/2020-12/schema".to_string()),
    );

    let type_val = node_to_schema_value(node);
    for (k, v) in type_val.as_object().unwrap() {
        schema.insert(k.clone(), v.clone());
    }

    Value::Object(schema)
}

/// Convert a SchemaNode to a JSON Schema fragment (without $schema)
fn node_to_schema_value(node: &SchemaNode) -> Value {
    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String(node.schema_type.clone()));

    match node.schema_type.as_str() {
        "object" => {
            let mut properties = Map::new();
            for (key, child) in &node.children {
                properties.insert(key.clone(), node_to_schema_value(child));
            }
            schema.insert("properties".to_string(), Value::Object(properties));

            if !node.required.is_empty() {
                let req: Vec<Value> = node
                    .required
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect();
                schema.insert("required".to_string(), Value::Array(req));
            }
        }
        "array" => {
            if let Some(items) = &node.items {
                schema.insert("items".to_string(), node_to_schema_value(items));
            }
        }
        _ => {}
    }

    Value::Object(schema)
}

/// Render a SchemaNode as human-readable text (tree view)
pub fn render_text(node: &SchemaNode, indent: usize) -> String {
    let prefix = "  ".repeat(indent);
    let mut output = String::new();

    match node.schema_type.as_str() {
        "object" => {
            output.push_str(&format!("{}{} (object)\n", prefix, node.path));
            for child in node.children.values() {
                output.push_str(&render_text(child, indent + 1));
            }
        }
        "array" => {
            if let Some(items) = &node.items {
                output.push_str(&format!(
                    "{}{} (array of {})\n",
                    prefix, node.path, items.schema_type
                ));
                if items.schema_type == "object" {
                    output.push_str(&render_text(items, indent + 1));
                }
            } else {
                output.push_str(&format!("{}{} (array, empty)\n", prefix, node.path));
            }
        }
        _ => {
            output.push_str(&format!("{}{} ({})\n", prefix, node.path, node.schema_type));
        }
    }

    output
}

/// Render a SchemaNode as JSON text output (simplified tree, not full JSON Schema)
pub fn render_json_tree(node: &SchemaNode) -> Value {
    let mut obj = Map::new();
    obj.insert("type".to_string(), Value::String(node.schema_type.clone()));
    obj.insert("path".to_string(), Value::String(node.path.clone()));

    match node.schema_type.as_str() {
        "object" => {
            let mut children = Map::new();
            for (key, child) in &node.children {
                children.insert(key.clone(), render_json_tree(child));
            }
            obj.insert("children".to_string(), Value::Object(children));
            let req: Vec<Value> = node
                .required
                .iter()
                .map(|s| Value::String(s.clone()))
                .collect();
            obj.insert("required".to_string(), Value::Array(req));
        }
        "array" => {
            if let Some(items) = &node.items {
                obj.insert("items".to_string(), render_json_tree(items));
            }
        }
        _ => {}
    }

    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_infer_null() {
        let node = infer_schema(&Value::Null, "$");
        assert_eq!(node.schema_type, "null");
    }

    #[test]
    fn test_infer_bool() {
        let node = infer_schema(&Value::Bool(true), "$");
        assert_eq!(node.schema_type, "boolean");
    }

    #[test]
    fn test_infer_integer() {
        let node = infer_schema(&json!(42), "$");
        assert_eq!(node.schema_type, "integer");
    }

    #[test]
    fn test_infer_float() {
        let node = infer_schema(&json!(2.5), "$");
        assert_eq!(node.schema_type, "number");
    }

    #[test]
    fn test_infer_string() {
        let node = infer_schema(&json!("hello"), "$");
        assert_eq!(node.schema_type, "string");
    }

    #[test]
    fn test_infer_array_of_integers() {
        let val = json!([1, 2, 3]);
        let node = infer_schema(&val, "$");
        assert_eq!(node.schema_type, "array");
        assert!(node.items.is_some());
        assert_eq!(node.items.as_ref().unwrap().schema_type, "integer");
    }

    #[test]
    fn test_infer_empty_array() {
        let val = json!([]);
        let node = infer_schema(&val, "$");
        assert_eq!(node.schema_type, "array");
        assert!(node.items.is_none());
    }

    #[test]
    fn test_infer_simple_object() {
        let val = json!({"name": "Alice", "age": 30, "active": true});
        let node = infer_schema(&val, "$");
        assert_eq!(node.schema_type, "object");
        assert_eq!(node.children.len(), 3);
        assert_eq!(node.children["name"].schema_type, "string");
        assert_eq!(node.children["age"].schema_type, "integer");
        assert_eq!(node.children["active"].schema_type, "boolean");
        assert_eq!(node.required.len(), 3);
    }

    #[test]
    fn test_infer_nested_object() {
        let val = json!({
            "config": {
                "host": "localhost",
                "port": 8080
            },
            "name": "test"
        });
        let node = infer_schema(&val, "$");
        assert_eq!(node.schema_type, "object");
        assert_eq!(node.children.len(), 2);
        assert_eq!(node.children["config"].schema_type, "object");
        assert_eq!(
            node.children["config"].children["host"].schema_type,
            "string"
        );
        assert_eq!(
            node.children["config"].children["port"].schema_type,
            "integer"
        );
    }

    #[test]
    fn test_infer_array_of_objects() {
        let val = json!([
            {"name": "Alice", "age": 30},
            {"name": "Bob", "age": 25}
        ]);
        let node = infer_schema(&val, "$");
        assert_eq!(node.schema_type, "array");
        let items = node.items.as_ref().unwrap();
        assert_eq!(items.schema_type, "object");
        assert_eq!(items.children["name"].schema_type, "string");
        assert_eq!(items.children["age"].schema_type, "integer");
    }

    #[test]
    fn test_to_json_schema_object() {
        let val = json!({"name": "test", "count": 42});
        let node = infer_schema(&val, "$");
        let schema = to_json_schema(&node);
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
        assert_eq!(schema["properties"]["name"]["type"], "string");
        assert_eq!(schema["properties"]["count"]["type"], "integer");
        assert!(schema["required"].is_array());
    }

    #[test]
    fn test_to_json_schema_array() {
        let val = json!([1, 2, 3]);
        let node = infer_schema(&val, "$");
        let schema = to_json_schema(&node);
        assert_eq!(schema["type"], "array");
        assert_eq!(schema["items"]["type"], "integer");
    }

    #[test]
    fn test_render_text_object() {
        let val = json!({"name": "test", "count": 42});
        let node = infer_schema(&val, "$");
        let text = render_text(&node, 0);
        assert!(text.contains("object"));
        assert!(text.contains("name"));
        assert!(text.contains("string"));
        assert!(text.contains("integer"));
    }

    #[test]
    fn test_render_text_array() {
        let val = json!([1, 2, 3]);
        let node = infer_schema(&val, "$");
        let text = render_text(&node, 0);
        assert!(text.contains("array"));
        assert!(text.contains("integer"));
    }

    #[test]
    fn test_render_json_tree() {
        let val = json!({"name": "test"});
        let node = infer_schema(&val, "$");
        let tree = render_json_tree(&node);
        assert_eq!(tree["type"], "object");
        assert_eq!(tree["children"]["name"]["type"], "string");
    }

    #[test]
    fn test_infer_csv_data() {
        // Simulate CSV parsed as array of objects
        let val = json!([
            {"name": "Alice", "age": 30},
            {"name": "Bob", "age": 25}
        ]);
        let node = infer_schema(&val, "$");
        assert_eq!(node.schema_type, "array");
        let items = node.items.as_ref().unwrap();
        assert_eq!(items.schema_type, "object");
        assert_eq!(items.children.len(), 2);
    }

    #[test]
    fn test_infer_deeply_nested() {
        let val = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "value": 42
                    }
                }
            }
        });
        let node = infer_schema(&val, "$");
        assert_eq!(
            node.children["level1"].children["level2"].children["level3"].children["value"]
                .schema_type,
            "integer"
        );
    }
}

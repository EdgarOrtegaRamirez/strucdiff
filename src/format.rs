use serde_json::Value;
use std::fmt;

/// Supported data formats
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Json,
    Yaml,
    Toml,
    Csv,
}

impl Format {
    /// Detect format from file extension
    pub fn from_extension(path: &str) -> Option<Format> {
        let ext = path.rsplit('.').next()?.to_lowercase();
        match ext.as_str() {
            "json" => Some(Format::Json),
            "yaml" | "yml" => Some(Format::Yaml),
            "toml" => Some(Format::Toml),
            "csv" => Some(Format::Csv),
            _ => None,
        }
    }

    pub fn extensions() -> &'static [&'static str] {
        &["json", "yaml", "yml", "toml", "csv"]
    }

    pub fn name(&self) -> &str {
        match self {
            Format::Json => "JSON",
            Format::Yaml => "YAML",
            Format::Toml => "TOML",
            Format::Csv => "CSV",
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Parse the content of a file into a serde_json::Value
pub fn parse_content(content: &str, format: Format) -> Result<Value, ParseError> {
    match format {
        Format::Json => parse_json(content),
        Format::Yaml => parse_yaml(content),
        Format::Toml => parse_toml(content),
        Format::Csv => parse_csv(content),
    }
}

/// Errors that can occur during parsing
#[derive(Debug)]
pub enum ParseError {
    Json(String),
    Yaml(String),
    Toml(String),
    Csv(String),
    #[allow(dead_code)]
    UnsupportedFormat(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Json(msg) => write!(f, "JSON parse error: {}", msg),
            ParseError::Yaml(msg) => write!(f, "YAML parse error: {}", msg),
            ParseError::Toml(msg) => write!(f, "TOML parse error: {}", msg),
            ParseError::Csv(msg) => write!(f, "CSV parse error: {}", msg),
            ParseError::UnsupportedFormat(msg) => write!(f, "Unsupported format: {}", msg),
        }
    }
}

fn parse_json(content: &str) -> Result<Value, ParseError> {
    serde_json::from_str(content).map_err(|e| ParseError::Json(e.to_string()))
}

fn parse_yaml(content: &str) -> Result<Value, ParseError> {
    serde_yaml::from_str(content).map_err(|e| ParseError::Yaml(e.to_string()))
}

fn parse_toml(content: &str) -> Result<Value, ParseError> {
    let value: toml::Value =
        toml::from_str(content).map_err(|e| ParseError::Toml(e.to_string()))?;
    toml_to_json(&value)
}

/// Convert toml::Value to serde_json::Value
fn toml_to_json(value: &toml::Value) -> Result<Value, ParseError> {
    match value {
        toml::Value::String(s) => Ok(Value::String(s.clone())),
        toml::Value::Integer(i) => Ok(Value::Number(serde_json::Number::from(*i))),
        toml::Value::Float(f) => {
            let n = serde_json::Number::from_f64(*f)
                .ok_or_else(|| ParseError::Toml(format!("Invalid float: {}", f)))?;
            Ok(Value::Number(n))
        }
        toml::Value::Boolean(b) => Ok(Value::Bool(*b)),
        toml::Value::Datetime(dt) => Ok(Value::String(dt.to_string())),
        toml::Value::Array(arr) => {
            let items: Result<Vec<Value>, ParseError> = arr.iter().map(toml_to_json).collect();
            Ok(Value::Array(items?))
        }
        toml::Value::Table(table) => {
            let mut map = serde_json::Map::new();
            for (k, v) in table.iter() {
                map.insert(k.clone(), toml_to_json(v)?);
            }
            Ok(Value::Object(map))
        }
    }
}

fn parse_csv(content: &str) -> Result<Value, ParseError> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(content.as_bytes());

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| ParseError::Csv(e.to_string()))?
        .iter()
        .map(|h| h.to_string())
        .collect();

    let mut rows = Vec::new();

    for result in reader.records() {
        let record = result.map_err(|e| ParseError::Csv(e.to_string()))?;
        let mut row = serde_json::Map::new();

        for (i, field) in record.iter().enumerate() {
            let key = if i < headers.len() {
                headers[i].clone()
            } else {
                format!("column_{}", i)
            };

            // Try to parse numbers
            let value = if let Ok(n) = field.parse::<i64>() {
                Value::Number(serde_json::Number::from(n))
            } else if let Ok(f) = field.parse::<f64>() {
                if let Some(n) = serde_json::Number::from_f64(f) {
                    Value::Number(n)
                } else {
                    Value::String(field.to_string())
                }
            } else if field.is_empty() {
                Value::Null
            } else {
                Value::String(field.to_string())
            };

            row.insert(key, value);
        }

        rows.push(Value::Object(row));
    }

    // If no headers detected, try positional array
    if headers.is_empty() && !rows.is_empty() {
        // Return as array of arrays
        return parse_csv_as_arrays(content);
    }

    Ok(Value::Array(rows))
}

fn parse_csv_as_arrays(content: &str) -> Result<Value, ParseError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(content.as_bytes());

    let mut rows = Vec::new();

    for result in reader.records() {
        let record = result.map_err(|e| ParseError::Csv(e.to_string()))?;
        let fields: Vec<Value> = record
            .iter()
            .map(|f| {
                if let Ok(n) = f.parse::<i64>() {
                    Value::Number(serde_json::Number::from(n))
                } else if let Ok(fl) = f.parse::<f64>() {
                    if let Some(n) = serde_json::Number::from_f64(fl) {
                        Value::Number(n)
                    } else {
                        Value::String(f.to_string())
                    }
                } else if f.is_empty() {
                    Value::Null
                } else {
                    Value::String(f.to_string())
                }
            })
            .collect();
        rows.push(Value::Array(fields));
    }

    Ok(Value::Array(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_json() {
        assert_eq!(Format::from_extension("data.json"), Some(Format::Json));
        assert_eq!(
            Format::from_extension("path/to/file.JSON"),
            Some(Format::Json)
        );
    }

    #[test]
    fn test_detect_yaml() {
        assert_eq!(Format::from_extension("config.yaml"), Some(Format::Yaml));
        assert_eq!(Format::from_extension("config.yml"), Some(Format::Yaml));
    }

    #[test]
    fn test_detect_toml() {
        assert_eq!(Format::from_extension("Cargo.toml"), Some(Format::Toml));
    }

    #[test]
    fn test_detect_csv() {
        assert_eq!(Format::from_extension("data.csv"), Some(Format::Csv));
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(Format::from_extension("file.txt"), None);
    }

    #[test]
    fn test_parse_json_valid() {
        let content = r#"{"name": "test", "value": 42}"#;
        let result = parse_content(content, Format::Json).unwrap();
        assert_eq!(result["name"], "test");
        assert_eq!(result["value"], 42);
    }

    #[test]
    fn test_parse_json_invalid() {
        let content = r#"{invalid json}"#;
        let result = parse_content(content, Format::Json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_yaml_valid() {
        let content = "name: test\nvalue: 42\n";
        let result = parse_content(content, Format::Yaml).unwrap();
        assert_eq!(result["name"], "test");
        assert_eq!(result["value"], 42);
    }

    #[test]
    fn test_parse_toml_valid() {
        let content = r#"
name = "test"
value = 42
[section]
key = true
"#;
        let result = parse_content(content, Format::Toml).unwrap();
        assert_eq!(result["name"], "test");
        assert_eq!(result["value"], 42);
        assert_eq!(result["section"]["key"], true);
    }

    #[test]
    fn test_parse_csv_valid() {
        let content = "name,age,city\nAlice,30,NYC\nBob,25,SF\n";
        let result = parse_content(content, Format::Csv).unwrap();
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "Alice");
        assert_eq!(arr[0]["age"], 30);
    }

    #[test]
    fn test_parse_csv_without_headers() {
        let content = "Alice,30,NYC\nBob,25,SF\n";
        let result = parse_content(content, Format::Csv);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_csv_empty() {
        let content = "";
        let result = parse_content(content, Format::Csv).unwrap();
        assert!(result.is_array());
        assert!(result.as_array().unwrap().is_empty());
    }

    #[test]
    fn test_format_extensions() {
        let exts = Format::extensions();
        assert!(exts.contains(&"json"));
        assert!(exts.contains(&"yaml"));
        assert!(exts.contains(&"toml"));
        assert!(exts.contains(&"csv"));
    }

    #[test]
    fn test_format_names() {
        assert_eq!(Format::Json.name(), "JSON");
        assert_eq!(Format::Yaml.name(), "YAML");
        assert_eq!(Format::Toml.name(), "TOML");
        assert_eq!(Format::Csv.name(), "CSV");
    }
}

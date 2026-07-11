use crate::diff::{DiffKind, DiffResult};
use colored::{ColoredString, Colorize};
use std::io::{self, Write};

/// Output format for diff results
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    /// Human-readable colorized terminal output
    Text,
    /// JSON output for CI/automation
    Json,
}

/// Render diff results to the given writer
pub fn render_diff(
    result: &DiffResult,
    format: OutputFormat,
    file1: &str,
    file2: &str,
    writer: &mut impl Write,
) -> io::Result<()> {
    match format {
        OutputFormat::Text => render_text(result, file1, file2, writer),
        OutputFormat::Json => render_json(result, file1, file2, writer),
    }
}

fn render_text(
    result: &DiffResult,
    file1: &str,
    file2: &str,
    writer: &mut impl Write,
) -> io::Result<()> {
    if result.identical {
        writeln!(writer, "{}", "✓ Files are identical".green().bold())?;
        return Ok(());
    }

    writeln!(writer, "{}", "✗ Files differ".red().bold())?;
    writeln!(
        writer,
        "  {}  vs  {}",
        file1.white().bold(),
        file2.white().bold()
    )?;
    writeln!(
        writer,
        "  {} change(s) found",
        result.entries.len().to_string().yellow().bold()
    )?;
    writeln!(writer)?;

    // Sort entries by path for consistent outputs
    let mut sorted = result.entries.clone();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    for entry in &sorted {
        let (symbol, kind_label, color_fn): (&str, &str, fn(&str) -> ColoredString) =
            match entry.kind {
                DiffKind::Added => ("+", "added", |s| s.green()),
                DiffKind::Removed => ("-", "removed", |s| s.red()),
                DiffKind::Changed => ("~", "changed", |s| s.yellow()),
                DiffKind::Renamed => ("→", "renamed", |s| s.cyan()),
            };

        // For renamed entries, show old_key → new_key
        if entry.kind == DiffKind::Renamed {
            let old_key = entry.old_key.as_deref().unwrap_or("");
            let new_key = entry.new_key.as_deref().unwrap_or("");
            writeln!(
                writer,
                "  {} {} {} {} {}",
                color_fn(symbol),
                color_fn(old_key),
                "→".cyan(),
                color_fn(new_key),
                color_fn(kind_label)
            )?;
        } else {
            writeln!(
                writer,
                "  {} {} {}",
                color_fn(symbol),
                entry.path.white().bold(),
                color_fn(kind_label)
            )?;
        }

        match entry.kind {
            DiffKind::Added => {
                if let Some(ref new) = entry.new_value {
                    writeln!(writer, "      {} {}", "+".green(), new.green())?;
                }
            }
            DiffKind::Removed => {
                if let Some(ref old) = entry.old_value {
                    writeln!(writer, "      {} {}", "-".red(), old.red())?;
                }
            }
            DiffKind::Changed => {
                if let Some(ref old) = entry.old_value {
                    writeln!(writer, "      {} {}", "-".red(), old.red())?;
                }
                if let Some(ref new) = entry.new_value {
                    writeln!(writer, "      {} {}", "+".green(), new.green())?;
                }
            }
            DiffKind::Renamed => {
                if let Some(ref old) = entry.old_value {
                    writeln!(writer, "      {} {}", "-".red(), old.red())?;
                }
                if let Some(ref new) = entry.new_value {
                    writeln!(writer, "      {} {}", "+".green(), new.green())?;
                }
            }
        }
    }

    Ok(())
}

fn render_json(
    result: &DiffResult,
    file1: &str,
    file2: &str,
    writer: &mut impl Write,
) -> io::Result<()> {
    let mut entries_json = Vec::new();

    for entry in &result.entries {
        let kind_str = match entry.kind {
            DiffKind::Added => "added",
            DiffKind::Removed => "removed",
            DiffKind::Changed => "changed",
            DiffKind::Renamed => "renamed",
        };

        let mut obj = serde_json::Map::new();
        obj.insert(
            "path".to_string(),
            serde_json::Value::String(entry.path.clone()),
        );
        obj.insert(
            "kind".to_string(),
            serde_json::Value::String(kind_str.to_string()),
        );
        if let Some(ref old) = entry.old_value {
            obj.insert(
                "old_value".to_string(),
                serde_json::Value::String(old.clone()),
            );
        }
        if let Some(ref new) = entry.new_value {
            obj.insert(
                "new_value".to_string(),
                serde_json::Value::String(new.clone()),
            );
        }
        if let Some(ref old_key) = entry.old_key {
            obj.insert(
                "old_key".to_string(),
                serde_json::Value::String(old_key.clone()),
            );
        }
        if let Some(ref new_key) = entry.new_key {
            obj.insert(
                "new_key".to_string(),
                serde_json::Value::String(new_key.clone()),
            );
        }
        entries_json.push(serde_json::Value::Object(obj));
    }

    let output = serde_json::json!({
        "identical": result.identical,
        "file1": file1,
        "file2": file2,
        "changes": result.entries.len(),
        "entries": entries_json,
    });

    writeln!(writer, "{}", serde_json::to_string_pretty(&output).unwrap())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{DiffEntry, DiffKind, DiffResult};

    fn make_result() -> DiffResult {
        DiffResult {
            identical: false,
            entries: vec![
                DiffEntry {
                    path: "config.host".to_string(),
                    kind: DiffKind::Changed,
                    old_value: Some("localhost".to_string()),
                    new_value: Some("prod.example.com".to_string()),
                    old_key: None,
                    new_key: None,
                },
                DiffEntry {
                    path: "config.port".to_string(),
                    kind: DiffKind::Changed,
                    old_value: Some("8080".to_string()),
                    new_value: Some("443".to_string()),
                    old_key: None,
                    new_key: None,
                },
                DiffEntry {
                    path: "features.new_feature".to_string(),
                    kind: DiffKind::Added,
                    old_value: None,
                    new_value: Some("true".to_string()),
                    old_key: None,
                    new_key: None,
                },
                DiffEntry {
                    path: "old_config".to_string(),
                    kind: DiffKind::Removed,
                    old_value: Some("deprecated".to_string()),
                    new_value: None,
                    old_key: None,
                    new_key: None,
                },
            ],
        }
    }

    fn make_rename_result() -> DiffResult {
        DiffResult {
            identical: false,
            entries: vec![DiffEntry {
                path: "username".to_string(),
                kind: DiffKind::Renamed,
                old_value: Some("\"alice\"".to_string()),
                new_value: Some("\"alice\"".to_string()),
                old_key: Some("userName".to_string()),
                new_key: Some("username".to_string()),
            }],
        }
    }

    #[test]
    fn test_render_text_identical() {
        let result = DiffResult {
            identical: true,
            entries: vec![],
        };
        let mut buf = Vec::new();
        render_text(&result, "a.json", "b.json", &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("identical"));
    }

    #[test]
    fn test_render_text_different() {
        let result = make_result();
        let mut buf = Vec::new();
        render_text(&result, "a.json", "b.json", &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("differ"));
        assert!(output.contains("config.host"));
        assert!(output.contains("config.port"));
        assert!(output.contains("new_feature"));
        assert!(output.contains("old_config"));
        assert!(output.contains("4 change(s)"));
    }

    #[test]
    fn test_render_text_renamed() {
        let result = make_rename_result();
        let mut buf = Vec::new();
        render_text(&result, "a.json", "b.json", &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("renamed"));
        assert!(output.contains("userName"));
        assert!(output.contains("username"));
    }

    #[test]
    fn test_render_json_identical() {
        let result = DiffResult {
            identical: true,
            entries: vec![],
        };
        let mut buf = Vec::new();
        render_json(&result, "a.json", "b.json", &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["identical"], true);
        assert_eq!(parsed["changes"], 0);
    }

    #[test]
    fn test_render_json_different() {
        let result = make_result();
        let mut buf = Vec::new();
        render_json(&result, "a.json", "b.json", &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["identical"], false);
        assert_eq!(parsed["changes"], 4);
        assert_eq!(parsed["entries"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn test_render_json_renamed() {
        let result = make_rename_result();
        let mut buf = Vec::new();
        render_json(&result, "a.json", "b.json", &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let entry = &parsed["entries"][0];
        assert_eq!(entry["kind"], "renamed");
        assert_eq!(entry["old_key"], "userName");
        assert_eq!(entry["new_key"], "username");
    }
}

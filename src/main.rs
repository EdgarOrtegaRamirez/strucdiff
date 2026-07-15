mod diff;
mod format;
mod output;
mod schema;

use clap::{Parser, Subcommand};
use colored::Colorize;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use walkdir::WalkDir;

use crate::diff::{csv_diff_keyed, diff_values_with_config, DiffConfig};
use crate::format::{parse_content, Format};
use crate::output::{render_diff, OutputFormat};
use crate::schema::{infer_schema, render_json_tree, render_text, to_json_schema};

#[derive(Parser)]
#[command(
    name = "strucdiff",
    about = "Semantic structured data diff tool — compare JSON, YAML, TOML, CSV structurally",
    version,
    long_version = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compare two structured data files
    Diff {
        /// First file (old)
        file1: String,
        /// Second file (new)
        file2: String,

        /// Output format: text (default), json, or summary
        #[arg(long, short = 'f', default_value = "text", value_parser = ["text", "json", "summary"])]
        format: String,

        /// Paths to ignore (dot-separated, can be specified multiple times)
        #[arg(long, short = 'i')]
        ignore: Vec<String>,

        /// Input format (auto-detect by default)
        #[arg(long, short = 't', value_parser = ["json", "yaml", "toml", "csv"])]
        r#type: Option<String>,

        /// Disable rename detection (by default, renamed keys are detected)
        #[arg(long = "no-rename")]
        no_rename: bool,

        /// Key column(s) for CSV key-based row matching (can specify multiple)
        #[arg(long, short = 'k')]
        key: Vec<String>,

        /// Include unchanged rows in CSV diff output
        #[arg(long = "include-unchanged")]
        include_unchanged: bool,
    },
    /// Compare all supported files in two directories
    Dir {
        /// Old directory
        dir1: String,
        /// New directory
        dir2: String,
        /// Output format: text (default) or json
        #[arg(long, short = 'f', default_value = "text", value_parser = ["text", "json"])]
        format: String,
    },
    /// Infer and display the schema of a structured data file
    Schema {
        /// Input file (use - for stdin)
        file: String,

        /// Output format: text (default), json (tree view), or schema (JSON Schema Draft 2020-12)
        #[arg(long, short = 'f', default_value = "text", value_parser = ["text", "json", "schema"])]
        format: String,

        /// Input format (auto-detect by default)
        #[arg(long, short = 't', value_parser = ["json", "yaml", "toml", "csv"])]
        r#type: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Diff {
            file1,
            file2,
            format,
            ignore,
            r#type,
            no_rename,
            key,
            include_unchanged,
        } => handle_diff(
            file1,
            file2,
            format,
            ignore,
            r#type,
            *no_rename,
            key,
            *include_unchanged,
        ),
        Commands::Dir { dir1, dir2, format } => handle_dir(dir1, dir2, format),
        Commands::Schema {
            file,
            format,
            r#type,
        } => handle_schema(file, format, r#type),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_diff(
    file1: &str,
    file2: &str,
    format_str: &str,
    ignore_paths: &[String],
    force_type: &Option<String>,
    no_rename: bool,
    key: &[String],
    include_unchanged: bool,
) -> Result<(), String> {
    let output_format = match format_str {
        "json" => OutputFormat::Json,
        "summary" => OutputFormat::Summary,
        _ => OutputFormat::Text,
    };

    let fmt = if let Some(ft) = force_type {
        match ft.as_str() {
            "json" => Format::Json,
            "yaml" => Format::Yaml,
            "toml" => Format::Toml,
            "csv" => Format::Csv,
            _ => {
                return Err(format!(
                    "Unknown format: {}. Supported: json, yaml, toml, csv",
                    ft
                ));
            }
        }
    } else {
        let ext_fmt = Format::from_extension(file1)
            .or_else(|| Format::from_extension(file2))
            .ok_or_else(|| {
                format!(
                    "Cannot detect format from file extensions. Supported formats: {}",
                    Format::extensions().join(", ")
                )
            })?;
        // Verify both files have compatible extensions
        if let Some(f1) = Format::from_extension(file1) {
            if let Some(f2) = Format::from_extension(file2) {
                if f1 != f2 {
                    return Err(format!(
                        "File format mismatch: {} is {} but {} is {}",
                        file1, f1, file2, f2
                    ));
                }
            }
        }
        ext_fmt
    };

    let content1 = read_file(file1)?;
    let content2 = read_file(file2)?;

    let val1 = parse_content(&content1, fmt).map_err(|e| e.to_string())?;
    let val2 = parse_content(&content2, fmt).map_err(|e| e.to_string())?;

    let config = DiffConfig {
        rename_detection: !no_rename,
        rename_threshold: 0.6,
    };

    // Use CSV key-based diff when format is CSV and key columns are specified
    let mut result = if fmt == Format::Csv && !key.is_empty() {
        csv_diff_keyed(&val1, &val2, key, include_unchanged)
    } else {
        diff_values_with_config(&val1, &val2, "", &config)
    };

    // Apply ignore filters
    if !ignore_paths.is_empty() {
        result.entries.retain(|e| {
            !ignore_paths
                .iter()
                .any(|ignored| e.path == *ignored || e.path.starts_with(&format!("{}.", ignored)))
        });
        result.identical = result.entries.is_empty();
    }

    let mut stdout = io::stdout();
    render_diff(&result, output_format, file1, file2, &mut stdout)
        .map_err(|e| format!("Output error: {}", e))?;

    // Exit code: 0 if identical, 1 if different
    if !result.identical {
        std::process::exit(1);
    }

    Ok(())
}

fn handle_dir(dir1: &str, dir2: &str, format_str: &str) -> Result<(), String> {
    let _output_format = match format_str {
        "json" => OutputFormat::Json,
        _ => OutputFormat::Text,
    };

    let dir1_path = Path::new(dir1);
    let dir2_path = Path::new(dir2);

    if !dir1_path.is_dir() {
        return Err(format!("Not a directory: {}", dir1));
    }
    if !dir2_path.is_dir() {
        return Err(format!("Not a directory: {}", dir2));
    }

    let mut results: Vec<(String, bool, usize)> = Vec::new();
    let mut had_diffs = false;

    // Walk dir1, find matching files in dir2
    let walker = WalkDir::new(dir1_path)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| {
            !e.file_name()
                .to_str()
                .map(|s| s.starts_with('.'))
                .unwrap_or(false)
        });

    for entry in walker {
        let entry = entry.map_err(|e| format!("Walk error: {}", e))?;

        if !entry.file_type().is_file() {
            continue;
        }

        let rel_path = entry
            .path()
            .strip_prefix(dir1_path)
            .map_err(|e| format!("Path error: {}", e))?;

        // Check if the file has a supported extension
        let rel_str = rel_path.to_str().ok_or("Non-UTF-8 path")?;
        if Format::from_extension(rel_str).is_none() {
            continue;
        }

        let file2_full = dir2_path.join(rel_path);

        if !file2_full.exists() {
            writeln!(
                io::stdout(),
                "  {} {} (no matching file in {})",
                "~".yellow(),
                rel_str,
                dir2
            )
            .map_err(|e| format!("Output error: {}", e))?;
            had_diffs = true;
            results.push((rel_str.to_string(), false, 0));
            continue;
        }

        if !file2_full.is_file() {
            writeln!(
                io::stdout(),
                "  {} {} (not a file in {})",
                "~".yellow(),
                rel_str,
                dir2
            )
            .map_err(|e| format!("Output error: {}", e))?;
            had_diffs = true;
            results.push((rel_str.to_string(), false, 1));
            continue;
        }

        // Run diff on the pair
        let content1 = read_file(entry.path().to_str().unwrap())?;
        let content2 = read_file(file2_full.to_str().unwrap())?;

        let fmt = Format::from_extension(rel_str).unwrap();
        let val1 = parse_content(&content1, fmt).map_err(|e| e.to_string())?;
        let val2 = parse_content(&content2, fmt).map_err(|e| e.to_string())?;

        let config = DiffConfig::default();
        let result = diff_values_with_config(&val1, &val2, "", &config);

        let count = result.entries.len();
        if !result.identical {
            writeln!(
                io::stdout(),
                "  {} {} ({} change(s))",
                "✗".red(),
                rel_str,
                count
            )
            .map_err(|e| format!("Output error: {}", e))?;
            had_diffs = true;
            results.push((rel_str.to_string(), false, count));
        } else {
            writeln!(io::stdout(), "  {} {}", "✓".green(), rel_str)
                .map_err(|e| format!("Output error: {}", e))?;
            results.push((rel_str.to_string(), true, 0));
        }
    }

    // Check for files in dir2 not in dir1
    let walker2 = WalkDir::new(dir2_path)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| {
            !e.file_name()
                .to_str()
                .map(|s| s.starts_with('.'))
                .unwrap_or(false)
        });

    for entry in walker2 {
        let entry = entry.map_err(|e| format!("Walk error: {}", e))?;

        if !entry.file_type().is_file() {
            continue;
        }

        let rel_path = entry
            .path()
            .strip_prefix(dir2_path)
            .map_err(|e| format!("Path error: {}", e))?;

        let rel_str = rel_path.to_str().ok_or("Non-UTF-8 path")?;
        if Format::from_extension(rel_str).is_none() {
            continue;
        }

        let file1_full = dir1_path.join(rel_path);
        if !file1_full.exists() {
            writeln!(
                io::stdout(),
                "  {} {} (new in {})",
                "+".green(),
                rel_str,
                dir2
            )
            .map_err(|e| format!("Output error: {}", e))?;
            had_diffs = true;
            results.push((rel_str.to_string(), false, 1));
        }
    }

    if !had_diffs {
        writeln!(
            io::stdout(),
            "{}",
            "✓ Directories are identical".green().bold()
        )
        .map_err(|e| format!("Output error: {}", e))?;
    }

    if had_diffs {
        std::process::exit(1);
    }

    Ok(())
}

fn handle_schema(file: &str, format_str: &str, force_type: &Option<String>) -> Result<(), String> {
    let fmt = if let Some(ft) = force_type {
        match ft.as_str() {
            "json" => Format::Json,
            "yaml" => Format::Yaml,
            "toml" => Format::Toml,
            "csv" => Format::Csv,
            _ => {
                return Err(format!(
                    "Unknown format: {}. Supported: json, yaml, toml, csv",
                    ft
                ));
            }
        }
    } else if file == "-" {
        // Default to JSON for stdin
        Format::Json
    } else {
        Format::from_extension(file).ok_or_else(|| {
            format!(
                "Cannot detect format from file extension. Supported formats: {}",
                Format::extensions().join(", ")
            )
        })?
    };

    let content = read_file(file)?;
    let val = parse_content(&content, fmt).map_err(|e| e.to_string())?;

    let node = infer_schema(&val, "$");

    let mut stdout = io::stdout();
    match format_str {
        "json" => {
            let tree = render_json_tree(&node);
            writeln!(stdout, "{}", serde_json::to_string_pretty(&tree).unwrap())
                .map_err(|e| format!("Output error: {}", e))?;
        }
        "schema" => {
            let schema = to_json_schema(&node);
            writeln!(stdout, "{}", serde_json::to_string_pretty(&schema).unwrap())
                .map_err(|e| format!("Output error: {}", e))?;
        }
        _ => {
            // Text format
            let text = render_text(&node, 0);
            write!(stdout, "{}", text).map_err(|e| format!("Output error: {}", e))?;
        }
    }

    Ok(())
}

fn read_file(path: &str) -> Result<String, String> {
    let path = path.strip_prefix("file://").unwrap_or(path);

    if path == "-" {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("Failed to read stdin: {}", e))?;
        return Ok(buf);
    }

    fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_file_nonexistent() {
        let result = read_file("/nonexistent/file.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_format_by_extension() {
        assert_eq!(Format::from_extension("config.yaml"), Some(Format::Yaml));
        assert_eq!(Format::from_extension("data.json"), Some(Format::Json));
        assert_eq!(Format::from_extension("file.txt"), None);
    }

    #[test]
    fn test_file_strip_protocol() {
        // Test that file:// protocol is stripped
        let result = read_file("file:///nonexistent/path.json");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("/nonexistent/path.json"));
    }
}

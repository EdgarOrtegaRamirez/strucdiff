# StrucDiff — Semantic Structured Data Diff CLI

**StrucDiff** is a CLI tool for semantically comparing structured data files. Unlike traditional line-by-line `diff`, StrucDiff understands JSON, YAML, TOML, and CSV structure — showing you exactly what changed at the path level.

## Features

- **Semantic diffs** — Compares at the structural level, not line-by-line
- **Multi-format support** — JSON, YAML, TOML, and CSV
- **Path-based output** — Shows added/removed/changed values with their dot-notation paths
- **Colorized terminal output** — Green for additions, red for removals, yellow for changes
- **JSON output mode** — Machine-readable output for CI/CD pipelines
- **Ignore paths** — Filter out specific fields from comparison
- **Format auto-detection** — Detects format from file extension
- **Force type** — Override format detection with `--type`
- **Stdin support** — Read data from stdin with `-`
- **Directory comparison** — Recursively compare all supported files in two directories
- **Exit codes** — 0 if identical, 1 if different (ideal for CI)

## Installation

### From source

```bash
# Clone the repository
git clone https://github.com/EdgarOrtegaRamirez/strucdiff.git
cd strucdiff

# Build
cargo build --release

# Install (optional)
cp target/release/strucdiff /usr/local/bin/
```

### Using Cargo

```bash
cargo install --git https://github.com/EdgarOrtegaRamirez/strucdiff
```

## Quick Start

```bash
# Compare two JSON files
strucdiff diff old.json new.json

# Compare YAML files
strucdiff diff config_v1.yaml config_v2.yaml

# Compare TOML files
strucdiff diff Cargo_old.toml Cargo_new.toml

# Compare CSV files
strucdiff diff data_v1.csv data_v2.csv
```

## Usage

### Basic diff

```bash
strucdiff diff <file1> <file2> [options]
```

### Options

| Option | Description |
|--------|-------------|
| `-f, --format <text\|json>` | Output format (default: text) |
| `-t, --type <json\|yaml\|toml\|csv>` | Force input format (bypass auto-detection) |
| `-i, --ignore <PATH>` | Paths to ignore (dot-separated, repeatable) |

### Output formats

**Text output** (default):

```
✗ Files differ
  old.json  vs  new.json
  4 change(s) found

  ~ app.host changed
      - "localhost"
      + "prod.example.com"
  ~ app.port changed
      - 8080
      + 443
  ~ features.metrics changed
      - false
      + true
  ~ version changed
      - "1.0.0"
      + "2.0.0"
```

**JSON output** (for CI):

```bash
strucdiff diff old.json new.json --format json
```

```json
{
  "identical": false,
  "file1": "old.json",
  "file2": "new.json",
  "changes": 4,
  "entries": [
    {
      "path": "app.host",
      "kind": "changed",
      "old_value": "\"localhost\"",
      "new_value": "\"prod.example.com\""
    }
  ]
}
```

### Ignoring paths

```bash
strucdiff diff prod.json dev.json --ignore version --ignore .metadata
```

### Forcing format

```bash
strucdiff diff config.txt settings.txt --type yaml
```

### Stdin

```bash
curl -s https://api.example.com/v1 | strucdiff diff - /tmp/expected.json
```

### Directory comparison

```bash
strucdiff dir ./configs-v1/ ./configs-v2/
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Files are identical |
| 1 | Files differ or an error occurred |

## Supported Formats

| Format | Extension | Status |
|--------|-----------|--------|
| JSON | `.json` | ✅ Full support |
| YAML | `.yaml`, `.yml` | ✅ Full support |
| TOML | `.toml` | ✅ Full support |
| CSV | `.csv` | ✅ Full support (with header auto-detection) |

## Architecture

StrucDiff is built in Rust with these core modules:

- **`diff`** — Core diff engine. Recursively compares `serde_json::Value` trees and produces path-based diffs for objects, arrays, and primitives.
- **`format`** — Format parsers. Each supported format is parsed into a unified `serde_json::Value` representation. TOML values (including datetimes) are converted losslessly.
- **`output`** — Output rendering. Supports both human-readable colorized text and machine-readable JSON.

## Why StrucDiff?

Traditional `diff` shows you line-level changes, which is useless for structured data:

```diff
-  "port": 8080,
+  "port": 443,
```

StrucDiff shows you **what** changed (the value) and **where** (the path):

```
  ~ app.port changed
      - 8080
      + 443
```

## License

MIT — see [LICENSE](LICENSE)
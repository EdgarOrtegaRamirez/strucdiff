# StrucDiff — Semantic Structured Data Diff CLI

**StrucDiff** is a CLI tool for semantically comparing structured data files. Unlike traditional line-by-line `diff`, StrucDiff understands JSON, YAML, TOML, and CSV structure — showing you exactly what changed at the path level.

## Features

- **Semantic diffs** — Compares at the structural level, not line-by-line
- **Rename detection** — Detects renamed keys (e.g., `userName` → `username`) using Levenshtein similarity
- **Schema inference** — Generate JSON Schema (Draft 2020-12) from any structured data file
- **Multi-format support** — JSON, YAML, TOML, and CSV
- **Path-based output** — Shows added/removed/changed/renamed values with their dot-notation paths
- **Colorized terminal output** — Green for additions, red for removals, yellow for changes, cyan for renames
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

# Infer schema from a data file
strucdiff schema config.yaml

# Generate JSON Schema (Draft 2020-12)
strucdiff schema config.yaml -f schema
```

## Usage

### Diff command

```bash
strucdiff diff <file1> <file2> [options]
```

#### Options

| Option | Description |
|--------|-------------|
| `-f, --format <text\|json>` | Output format (default: text) |
| `-t, --type <json\|yaml\|toml\|csv>` | Force input format (bypass auto-detection) |
| `-i, --ignore <PATH>` | Paths to ignore (dot-separated, repeatable) |
| `--no-rename` | Disable rename detection (report as separate add/remove) |

#### Rename detection

By default, StrucDiff detects renamed keys. When a key is removed from one side and a similar key is added on the other (similarity > 0.6 by Levenshtein distance after normalization), it's reported as a rename rather than separate add/remove operations.

```bash
# Rename detection is ON by default
strucdiff diff old.json new.json

# Disable rename detection
strucdiff diff old.json new.json --no-rename
```

**Text output with rename:**

```
✗ Files differ
  old.json  vs  new.json
  3 change(s) found

  → userName → username renamed
      - "alice"
      + "alice"
  → emailAddress → email_address renamed
      - "alice@example.com"
      + "alice@example.com"
  ~ age changed
      - 30
      + 31
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
  "changes": 3,
  "entries": [
    {
      "path": "username",
      "kind": "renamed",
      "old_value": "\"alice\"",
      "new_value": "\"alice\"",
      "old_key": "userName",
      "new_key": "username"
    }
  ]
}
```

### Schema command

```bash
strucdiff schema <file> [options]
```

Infers and displays the schema of a structured data file. Supports three output formats:

| Option | Description |
|--------|-------------|
| `-f, --format <text\|json\|schema>` | Output format (default: text) |
| `-t, --type <json\|yaml\|toml\|csv>` | Force input format (bypass auto-detection) |

#### Text output (tree view)

```bash
strucdiff schema config.json
```

```
$ (object)
  $.active (boolean)
  $.address (object)
    $.address.city (string)
    $.address.street (string)
    $.address.zip (string)
  $.age (integer)
  $.name (string)
  $.scores (array of integer)
  $.tags (array of string)
```

#### JSON tree output

```bash
strucdiff schema config.json -f json
```

#### JSON Schema (Draft 2020-12) output

```bash
strucdiff schema config.json -f schema
```

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "name": { "type": "string" },
    "age": { "type": "integer" },
    "scores": {
      "type": "array",
      "items": { "type": "integer" }
    }
  },
  "required": ["name", "age", "scores"]
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
# Diff from stdin
curl -s https://api.example.com/v1 | strucdiff diff - /tmp/expected.json

# Schema from stdin (defaults to JSON)
echo '{"name": "test", "value": 42}' | strucdiff schema - -f schema
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

- **`diff`** — Core diff engine. Recursively compares `serde_json::Value` trees and produces path-based diffs for objects, arrays, and primitives. Includes rename detection via Levenshtein distance with key normalization.
- **`format`** — Format parsers. Each supported format is parsed into a unified `serde_json::Value` representation. TOML values (including datetimes) are converted losslessly.
- **`output`** — Output rendering. Supports both human-readable colorized text and machine-readable JSON. Handles added, removed, changed, and renamed entries.
- **`schema`** — Schema inference. Walks a `serde_json::Value` tree and produces a `SchemaNode` with type information, nested object schemas, array item types, and required fields. Outputs as text tree, JSON tree, or JSON Schema (Draft 2020-12).

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

And it detects renames that line-diff would miss entirely:

```
  → userName → username renamed
      - "alice"
      + "alice"
```

## License

MIT — see [LICENSE](LICENSE)

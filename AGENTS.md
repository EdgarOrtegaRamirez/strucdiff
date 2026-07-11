# AGENTS.md — for AI coding agents

## Project Overview

StrucDiff is a Rust CLI tool that semantically diffs structured data files (JSON, YAML, TOML, CSV). Unlike `diff`, it parses each format into `serde_json::Value` and compares at the structural level. It also features rename detection and schema inference.

## Project Structure

```
strucdiff/
├── Cargo.toml          # Rust project config (clap, serde_json, serde_yaml, toml, csv, colored, walkdir)
├── src/
│   ├── main.rs         # CLI entry point with clap Subcommand (Diff, Dir, Schema)
│   ├── diff.rs         # Core diff engine — recursive value comparison, rename detection, Levenshtein
│   ├── format.rs       # Format parsers — JSON, YAML, TOML, CSV → serde_json::Value
│   ├── output.rs       # Output renderers — Text (colorized) and JSON, handles Renamed entries
│   └── schema.rs       # Schema inference — type detection, JSON Schema (Draft 2020-12) generation
├── .github/workflows/ci.yml  # CI build + test pipeline
├── README.md           # Full documentation
├── LICENSE             # MIT
├── AGENTS.md           # This file
└── .gitignore
```

## Build & Test Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run all tests (67 tests)
cargo clippy             # Lint
cargo fmt                # Format code
```

## Key Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| clap | 4.5 | CLI argument parsing (derive mode) |
| serde_json | 1 | Universal value representation + JSON parsing |
| serde_yaml | 0.9 | YAML parsing |
| toml | 0.8 | TOML parsing |
| csv | 1.3 | CSV parsing |
| colored | 2.1 | Terminal color output |
| walkdir | 2.5 | Recursive directory traversal |

## Architecture Notes

1. **Unified representation**: All formats are parsed into `serde_json::Value`. This allows a single diff engine regardless of input format.
2. **Path-based diffs**: Changes are reported as dot-notation paths (e.g., `app.config.host`). Arrays use bracket notation (`items[0].name`).
3. **Format detection**: By file extension (.json, .yaml, .yml, .toml, .csv). Override with `--type`. Stdin defaults to JSON.
4. **Output separation**: Output rendering is decoupled from diff logic. Adding new output formats only requires a new renderer.
5. **Rename detection**: When comparing objects, keys only-in-left are matched to keys only-in-right by Levenshtein similarity (threshold 0.6). Keys are normalized (lowercase, separator unification) before comparison. Disable with `--no-rename`.
6. **Schema inference**: The `schema` subcommand walks a value tree and infers types (integer, number, string, boolean, null, object, array), required fields, nested object schemas, and array item types. Outputs as text tree, JSON tree, or JSON Schema (Draft 2020-12).

## Key Algorithms

### Rename Detection
- `find_renames()` — matches keys-only-in-left to keys-only-in-right by similarity
- `key_similarity()` — normalizes keys (lowercase, unify separators -/_/./space to dash), then computes `1.0 - levenshtein/max_len`
- `levenshtein()` — standard edit distance with two-row DP for space efficiency
- Threshold: 0.6 (configurable via `DiffConfig.rename_threshold`)

### Schema Inference
- `infer_schema()` — recursively walks `serde_json::Value`, producing `SchemaNode` tree
- `merge_schemas()` — merges schemas from array items (for heterogeneous arrays)
- `to_json_schema()` — converts `SchemaNode` to JSON Schema (Draft 2020-12) `serde_json::Value`
- `render_text()` — human-readable tree view
- `render_json_tree()` — JSON tree representation

## Adding a New Format

1. Add a variant to `Format` enum in `format.rs`
2. Implement a `parse_<format>()` function that returns `Result<Value, ParseError>`
3. Add the extension to `Format::extensions()`
4. Add the format to `Format::from_extension()`
5. Add tests to the `#[cfg(test)]` module

## Exit Codes

- 0: Files are identical
- 1: Files differ or error occurred

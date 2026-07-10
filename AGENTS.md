# AGENTS.md — for AI coding agents

## Project Overview

StrucDiff is a Rust CLI tool that semantically diffs structured data files (JSON, YAML, TOML, CSV). Unlike `diff`, it parses each format into `serde_json::Value` and compares at the structural level.

## Project Structure

```
strucdiff/
├── Cargo.toml          # Rust project config (clap, serde_json, serde_yaml, toml, csv, colored, walkdir)
├── src/
│   ├── main.rs         # CLI entry point with clap Subcommand (Diff, Dir)
│   ├── diff.rs         # Core diff engine — recursive value comparison, DiffResult/DiffEntry types
│   ├── format.rs       # Format parsers — JSON, YAML, TOML, CSV → serde_json::Value
│   └── output.rs       # Output renderers — Text (colorized terminal) and JSON
├── tests/              # Integration tests (future)
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
cargo test               # Run all tests (unit tests in doc modules)
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
3. **Format detection**: By file extension (.json, .yaml, .yml, .toml, .csv). Override with `--type`.
4. **Output separation**: Output rendering is decoupled from diff logic. Adding new output formats (e.g., HTML, YAML) only requires a new renderer.
5. **TOML conversion**: TOML's type system (datetimes, inline tables) is converted to serde_json::Value in `parse_toml()`.

## Adding a New Format

1. Add a variant to `Format` enum in `format.rs`
2. Implement a `parse_<format>()` function that returns `Result<Value, ParseError>`
3. Add the extension to `Format::extensions()`
4. Add the format to `Format::from_extension()`
5. Add tests to the `#[cfg(test)]` module

## Exit Codes

- 0: Files are identical
- 1: Files differ or error occurred
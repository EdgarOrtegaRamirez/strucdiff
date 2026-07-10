# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | ✅ Active          |

## Reporting a Vulnerability

This tool processes structured data files and is used in CI/CD pipelines. If you discover a security vulnerability, please open an issue or contact the maintainer directly.

## Security Considerations

### Input Validation
- All file inputs are validated before parsing. Invalid or malformed files produce clear error messages.
- File paths are read as provided — no path traversal vulnerabilities exist as the tool operates on user-specified files.

### File Parsing
- JSON, YAML, TOML, and CSV parsers are from well-maintained crates (`serde_json`, `serde_yaml`, `toml`, `csv`).
- TOML parsing converts values to `serde_json::Value` via a controlled conversion function that handles all TOML types (strings, integers, floats, booleans, datetimes, arrays, tables).

### Output Safety
- Terminal output uses ANSI color codes via the `colored` crate — safe for all terminal emulators.
- JSON output uses `serde_json::to_string_pretty` which properly escapes all special characters.

### No Network Access
- StrucDiff operates entirely on local files. It makes no network requests.
- The only external dependencies are Rust crate downloads at build time.

### No Code Execution
- StrucDiff parses data files only. It does not execute or evaluate any code from input files.
- YAML parsing uses `serde_yaml` which does not support arbitrary YAML tags or code execution.

## Best Practices

1. Always pin your StrucDiff version in CI/CD pipelines
2. Validate file sources before passing to StrucDiff
3. Use the JSON output format for programmatic consumption
4. Run tests locally before pushing to production
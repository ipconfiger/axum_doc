# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**axum_doc** is a CLI tool that generates OpenAPI 3.0 JSON specifications from Axum web framework projects through static analysis. Unlike macro-based alternatives (e.g., utoipa), it parses Rust source code to extract routing information without requiring code annotations.

**Key characteristics:**
- Single-file architecture (`src/main.rs` ~837 lines)
- AST-based parsing using `syn` crate
- Targets Axum 0.7 routing style
- No test coverage currently (noted improvement area in `docs/axum_doc.md`)

## Development Commands

### Build
```bash
cargo build --release
```

### Run the CLI tool
```bash
# Basic usage
cargo run -- --base-dir . --handler-file src/main.rs --model-files src/form.rs,src/response.rs,src/types.rs --output openapi.json

# After installation
axum_doc --base-dir . --handler-file src/main.rs --output openapi.json
```

### Install locally
```bash
cargo install --path .
```

### Check code (fast compile check)
```bash
cargo check
```

### Lint (fix warnings)
```bash
# Current warnings:
# - main.rs:577:17 - unused `mut`
# - main.rs:50:5 - unused field `base_path`
# - form.rs:8:9 - unused field `pass`
cargo clippy --fix
```

## Architecture

### Core Processing Pipeline
```
Input Files → AST Parsing → Route Extraction → Type Resolution → OpenAPI Generation
```

### Key Components (in `src/main.rs`)

1. **RouterVisitor** (lines 76-183)
   - AST visitor traversing Axum Router expressions via `syn::visit::Visit`
   - Tracks nested routes using `state_stack`
   - Extracts routes from `.route()` calls
   - Handles `.nest()` calls for modular routing
   - Maintains base_path context for nested modules

2. **Handler Parser** (lines 233-332)
   - Parses handler function signatures
   - Extracts documentation comments (`#[doc]` attributes)
   - Identifies Axum extractors: `Json`, `Query`, `Path`, `Form`

3. **Model Parser** (lines 352-379)
   - Parses struct definitions for OpenAPI schemas

4. **Type Mapping System** (lines 381-433)
   - Converts Rust types to OpenAPI schemas
   - Handles basic types: `String`, `i32`, `i64`, `f32`, `f64`, `bool`
   - Supports generics: `Vec<T>`, `Option<T>`
   - Maps to OpenAPI components

5. **OpenAPI Generator** (lines 435-623)
   - Constructs final OpenAPI 3.0 JSON

### Entry Point
`main()` function (line 727) using `clap` for CLI argument parsing:
- `--base-dir`: Project root (default: ".")
- `--handler-file`: Main router file (default: "src/main.rs")
- `--model-files`: Comma-separated model files (default: "src/form.rs,src/response.rs,src/types.rs")
- `--output`: Output file (default: "openapi-bak.json")

### Module Structure
- `src/main.rs` - All CLI logic and parsing
- `src/form.rs` - Example form models (UserLogin)
- `src/response.rs` - Example response models (UserProfile)
- `src/types.rs` - Example type models (User)
- `user/handlers.rs` - Example handler module

## Known Limitations (from `docs/axum_doc.md`)

1. **Incomplete module support** - Doesn't track `Router::merge()` calls
2. **Type mapping gaps** - Missing UUID, DateTime, and other common types
3. **Path prefix issues** - Nested routes may lose prefix context
4. **No configuration file** - All settings via CLI arguments
5. **No test coverage** - Critical validation gaps

## Important Design Patterns

- **Visitor Pattern** - Uses `syn::visit::Visit` for AST traversal
- **Stack-based state management** - Maintains nested route contexts
- **String-based route extraction** - Parses `.route()` and `.nest()` calls via regex
- **Doc comment extraction** - Parses `#[doc = "..."]` attributes for OpenAPI descriptions

## Dependencies

- `syn 2.0.104` (features: full, visit) - AST parsing
- `quote 1.0.40` - Code generation
- `serde_json 1.0.140` - JSON output
- `axum 0.7.5` - Target framework
- `clap 4.0` (features: derive) - CLI parsing
- `regex 1.11.1` - Path parameter extraction

## Notes for Modifications

- The codebase has no tests - when adding features, consider adding test coverage
- Current warnings should be addressed (unused `mut`, unused fields)
- Refer to `docs/axum_doc.md` for detailed improvement proposals
- The tool is in early development (v0.1.1) with documented architectural gaps

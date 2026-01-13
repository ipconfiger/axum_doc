# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**axum_doc** is a CLI tool that generates OpenAPI 3.0 JSON specifications from Axum web framework projects through static analysis. Unlike macro-based alternatives (e.g., utoipa), it parses Rust source code to extract routing information without requiring code annotations.

**Key characteristics:**
- Single-file architecture (`src/main.rs` ~1459 lines)
- AST-based parsing using `syn` crate
- Targets Axum 0.7 routing style
- **High test coverage**: 37 tests (22 unit + 15 integration)
- **Production-ready**: Zero clippy warnings, zero panic risks

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

### Lint and fix warnings
```bash
# Note: As of 2026-01-13, all clippy warnings have been fixed
cargo clippy -- -D warnings  # Should pass with zero warnings
```

## Architecture

### Core Processing Pipeline
```
Input Files → AST Parsing → Route Extraction → Type Resolution → OpenAPI Generation
```

### Key Components (in `src/main.rs`)

1. **RouterVisitor** (lines 94-156, 158-260)
   - AST visitor traversing Axum Router expressions via `syn::visit::Visit`
   - Tracks nested routes using `state_stack`
   - Extracts routes from `.route()` calls
   - Handles `.nest()` and `.merge()` calls for modular routing
   - **Shared method**: `visit_module_router()` (lines 101-156) - eliminates ~70 lines of duplicate code

2. **Handler Parser** (lines 262-370+)
   - Parses handler function signatures
   - Extracts documentation comments (`#[doc]` attributes)
   - Identifies Axum extractors: `Json`, `Query`, `Path`, `Form`
   - Supports tuple struct field handling

3. **Model Parser** (lines 527-575)
   - Parses struct definitions for OpenAPI schemas
   - Supports named structs and tuple structs
   - Handles `Fields::Named` and `Fields::Unnamed`

4. **Type Mapping System** (lines 605-658)
   - Converts Rust types to OpenAPI schemas
   - Handles basic types: `String`, `i32`, `i64`, `f32`, `f64`, `bool`, `usize`, `isize`
   - Supports special types: `uuid::Uuid`, `chrono::DateTime`, `std::time::Duration`
   - Supports generics: `Vec<T>`, `Option<T>`, `HashMap<K,V>`
   - Maps to OpenAPI components

5. **OpenAPI Generator** (lines 660-810+)
   - Constructs final OpenAPI 3.0 JSON
   - Generates paths, parameters, request bodies, responses, schemas

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

## Important Design Patterns

- **Visitor Pattern** - Uses `syn::visit::Visit` for AST traversal
- **Stack-based state management** - Maintains nested route contexts
- **String-based route extraction** - Parses `.route()` and `.nest()` calls via regex
- **Doc comment extraction** - Parses `#[doc = "..."]` attributes for OpenAPI descriptions
- **Shared method pattern** - `visit_module_router()` eliminates code duplication

## Dependencies

- `syn 2.0.104` (features: full, visit) - AST parsing
- `quote 1.0.40` - Code generation
- `serde_json 1.0.140` - JSON output
- `axum 0.7.5` - Target framework
- `clap 4.0` (features: derive) - CLI parsing
- `regex 1.11.1` - Path parameter extraction
- `once_cell 1.19` - Precompiled regex (added 2026-01-13)

## Notes for Modifications

### Test Coverage
- **Excellent coverage**: 37 tests (22 unit + 15 integration)
- When adding features, always add corresponding tests
- Run `cargo test` to ensure all tests pass before committing

### Code Quality Standards
- **Zero tolerance for clippy warnings**: Run `cargo clippy -- -D warnings`
- **No panic in production code**: Use proper error handling with `Result` types
- **DRY principle**: Extract duplicate code into shared methods
- Follow existing patterns in the codebase

### Recent Improvements (2026-01-13)
- ✅ Fixed all 6 clippy warnings
- ✅ Eliminated all 3 panic risks
- ✅ Extracted ~70 lines of duplicate code into shared method `visit_module_router()`
- ✅ Added tuple struct support
- ✅ Precompiled regex for better performance

### Documentation Updates
- Refer to `docs/axum_doc.md` for detailed project history and improvements
- Keep this file (CLAUDE.md) updated when making architectural changes
- Update README.md when adding user-facing features

### Current Version
- **Version**: v0.2.1
- **Status**: Production-ready
- **Quality**: High (100% test pass, zero warnings)

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

CodePrism is a high-performance code analysis tool built with Rust that scans Git repositories, extracts metrics, and provides insights through a web dashboard. It uses a server-driven UI architecture where the backend handles complex aggregation logic and the frontend renders generic chart components.

The project consists of:
- **Rust backend**: CLI tool and API server with embedded SQLite database
- **React frontend**: Built with Vite, TypeScript, and ECharts, embedded in the binary
- **Modular architecture**: Split across multiple Rust crates in a workspace

## Architecture

### Workspace Structure
- `crates/core`: Shared types, enums, error definitions, and configuration parsing (YAML-based)
- `crates/database`: SQLite connections and migration auto-application
- `crates/git_scanner`: Git repository interaction (snapshot and diff scanning) via git2
- `crates/analyzer`: Pure function analyzers that process file content into metrics
- `crates/server`: Axum web server, aggregation engine, and Swagger UI
- `src/main.rs`: CLI entry point using clap for command parsing
- `web/`: React frontend

### Cross-Crate Dependency Flow
```
core  ←  database  ←  git_scanner  →  analyzer
  ↓                                      ↑
 server  ────────────────────────────────┘
(imports database + core via scanner)
```
- `core` has zero crate dependencies (only serde + thiserror)
- `database` depends on `core`
- `analyzer` depends on `core`
- `git_scanner` depends on `core`, `database`, `analyzer` — this is the coordinator crate
- `server` depends on `core`, `database`, `git_scanner` (via Scanner), and `analyzer` (indirectly)

### Data Flow
1. **Scanning**: Git scanner reads repository (snapshot or diff mode) via libgit2
2. **Analysis**: File content passed through configured analyzers per tech stack
3. **Storage**: Metrics stored in SQLite with change type metadata
4. **Aggregation**: Server processes metrics into views defined in YAML config
5. **Presentation**: Frontend displays server-generated chart data

### Key Architectural Patterns

**Scanner: Channel-Based Decoupling**: `crates/git_scanner/src/lib.rs` uses `tokio::sync::mpsc` channels to bridge synchronous git2 operations (run via `spawn_blocking`) with asynchronous database writes. The git walker/diff calculator runs in a blocking thread, sending `ScanEvent` messages to the main async loop for DB persistence.

**Analyzer Extension System**: Four analyzer types loaded in `crates/git_scanner/src/lib.rs::with_config()`:
- `FileCountAnalyzer` / `CharCountAnalyzer` — built-in Rust structs
- `RegexAnalyzer` — configured via YAML (`custom_regex_analyzers`), compiled at startup
- `ScriptAnalyzer` — Python scripts auto-discovered from `custom_analyzers/` directory
- `WasmAnalyzer` — WASM modules referenced by path in YAML (`external_analyzers`)

**Config: Multi-Project with Legacy Support**: `CodePrismConfig` supports both a `projects: []` list (multi-project) and root-level fields (legacy single-project). `get_projects()` merges legacy fields into a default project if `projects` is empty. This dual-format design is important to preserve when editing config code.

**Server Config Conversion**: `crates/server/src/lib.rs::convert_project_views()` transforms core `AggregationFunc` enums into server-side `ViewKind` enums (TopN, Sum, Avg, Min, Max, Distribution with SourceConfig). This is the boundary between config parsing and server logic.

### Key Concepts
- **Tech Stack Classification**: Files categorized by extension, each stack has specific analyzers
- **Server-Driven UI**: Dashboard views defined in `codeprism.yaml` configuration
- **Everything is Metric**: All analysis outputs stored as `(analyzer_id, metric_key, value)` tuples
- **Change Types**: A (Add), M (Modify), D (Delete) for diff scanning

## Common Development Tasks

### Building
```bash
# Debug build (faster iteration)
cargo build

# Release build (also triggers frontend build via build.rs)
cargo build --release

# Build a specific crate
cargo build -p codeprism-scanner

# Binary location: target/debug/codeprism or target/release/codeprism
```

### Frontend Development
```bash
cd web
npm install
npm run dev          # Dev server on port 5173 (hot reload)
npm run build        # Production build to web/dist/
npm run lint         # ESLint
npm run preview      # Preview production build
```

### Running Tests
```bash
# All workspace tests
cargo test

# Specific crate tests
cargo test -p codeprism-scanner
cargo test -p codeprism-core
cargo test -p codeprism-server

# Single test function (supports substring matching)
cargo test test_load_config

# Frontend tests
cd web && npm test

# Watch mode for frontend tests
cd web && npm run test:watch
```

### CLI Development
```bash
# Run any CLI command during development
cargo run -- init
cargo run -- scan .
cargo run -- serve

# With custom config path
cargo run -- --config custom.yaml scan /path/to/repo

# Serve on custom port
cargo run -- serve --port 8080
```

### Other Commands
```bash
# Generate default config
cargo run -- init-config

# Validate config
cargo run -- check-config

# Test custom Python analyzers
cargo run -- test-analyzers
```

## Configuration

- Primary config file: `codeprism.yaml` (generated with `init-config`)
- Supports multi-project configurations with per-project tech stacks and views
- View display rules: undefined or empty `tech_stacks` → Summary tab; specific stacks → corresponding tabs; `"All"` → Summary tab
- `change_type_mode`: `"all"` (stacked chart by A/M/D), `"switchable"` (A/M/D toggle buttons), undefined (no change-type filtering)

### Analyzer Config Format
```yaml
custom_regex_analyzers:
  my_finder:
    pattern: "\\b(pattern)\\b"
    metric_key: "my_metric"
    category: "my_category"

custom_impl_analyzers:
  python_analyzer:
    metric_key: "complexity"
    category: "maintainability"
```

## API

- Swagger UI available at `/swagger-ui` when server is running
- OpenAPI spec at `/api-docs/openapi.json`
- Primary endpoint: `GET /api/v1/projects/:project_name/scans/:scan_id/views/:view_id`
  - Query params: `tech_stack`, `category`, `metric_key`, `change_type`, `group_by`
- Scan listing: `GET /api/v1/projects/:project_name/scans?mode=SNAPSHOT|DIFF`
- Config: `GET /api/v1/config`
- Frontend fallback: SPA-style, all non-API routes serve `index.html`

## Important Paths

- `crates/core/src/lib.rs` (config.rs merged into lib.rs): Configuration structure, parsing, validation, and MetricEntry/ChangeType types
- `crates/git_scanner/src/lib.rs`: Scanner with channel-based git → DB bridge
- `crates/server/src/lib.rs`: Server setup, config conversion, Swagger UI wiring
- `crates/server/src/routes.rs`: API route handlers for views, scans, config, static assets
- `crates/server/src/aggregation.rs`: Aggregation engine (TopN, Sum, Avg, Min, Max, Distribution)
- `crates/server/src/config.rs`: Server-side ViewKind/SourceConfig types
- `crates/server/build.rs`: Automatic frontend build during `cargo build`
- `crates/analyzer/src/lib.rs`: Analyzer trait and built-in implementations
- `migrations/`: SQLx SQLite migration files
- `custom_analyzers/`: Python-based custom analyzers (auto-discovered)
- `web/src/services/data.ts`: Frontend API client
- `web/src/contexts/AppContext.tsx`: React context for global state
- `web/src/components/Dashboard.tsx`: Main dashboard page with all chart rendering logic
- `web/src/components/ChartRenderer.tsx`: Generic ECharts wrapper component

## Development Notes

- **Frontend embedding**: `crates/server/build.rs` runs npm install/build automatically. The built assets in `web/dist/` are embedded into the binary using `rust-embed`. If npm is unavailable, manual build is required or the build fails.
- **Git interaction**: Uses git2 (libgit2) to read directly from Git ODB via Tree/Blob/Diff APIs — no checkout needed.
- **Error handling**: Uses `anyhow` for application-level errors, `thiserror` for library errors in `core`.
- **Database**: SQLite with SQLx for compile-time query checking. Schema migrations auto-apply on connect.
- **Docker**: Minimal `debian:bookworm-slim` image, expects pre-built binary at `docker-build/codeprism`.
- **Rust edition**: 2024 edition across all crates.

## Code Style & Patterns

- Use workspace dependencies between crates
- Prefer pure functions in analyzer crate
- Database queries use SQLx macros for compile-time validation
- Frontend organized by technical layers (components/, contexts/, services/, lib/)

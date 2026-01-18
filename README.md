# CodePrism User Guide

CodePrism is a high-performance code analysis tool designed to scan Git repositories, extract metrics, and provide insights. Built with Rust and Server-Driven UI architecture.

## 🚀 Installation

Ensure you have Rust installed. Clone the repository and build:

```bash
git clone https://github.com/your-repo/codeprism.git
cd codeprism
cargo build --release
```

The binary will be available at `target/release/codeprism`.

## 🛠️ Usage

CodePrism is a CLI tool. The main entry point is the `codeprism` command.

### 1. Initialize Project

Run `init` to setup the local database (`.codeprism.db`).

```bash
cargo run -- init
```

This creates the SQLite database and applies the initial schema.


### 2. Scan a Repository

CodePrism supports two scanning modes: **Snapshot** (full analysis) and **Diff** (delta analysis).

#### 2.1 Snapshot Mode (Default)

Analyzes the entire codebase at a specific point in time (HEAD or specific commit).

```bash
# Scan current HEAD
cargo run -- scan .

# Scan a specific commit
cargo run -- scan . --commit <COMMIT_HASH>

# Scan with custom project name
cargo run -- scan ../other-repo --project "legacy-app"
```

#### 2.2 Diff Mode

Analyzes changes between two commits (Base vs Target). Only files Added or Modified in the Target (relative to Base) are analyzed.

```bash
# Compare HEAD (Target) against a base commit
cargo run -- scan . --mode diff --base <BASE_COMMIT_HASH>

# Compare two specific commits
# Compare two specific commits
cargo run -- scan . --mode diff --base <OLD_HASH> --target <NEW_HASH>
```

### 3. Start API Server

Start the REST API server to query aggregation views.

```bash
# Start server on default port (3000)
cargo run -- serve

# Start on custom port
cargo run -- serve --port 8080
```

Once running, you can access the Swagger UI documentation at:
`http://localhost:3000/swagger-ui`

**What happens during a scan?**
1.  **Detection**: CodePrism identifies the target Commit and Branch (or Diff Base/Target).
2.  **Traversal**: 
    -   *Snapshot*: Walks the Git Tree (all files).
    -   *Diff*: Calculates git diff (only changed files).
3.  **Analysis**: Runs configured analyzers (e.g., Line Counter) on the identified files.
4.  **Storage**: Saves atomic metrics and file change events into the SQLite database.

## 📊 Architecture

-   **Core**: Rust Logic for maximum performance.
-   **Database**: SQLite for portable, structured data storage.
-   **Architecture**: "Everything is Metric" - flat storage of analysis results.

## 🤝 Contributing

We welcome contributions! Please check `PROJECT_BLUEPRINT.md` and `STRUCTURE_AND_MODULES.md` for architectural guidelines.

# Scanning Strategy & Testing Guide

## 1. Overview
CodePrism supports two scanning strategies:
1.  **Snapshot Mode**: Analyzes a full commit state (Tree).
2.  **Diff Mode**: Analyzes the delta between two commits.

## 2. Command Line Usage

### Snapshot Mode (Default)
Scans the current HEAD or a specific commit.

```bash
# Scan current HEAD
cargo run -- scan .

# Scan specific commit
cargo run -- scan . --commit <COMMIT_HASH>
```

### Diff Mode
Scans the changes between a Base commit and a Target commit.

```bash
# Compare HEAD against a previous commit
cargo run -- scan . --mode diff --base <OLD_HASH>

# Compare two specific commits
cargo run -- scan . --mode diff --base <OLD_HASH> --target <NEW_HASH>
```

## 3. Testing Guide

We have implemented an integration test suite that verifies both modes using a real temporary Git repository.

### Run tests
```bash
cargo test --package codeprism-scanner
```

### Expected Output
```
test test_git_scan_integration ... ok
```

### Test Logic
The `integration.rs` test performs the following:
1.  Creates a temporary git repo.
2.  **Commit 1**: Adds `file1.txt`.
3.  **Commit 2**: Modifies `file1.txt`, Adds `file2.txt`.
4.  **Snapshot Scan**: Runs on Commit 2. Verifies metrics exist.
5.  **Diff Scan**: Runs Commit 1 -> Commit 2.
    *   Verifies `file1.txt` is marked as **M** (Modified).
    *   Verifies `file2.txt` is marked as **A** (Added).

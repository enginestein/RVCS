# RVCS — A Locally Running CLI-First Version Control System

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)]()
[![Tests](https://img.shields.io/badge/tests-191%20passing-brightgreen)]()
[![License](https://img.shields.io/badge/license-MIT-blue)]()
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)]()

> **rvcs** is a lightweight version control system written in Rust. It provides content-addressable storage, branching, and a clean CLI — all without any external server or daemon.

---

## Table of Contents

- [RVCS — A Locally Running CLI-First Version Control System](#rvcs--a-locally-running-cli-first-version-control-system)
  - [Table of Contents](#table-of-contents)
  - [Features](#features)
  - [Architecture Overview](#architecture-overview)
    - [Object Model (Git-compatible)](#object-model-git-compatible)
  - [Installation](#installation)
    - [Prerequisites](#prerequisites)
    - [Build from Source](#build-from-source)
    - [Install Globally (Optional)](#install-globally-optional)
    - [Verify Installation](#verify-installation)
  - [Quick Start](#quick-start)
  - [Commands Reference](#commands-reference)
    - [Core Commands](#core-commands)
    - [Branch Commands](#branch-commands)
  - [Branch Workflow (Safe Experimentation)](#branch-workflow-safe-experimentation)
    - [Branch vs Revert vs Checkout](#branch-vs-revert-vs-checkout)
  - [How It Works](#how-it-works)
    - [Object Storage](#object-storage)
    - [Commit Creation](#commit-creation)
    - [Branching](#branching)
    - [Diff Algorithm](#diff-algorithm)
  - [Testing](#testing)
  - [Contributing](#contributing)
    - [Code Style](#code-style)
  - [License](#license)
  - [FAQ](#faq)

---

## Features

| Feature | Description |
|---------|-------------|
| **Content-addressable storage** | SHA-1 hashed blobs, trees, commits stored in `.rvcs/objects/` |
| **Zlib compression** | All objects compressed for efficient disk usage |
| **Staging area (index)** | Tracks files ready for commit |
| **Atomic commits** | Snapshot of entire working tree with author, message, timestamp |
| **Commit history** | Parent-linked commit graph with `log` |
| **Line-level diffs** | LCS-based diff algorithm (`diff`) |
| **Revert changes** | Restore files to last commit or any branch (`revert`) |
| **Branch management** | Create, list, switch, delete branches (`branch`, `switch`) |
| **Safe experimentation** | Snapshot working state, make risky changes, restore instantly |
| **Checkout commits/branches** | Jump to any point in history |
| **Zero dependencies at runtime** | Single binary, no daemon, no server |
| **Comprehensive test suite** | 191 tests (167 unit + 24 integration) |
| **Nested directory trees** | Recursive tree objects for accurate directory structure |
| **.rvcsignore** | Custom ignore patterns via `.rvcsignore` file |
| **diff --cached** | Show staged changes vs last commit |
| **Reset (soft/hard)** | Move HEAD without (soft) or with (hard) working tree changes |

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        rvcs Repository                          │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │  Working     │  │  Staging     │  │  .rvcs/                │ │
│  │  Directory   │──▶│  Area (Index)│──▶│  objects/   (blobs,    │ │
│  │  (your code) │  │  (index)     │  │  trees, commits)       │ │
│  └──────────────┘  └──────────────┘  │  refs/      (branches) │ │
│                                      │  HEAD       (ref/ptr)  │ │
│                                      │  index      (staging)  │ │
│                                      └────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### Object Model (Git-compatible)

- **Blob** — File content: `blob <size>\0<content>`
- **Tree** — Directory listing: `tree <size>\0<entries...>`
- **Commit** — Snapshot metadata: `commit <size>\0<header>\n\n<message>`

Each object is:
1. Serialized with type header + null terminator
2. Compressed with Zlib
3. Stored at `.rvcs/objects/<first2>/<rest38>`

---

## Installation

### Prerequisites

- **Rust 1.70+** (install via [rustup](https://rustup.rs/))
- **Cargo** (comes with Rust)

### Build from Source

```bash
# Clone the repository
git clone <your-repo-url> rvcs
cd rvcs

# Build release binary (optimized)
cargo build --release

# Binary is at: ./target/release/rvcs
```

### Install Globally (Optional)

```bash
# Install to ~/.cargo/bin (ensure it's in PATH)
cargo install --path .

# Or copy manually
cp target/release/rvcs /usr/local/bin/rvcs
```

### Verify Installation

```bash
rvcs --help
# A locally running CLI-first Version Control System
```

---

## Quick Start

```bash
# 1. Initialize a repository in your project
cd my-project
rvcs init

# 2. Create some files
echo "fn main() { println!(\"Hello, rvcs!\"); }" > main.rs
echo "# My Project" > README.md

# 3. Stage and commit
rvcs add main.rs README.md
rvcs commit -m "Initial commit" -a "Your Name <you@example.com>"

# 4. Make changes
echo "// Added feature" >> main.rs

# 5. See what changed
rvcs status
rvcs diff main.rs

# 6. Commit the change
rvcs add main.rs
rvcs commit -m "Add feature comment"

# 7. View history
rvcs log

# 8. Undo changes if needed
rvcs revert main.rs          # Restore single file to last commit
rvcs revert --from safe      # Restore ALL from 'safe' branch
```

---

## Commands Reference

### Core Commands

| Command | Description | Example |
|---------|-------------|---------|
| `rvcs init [path]` | Initialize new repository | `rvcs init .` |
| `rvcs add [files...]` | Stage files (empty = all) | `rvcs add src/main.rs` |
| `rvcs commit -m "msg" -a "author"` | Commit staged changes | `rvcs commit -m "Fix bug" -a "Me <me@test.com>"` |
| `rvcs status` | Show working tree status | `rvcs status` |
| `rvcs log` | Show commit history | `rvcs log` |
| `rvcs diff [file]` | Show line-level diffs | `rvcs diff src/main.rs` |
| `rvcs diff --cached [file]` | Show staged changes | `rvcs diff --cached` |
| `rvcs reset <target>` | Soft reset (move HEAD only) | `rvcs reset abc123` |
| `rvcs reset --hard <target>` | Hard reset (restore working tree) | `rvcs reset --hard abc123` |
| `rvcs revert [files...]` | Revert to last commit | `rvcs revert main.rs` |
| `rvcs revert --from <branch>` | Restore from branch | `rvcs revert --from safe` |
| `rvcs checkout <hash\|branch>` | Checkout commit or branch | `rvcs checkout abc123` |

### Branch Commands

| Command | Description | Example |
|---------|-------------|---------|
| `rvcs branch create <name>` | Create branch at HEAD | `rvcs branch create safe` |
| `rvcs branch list` | List all branches | `rvcs branch list` |
| `rvcs branch delete <name>` | Delete a branch | `rvcs branch delete old-feature` |
| `rvcs switch <name>` | Switch to branch | `rvcs switch feature` |

---

## Branch Workflow (Safe Experimentation)

**The Problem:** You want to try a risky refactor but need a quick way back if it breaks.

**The Solution:** Create a "safe" branch before you start.

```bash
# 1. Your code is working. Save this state.
rvcs branch create safe
# Created branch 'safe'
# * main
#   safe

# 2. Switch to main (or stay there) and experiment wildly
echo "risky_experiment()" > main.rs
rvcs add main.rs
rvcs commit -m "Try risky refactor"

# 3a. It worked! Keep it. (Optional: delete safe branch)
rvcs branch delete safe

# 3b. It broke! Restore instantly from safe branch:
rvcs revert --from safe
# Restored all files from branch 'safe'

# OR switch entirely:
rvcs switch safe
# Switched from 'main' to 'safe'
```

### Branch vs Revert vs Checkout

| Scenario | Use |
|----------|-----|
| Undo uncommitted changes to one file | `rvcs revert file.rs` |
| Undo ALL uncommitted changes | `rvcs revert` |
| Save checkpoint, experiment, maybe restore | `rvcs branch create safe` → `rvcs revert --from safe` |
| Switch between parallel lines of work | `rvcs switch feature` / `rvcs switch main` |
| View old version without changing working tree | `rvcs checkout abc123` |

---

## How It Works

### Object Storage

Every piece of data in rvcs is a **content-addressable object**:

```bash
# When you run: rvcs add file.txt
# 1. Read file content
# 2. Create blob object: "blob 12\0Hello World"
# 3. SHA-1 hash it: a1b2c3d4...
# 4. Zlib compress and store: .rvcs/objects/a1/b2c3d4...
# 5. Update index with path → hash mapping
```

### Commit Creation

```bash
# When you run: rvcs commit -m "msg" -a "author"
# 1. Build tree from index (recursive directory structure)
# 2. Store tree object
# 3. Create commit object with: tree, parent, author, timestamp, message
# 4. Store commit object
# 5. Update branch ref (HEAD) to new commit hash
# 6. Clear index
```

### Branching

Branches are **lightweight pointers** — just files in `.rvcs/refs/` containing a commit hash.

```bash
# .rvcs/refs/main     → "a1b2c3d4..."
# .rvcs/refs/safe     → "a1b2c3d4..."  (same commit initially)
# .rvcs/HEAD          → "ref: refs/main"

# Creating a branch: writes current HEAD hash to new ref file
# Switching: updates HEAD to point to different ref
# Deleting: removes ref file
```

### Diff Algorithm

Uses **Longest Common Subsequence (LCS)** for line-level diffs:

```rust
// O(n*m) dynamic programming table
// Backtracks to produce: Added, Removed, Context lines
```


---

## Testing

```bash
# Run all tests (unit + integration + doc)
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test integration_tests

# Run with output
cargo test -- --nocapture
```

---

## Contributing

1. **Fork** the repository
2. **Create a feature branch**: `rvcs branch create my-feature`
3. **Make changes** with tests
4. **Run tests**: `cargo test`
5. **Submit a PR**

### Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` for linting
- All tests must pass
- Add tests for new functionality

---

## License

MIT License

---

## FAQ

**Q: Can I use rvcs with a remote server?**
A: No — rvcs is intentionally local-only. For remote collaboration, use Git.

**Q: Is rvcs compatible with Git repositories?**
A: No — it uses a similar but distinct object model and storage format.

**Q: What's the maximum repository size?**
A: Limited only by disk space. Objects are compressed; tested with 100KB+ files.

**Q: Can I hook rvcs into my editor/IDE?**
A: Yes — the binary is self-contained. Call `rvcs` from any tool.

**Q: How do I backup my repository?**
A: Copy the entire project directory including `.rvcs/`. No special export needed.

---
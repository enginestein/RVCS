# RVCS — A Locally Running CLI-First Version Control System

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)]()
[![Tests](https://img.shields.io/badge/tests-210%20passing-brightgreen)]()
[![License](https://img.shields.io/badge/license-MIT-blue)]()
[![Version](https://img.shields.io/badge/version-0.3.0-blue)]()
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)]()

> **rvcs** is a lightweight version control system written in Rust. It provides content-addressable storage, branching, tagging, stashing, merging, and a clean CLI — all without any external server or daemon.

---

## Table of Contents

- [RVCS — A Locally Running CLI-First Version Control System](#rvcs--a-locally-running-cli-first-version-control-system)
  - [Table of Contents](#table-of-contents)
  - [Features](#features)
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
    - [Tag Commands](#tag-commands)
    - [Stash Commands](#stash-commands)
  - [Branch Workflow (Safe Experimentation)](#branch-workflow-safe-experimentation)
    - [Branch vs Revert vs Checkout](#branch-vs-revert-vs-checkout)
  - [Merge Workflow](#merge-workflow)
  - [How It Works](#how-it-works)
    - [Object Storage](#object-storage)
    - [Commit Creation](#commit-creation)
    - [Branching](#branching)
    - [Diff Algorithm](#diff-algorithm)
    - [Merge Algorithm](#merge-algorithm)
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
| **Checkout commits/branches** | Jump to any point in history |
| **Tag management** | Create (at HEAD or specific commit), list, delete tags (`tag`) |
| **Stash** | Temporarily shelve changes with push/pop/list/drop (`stash`) |
| **Merge** | Three-way merge of branches, automatic conflict detection with markers (`merge`) |
| **Remove files** | Delete from working tree and/or staging (`rm`) |
| **Safe experimentation** | Snapshot working state, make risky changes, restore instantly |
| **Zero dependencies at runtime** | Single binary, no daemon, no server |
| **Colorized output** | Syntax highlighting for status, diffs, logs (respects `NO_COLOR`) |
| **Comprehensive test suite** | 210 tests (186 unit + 24 integration) |
| **Nested directory trees** | Recursive tree objects for accurate directory structure |
| **.rvcsignore** | Custom ignore patterns via `.rvcsignore` file |
| **diff --cached** | Show staged changes vs last commit |
| **Reset (soft/hard)** | Move HEAD without (soft) or with (hard) working tree changes |

---


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
git clone https://github.com/enginestein/RVCS.git
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
| `rvcs commit -m "msg" -a "author"` | Commit staged changes | `rvcs commit -m "Fix bug"` |
| `rvcs status` | Show working tree status | `rvcs status` |
| `rvcs log` | Show commit history | `rvcs log` |
| `rvcs diff [file]` | Show line-level diffs | `rvcs diff src/main.rs` |
| `rvcs diff --cached [file]` | Show staged changes | `rvcs diff --cached` |
| `rvcs reset <target>` | Soft reset (move HEAD only) | `rvcs reset abc123` |
| `rvcs reset --hard <target>` | Hard reset (restore working tree) | `rvcs reset --hard abc123` |
| `rvcs revert [files...]` | Revert to last commit | `rvcs revert main.rs` |
| `rvcs revert --from <branch>` | Restore from branch | `rvcs revert --from safe` |
| `rvcs checkout <hash\|branch>` | Checkout commit or branch | `rvcs checkout abc123` |
| `rvcs rm [files...]` | Remove files (also from staging) | `rvcs rm old.txt` |
| `rvcs rm --staged [files...]` | Remove from staging only | `rvcs rm --staged secret.txt` |
| `rvcs merge <branch>` | Merge a branch into HEAD | `rvcs merge feature` |

### Branch Commands

| Command | Description | Example |
|---------|-------------|---------|
| `rvcs branch create <name>` | Create branch at HEAD | `rvcs branch create safe` |
| `rvcs branch list` | List all branches | `rvcs branch list` |
| `rvcs branch delete <name>` | Delete a branch | `rvcs branch delete old-feature` |
| `rvcs switch <name>` | Switch to branch | `rvcs switch feature` |

### Tag Commands

| Command | Description | Example |
|---------|-------------|---------|
| `rvcs tag create <name> [target]` | Create tag at HEAD or specific commit | `rvcs tag create v1.0` |
| `rvcs tag list` | List all tags | `rvcs tag list` |
| `rvcs tag delete <name>` | Delete a tag | `rvcs tag delete v1.0` |

### Stash Commands

| Command | Description | Example |
|---------|-------------|---------|
| `rvcs stash push` | Push staged changes onto the stack | `rvcs stash push` |
| `rvcs stash list` | List all stashes | `rvcs stash list` |
| `rvcs stash pop` | Restore and remove the latest stash | `rvcs stash pop` |
| `rvcs stash drop <name>` | Drop a specific stash | `rvcs stash drop stash@{0}` |

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
| Temporarily shelve changes | `rvcs stash push` / `rvcs stash pop` |

---

## Merge Workflow

```bash
# 1. Create a feature branch and do work
rvcs branch create feature
rvcs switch feature
echo "feature code" > new.txt
rvcs add new.txt
rvcs commit -m "Add feature" -a "Dev"

# 2. Switch back to main and merge
rvcs switch main
rvcs merge feature
# ✓ Merge successful: branch 'feature' merged into HEAD

# 3. If conflicts arise, resolve them manually:
rvcs merge risky-feature
# ✖ CONFLICT in file.txt
# ⚠ Merge with conflicts — resolve them, then commit
# > Files with conflicts:
#     ● file.txt
# (file.txt now contains conflict markers: <<<<<<<, =======, >>>>>>>)
```

For conflict resolution: edit the file, remove the `<<<<<<< HEAD`, `=======`, and `>>>>>>> branch` markers, keep the correct content, then `rvcs add` and `rvcs commit`.

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

### Merge Algorithm

The merge algorithm uses a **three-way merge** strategy:

1. Find the **merge base** (most recent common ancestor) of HEAD and the target branch via ancestor traversal
2. Compare files at HEAD, target branch, and ancestor
3. For each file:
   - **Same at HEAD and branch**: skip
   - **Changed in branch, unchanged in HEAD**: take branch version
   - **Changed in HEAD, unchanged in branch**: keep HEAD version
   - **Changed differently in both**: conflict — write conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`) to the file
4. If no conflicts, fast-forward HEAD to the merge target


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

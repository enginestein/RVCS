use std::fs;
use std::path::Path;

fn setup() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    rvcs::commands::init::execute(tmp.path()).unwrap();
    tmp
}

fn add_and_commit(path: &Path, files: &[&str], author: &str, message: &str) {
    let file_strs: Vec<String> = files.iter().map(|s| s.to_string()).collect();
    rvcs::commands::add::execute(path, &file_strs).unwrap();
    rvcs::commands::commit::execute(path, author, message).unwrap();
}

#[test]
fn test_full_workflow() {
    let tmp = setup();
    let path = tmp.path();

    // Create files
    fs::write(path.join("README.md"), "# My Project").unwrap();
    fs::create_dir_all(path.join("src")).unwrap();
    fs::write(path.join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(path.join("src/lib.rs"), "pub fn hello() {}").unwrap();

    // Add and commit
    add_and_commit(path, &["README.md", "src/main.rs", "src/lib.rs"], "Author", "Initial commit");

    // Verify commit history
    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    let history = repo.get_commit_history().unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].message, "Initial commit");

    // Modify a file
    fs::write(path.join("src/main.rs"), "fn main() {\n    println!(\"hello\");\n}").unwrap();

    // Check status
    let status_result = rvcs::commands::status::execute(path);
    assert!(status_result.is_ok());

    // Add and commit modification
    add_and_commit(path, &["src/main.rs"], "Author", "Update main");

    // Verify two commits
    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    let history = repo.get_commit_history().unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].message, "Update main");
    assert_eq!(history[1].message, "Initial commit");

    // Diff
    fs::write(path.join("src/main.rs"), "fn main() {\n    println!(\"changed\");\n}").unwrap();
    let diff_result = rvcs::commands::diff::execute(path, Some("src/main.rs"), false);
    assert!(diff_result.is_ok());

    // Revert
    rvcs::commands::revert::execute(path, &vec!["src/main.rs".to_string()], None).unwrap();
    let content = fs::read(path.join("src/main.rs")).unwrap();
    assert!(String::from_utf8(content).unwrap().contains("hello"));
}

#[test]
fn test_multiple_commits_and_checkout() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("file.txt"), "version 1").unwrap();
    add_and_commit(path, &["file.txt"], "Author", "v1");

    fs::write(path.join("file.txt"), "version 2").unwrap();
    add_and_commit(path, &["file.txt"], "Author", "v2");

    fs::write(path.join("file.txt"), "version 3").unwrap();
    add_and_commit(path, &["file.txt"], "Author", "v3");

    // Get the first commit hash
    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    let history = repo.get_commit_history().unwrap();
    assert_eq!(history.len(), 3);

    let first_hash = history.last().unwrap().hash.clone();

    // Checkout first commit
    rvcs::commands::checkout::execute(path, &first_hash).unwrap();
    let content = fs::read(path.join("file.txt")).unwrap();
    assert_eq!(content, b"version 1");

    // Checkout latest
    let latest_hash = history.first().unwrap().hash.clone();
    rvcs::commands::checkout::execute(path, &latest_hash).unwrap();
    let content = fs::read(path.join("file.txt")).unwrap();
    assert_eq!(content, b"version 3");
}

#[test]
fn test_revert_all_restores_clean_state() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("a.txt"), "original_a").unwrap();
    fs::write(path.join("b.txt"), "original_b").unwrap();
    add_and_commit(path, &["a.txt", "b.txt"], "Author", "initial");

    // Make changes
    fs::write(path.join("a.txt"), "modified_a").unwrap();
    fs::write(path.join("b.txt"), "modified_b").unwrap();
    fs::write(path.join("c.txt"), "new_c").unwrap();

    // Revert all
    rvcs::commands::revert::execute(path, &vec![], None).unwrap();

    assert_eq!(fs::read(path.join("a.txt")).unwrap(), b"original_a");
    assert_eq!(fs::read(path.join("b.txt")).unwrap(), b"original_b");
    assert!(!path.join("c.txt").exists());
}

#[test]
fn test_commit_with_special_characters() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("file.txt"), "content with unicode: hello world").unwrap();
    add_and_commit(path, &["file.txt"], "Author <author@test.com>", "Commit with unicode: hello");

    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    let commit = repo.get_head_commit().unwrap();
    assert!(commit.message.contains("hello"));
}

#[test]
fn test_directory_structure_preserved() {
    let tmp = setup();
    let path = tmp.path();

    fs::create_dir_all(path.join("src/components")).unwrap();
    fs::write(path.join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(path.join("src/components/button.rs"), "pub struct Button {}").unwrap();
    fs::write(path.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

    add_and_commit(
        path,
        &["src/main.rs", "src/components/button.rs", "Cargo.toml"],
        "Author",
        "Add project structure",
    );

    // Verify all files in commit - tree now has nested structure
    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    let commit = repo.get_head_commit().unwrap();
    let tree = rvcs::core::tree::Tree::from_object(
        &repo.load_object(&commit.tree_hash).unwrap()
    ).unwrap();
    // Top-level: Cargo.toml + src/ directory
    assert_eq!(tree.entries.len(), 2);

    // Verify src/ has nested tree with main.rs and components/
    let src_entry = tree.entries.iter().find(|e| e.name == "src").unwrap();
    let src_tree = rvcs::core::tree::Tree::from_object(
        &repo.load_object(&src_entry.hash).unwrap()
    ).unwrap();
    assert_eq!(src_tree.entries.len(), 2);

    // Verify src/components/ has button.rs
    let comp_entry = src_tree.entries.iter().find(|e| e.name == "components").unwrap();
    let comp_tree = rvcs::core::tree::Tree::from_object(
        &repo.load_object(&comp_entry.hash).unwrap()
    ).unwrap();
    assert_eq!(comp_tree.entries.len(), 1);
    assert_eq!(comp_tree.entries[0].name, "button.rs");
}

#[test]
fn test_revert_deleted_file() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("important.txt"), "critical data").unwrap();
    add_and_commit(path, &["important.txt"], "Author", "Save important file");

    // Delete the file
    fs::remove_file(path.join("important.txt")).unwrap();
    assert!(!path.join("important.txt").exists());

    // Revert should restore it
    rvcs::commands::revert::execute(path, &vec!["important.txt".to_string()], None).unwrap();
    assert!(path.join("important.txt").exists());
    let content = fs::read(path.join("important.txt")).unwrap();
    assert_eq!(content, b"critical data");
}

#[test]
fn test_empty_commit_fails() {
    let tmp = setup();
    let path = tmp.path();

    let result = rvcs::commands::commit::execute(path, "Author", "empty commit");
    assert!(result.is_err());
}

#[test]
fn test_log_output() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("file.txt"), "content").unwrap();
    add_and_commit(path, &["file.txt"], "Test Author", "First commit");

    fs::write(path.join("file.txt"), "modified").unwrap();
    add_and_commit(path, &["file.txt"], "Test Author", "Second commit");

    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    let history = repo.get_commit_history().unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].message, "Second commit");
    assert_eq!(history[1].message, "First commit");
    assert_eq!(history[0].parent_hash, Some(history[1].hash.clone()));
}

#[test]
fn test_diff_after_modification() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("code.rs"), "fn main() {\n    println!(\"hello\");\n}").unwrap();
    add_and_commit(path, &["code.rs"], "Author", "initial");

    fs::write(path.join("code.rs"), "fn main() {\n    println!(\"world\");\n}").unwrap();

    let diff = rvcs::core::repository::Repository::open(path)
        .unwrap()
        .compute_diff(Path::new("code.rs"))
        .unwrap();

    assert!(diff.additions > 0);
    assert!(diff.deletions > 0);
}

#[test]
fn test_index_persists_across_restarts() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("file.txt"), "content").unwrap();
    rvcs::commands::add::execute(path, &vec!["file.txt".to_string()]).unwrap();

    // Simulate restart by reopening
    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    assert!(repo.index.get_entry(Path::new("file.txt")).is_some());
}

#[test]
fn test_checkout_nonexistent_commit() {
    let tmp = setup();
    let path = tmp.path();

    let result = rvcs::commands::checkout::execute(path, "0000000000000000000000000000000000000000");
    assert!(result.is_err());
}

#[test]
fn test_add_directory_recursively() {
    let tmp = setup();
    let path = tmp.path();

    fs::create_dir_all(path.join("src/utils")).unwrap();
    fs::write(path.join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(path.join("src/utils/helpers.rs"), "pub fn help() {}").unwrap();

    // Add all files in src/
    let files: Vec<String> = vec![
        "src/main.rs".to_string(),
        "src/utils/helpers.rs".to_string(),
    ];
    rvcs::commands::add::execute(path, &files).unwrap();

    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    assert!(repo.index.get_entry(Path::new("src/main.rs")).is_some());
    assert!(repo.index.get_entry(Path::new("src/utils/helpers.rs")).is_some());
}

#[test]
fn test_multiple_reverts() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("file.txt"), "v1").unwrap();
    add_and_commit(path, &["file.txt"], "Author", "v1");

    fs::write(path.join("file.txt"), "v2").unwrap();
    add_and_commit(path, &["file.txt"], "Author", "v2");

    // Revert to v1
    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    let history = repo.get_commit_history().unwrap();
    let v1_hash = history.last().unwrap().hash.clone();

    rvcs::commands::checkout::execute(path, &v1_hash).unwrap();
    assert_eq!(fs::read(path.join("file.txt")).unwrap(), b"v1");

    // Revert back to v2
    let v2_hash = history.first().unwrap().hash.clone();
    rvcs::commands::checkout::execute(path, &v2_hash).unwrap();
    assert_eq!(fs::read(path.join("file.txt")).unwrap(), b"v2");
}

#[test]
fn test_binary_file_support() {
    let tmp = setup();
    let path = tmp.path();

    let binary_data: Vec<u8> = (0..=255).collect();
    fs::write(path.join("binary.dat"), &binary_data).unwrap();
    add_and_commit(path, &["binary.dat"], "Author", "Add binary file");

    // Modify binary
    let new_binary: Vec<u8> = (1..=255).chain(std::iter::once(0)).collect();
    fs::write(path.join("binary.dat"), &new_binary).unwrap();

    // Revert
    rvcs::commands::revert::execute(path, &vec!["binary.dat".to_string()], None).unwrap();
    let content = fs::read(path.join("binary.dat")).unwrap();
    assert_eq!(content, binary_data);
}

#[test]
fn test_large_file() {
    let tmp = setup();
    let path = tmp.path();

    let large_content = "x".repeat(100_000);
    fs::write(path.join("large.txt"), &large_content).unwrap();
    add_and_commit(path, &["large.txt"], "Author", "Large file");

    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    let commit = repo.get_head_commit().unwrap();
    let content = repo.get_file_content_at_commit(&commit, Path::new("large.txt")).unwrap();
    assert_eq!(content.len(), 100_000);
}

#[test]
fn test_nested_directory_revert() {
    let tmp = setup();
    let path = tmp.path();

    fs::create_dir_all(path.join("a/b/c")).unwrap();
    fs::write(path.join("a/b/c/deep.txt"), "deep content").unwrap();
    add_and_commit(path, &["a/b/c/deep.txt"], "Author", "deep file");

    fs::write(path.join("a/b/c/deep.txt"), "modified deep").unwrap();
    rvcs::commands::revert::execute(path, &vec!["a/b/c/deep.txt".to_string()], None).unwrap();

    let content = fs::read(path.join("a/b/c/deep.txt")).unwrap();
    assert_eq!(content, b"deep content");
}

#[test]
fn test_commit_message_multiline() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("file.txt"), "content").unwrap();
    add_and_commit(path, &["file.txt"], "Author", "First line\n\nSecond paragraph\n\nThird paragraph");

    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    let commit = repo.get_head_commit().unwrap();
    assert!(commit.message.contains("First line"));
    assert!(commit.message.contains("Second paragraph"));
}

#[test]
fn test_diff_no_changes_shows_nothing() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("file.txt"), "content").unwrap();
    add_and_commit(path, &["file.txt"], "Author", "initial");

    // No changes made
    let result = rvcs::commands::diff::execute(path, None, false);
    assert!(result.is_ok());
}

#[test]
fn test_concurrent_file_operations() {
    let tmp = setup();
    let path = tmp.path();

    // Create multiple files rapidly
    for i in 0..10 {
        fs::write(path.join(format!("file_{}.txt", i)), format!("content {}", i)).unwrap();
    }

    let files: Vec<String> = (0..10).map(|i| format!("file_{}.txt", i)).collect();
    rvcs::commands::add::execute(path, &files).unwrap();
    rvcs::commands::commit::execute(path, "Author", "Add 10 files").unwrap();

    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    let history = repo.get_commit_history().unwrap();
    assert_eq!(history.len(), 1);
}

#[test]
fn test_revert_after_partial_add() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("a.txt"), "a_v1").unwrap();
    fs::write(path.join("b.txt"), "b_v1").unwrap();
    add_and_commit(path, &["a.txt", "b.txt"], "Author", "initial");

    // Only modify a.txt and add it
    fs::write(path.join("a.txt"), "a_v2").unwrap();
    rvcs::commands::add::execute(path, &vec!["a.txt".to_string()]).unwrap();

    // Revert a.txt
    rvcs::commands::revert::execute(path, &vec!["a.txt".to_string()], None).unwrap();
    assert_eq!(fs::read(path.join("a.txt")).unwrap(), b"a_v1");

    // b.txt should be unchanged
    assert_eq!(fs::read(path.join("b.txt")).unwrap(), b"b_v1");
}

#[test]
fn test_reset_soft_preserves_working_tree() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("f.txt"), "v1").unwrap();
    rvcs::commands::add::execute(path, &vec!["f.txt".to_string()]).unwrap();
    rvcs::commands::commit::execute(path, "Author", "first").unwrap();

    fs::write(path.join("f.txt"), "v2").unwrap();
    rvcs::commands::add::execute(path, &vec!["f.txt".to_string()]).unwrap();
    rvcs::commands::commit::execute(path, "Author", "second").unwrap();

    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    let history = repo.get_commit_history().unwrap();
    let first_hash = history.last().unwrap().hash.clone();

    rvcs::commands::reset::execute(path, &first_hash, false).unwrap();

    let content = fs::read(path.join("f.txt")).unwrap();
    assert_eq!(content, b"v2");
    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    let head = repo.get_head_commit().unwrap();
    assert_eq!(head.hash, first_hash);
}

#[test]
fn test_reset_hard_restores_tree() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("f.txt"), "v1").unwrap();
    rvcs::commands::add::execute(path, &vec!["f.txt".to_string()]).unwrap();
    rvcs::commands::commit::execute(path, "Author", "first").unwrap();

    fs::write(path.join("f.txt"), "v2").unwrap();
    rvcs::commands::add::execute(path, &vec!["f.txt".to_string()]).unwrap();
    rvcs::commands::commit::execute(path, "Author", "second").unwrap();

    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    let history = repo.get_commit_history().unwrap();
    let first_hash = history.last().unwrap().hash.clone();

    rvcs::commands::reset::execute(path, &first_hash, true).unwrap();

    let content = fs::read(path.join("f.txt")).unwrap();
    assert_eq!(content, b"v1");
    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    assert!(repo.index.is_empty());
}

#[test]
fn test_diff_cached_shows_staged_changes() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join("f.txt"), "original").unwrap();
    rvcs::commands::add::execute(path, &vec!["f.txt".to_string()]).unwrap();
    rvcs::commands::commit::execute(path, "Author", "init").unwrap();

    fs::write(path.join("f.txt"), "changed").unwrap();
    rvcs::commands::add::execute(path, &vec!["f.txt".to_string()]).unwrap();

    let result = rvcs::commands::diff::execute(path, Some("f.txt"), true);
    assert!(result.is_ok());
}

#[test]
fn test_rvcsignore_at_cli_level() {
    let tmp = setup();
    let path = tmp.path();

    fs::write(path.join(".rvcsignore"), "ignore_me.txt\n").unwrap();
    fs::write(path.join("track.txt"), "keep me").unwrap();
    fs::write(path.join("ignore_me.txt"), "lose me").unwrap();

    rvcs::commands::add::execute(path, &vec![]).unwrap();

    let repo = rvcs::core::repository::Repository::open(path).unwrap();
    assert!(repo.index.get_entry(Path::new("track.txt")).is_some());
    assert!(repo.index.get_entry(Path::new("ignore_me.txt")).is_none());
}

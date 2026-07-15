use crate::core::repository::Repository;
use crate::error::Result;
use crate::utils::color::Color;
use std::path::Path;

pub fn execute(repo_path: &Path, author: &str, message: &str) -> Result<()> {
    let mut repo = Repository::open(repo_path)?;
    let file_count = repo.index.entries.len();
    let commit = repo.commit_staged(author, message)?;
    let branch = repo.get_current_branch().unwrap_or_else(|| "HEAD".to_string());
    let c = Color::new();
    println!("[{} {}] {} ({} file(s))", c.bold(&c.cyan(&branch)), c.yellow(&commit.hash[..12]), c.green(message), file_count);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{init, add};
    use std::fs;

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        init::execute(&path).unwrap();
        (tmp, path)
    }

    #[test]
    fn test_commit_basic() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        let result = execute(&path, "Test Author", "Initial commit");
        assert!(result.is_ok());
    }

    #[test]
    fn test_commit_no_staged() {
        let (_tmp, path) = setup();
        assert!(execute(&path, "Author", "msg").is_err());
    }

    #[test]
    fn test_commit_updates_history() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "v1").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        execute(&path, "Author", "first").unwrap();

        fs::write(path.join("file.txt"), "v2").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        execute(&path, "Author", "second").unwrap();

        let repo = Repository::open(&path).unwrap();
        let history = repo.get_commit_history().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].message, "second");
        assert_eq!(history[1].message, "first");
    }

    #[test]
    fn test_commit_hash_format() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        let mut repo = Repository::open(&path).unwrap();
        let commit = repo.commit_staged("Author", "msg").unwrap();
        assert_eq!(commit.hash.len(), 40);
    }

    #[test]
    fn test_commit_clears_staging() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()] ).unwrap();
        execute(&path, "Author", "msg").unwrap();

        let repo = Repository::open(&path).unwrap();
        assert!(repo.index.is_empty());
    }

    #[test]
    fn test_commit_empty_message_allowed() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        let result = execute(&path, "Author", "");
        assert!(result.is_ok());
    }

    #[test]
    fn test_commit_multiple_files() {
        let (_tmp, path) = setup();
        fs::write(path.join("a.txt"), "a").unwrap();
        fs::write(path.join("b.txt"), "b").unwrap();
        fs::write(path.join("c.txt"), "c").unwrap();
        add::execute(&path, &vec!["a.txt".to_string()]).unwrap();
        add::execute(&path, &vec!["b.txt".to_string()]).unwrap();
        add::execute(&path, &vec!["c.txt".to_string()]).unwrap();
        let result = execute(&path, "Author", "commit three");
        assert!(result.is_ok());

        let repo = Repository::open(&path).unwrap();
        let history = repo.get_commit_history().unwrap();
        assert_eq!(history.len(), 1);

        let tree_obj = repo.load_object(&history[0].tree_hash).unwrap();
        let tree = crate::core::tree::Tree::from_object(&tree_obj).unwrap();
        assert_eq!(tree.entries.len(), 3);
    }

    #[test]
    fn test_commit_author_with_special_chars() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "test").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        let result = execute(&path, "John (O'Brien) <john.obrien@test.com>", "author with special chars");
        assert!(result.is_ok());

        let repo = Repository::open(&path).unwrap();
        let commit = repo.get_head_commit().unwrap();
        assert_eq!(commit.author, "John (O'Brien) <john.obrien@test.com>");
    }
}

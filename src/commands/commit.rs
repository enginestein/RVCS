use crate::core::repository::Repository;
use crate::error::Result;
use std::path::Path;

pub fn execute(repo_path: &Path, author: &str, message: &str) -> Result<()> {
    let mut repo = Repository::open(repo_path)?;
    let commit = repo.commit_staged(author, message)?;
    println!("[main {}] {}", &commit.hash[..12], message);
    println!(" {} file(s) committed", repo.index.entries.len());
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
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        execute(&path, "Author", "msg").unwrap();

        let repo = Repository::open(&path).unwrap();
        assert!(repo.index.is_empty());
    }
}

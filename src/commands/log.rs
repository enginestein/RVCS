use crate::core::repository::Repository;
use crate::error::Result;
use std::path::Path;

pub fn execute(repo_path: &Path) -> Result<()> {
    let repo = Repository::open(repo_path)?;
    let history = repo.get_commit_history()?;

    if history.is_empty() {
        println!("No commits yet");
        return Ok(());
    }

    for commit in &history {
        print!("{}", commit);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{init, add, commit};
    use std::fs;

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        init::execute(&path).unwrap();
        (tmp, path)
    }

    #[test]
    fn test_log_empty() {
        let (_tmp, path) = setup();
        let result = execute(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_log_one_commit() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "Initial commit").unwrap();

        let result = execute(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_log_multiple_commits() {
        let (_tmp, path) = setup();
        fs::write(path.join("a.txt"), "first").unwrap();
        add::execute(&path, &vec!["a.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "First commit").unwrap();

        fs::write(path.join("b.txt"), "second").unwrap();
        add::execute(&path, &vec!["b.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "Second commit").unwrap();

        let repo = Repository::open(&path).unwrap();
        let history = repo.get_commit_history().unwrap();
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_log_displays_hash() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "data").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "test").unwrap();

        let repo = Repository::open(&path).unwrap();
        let commit = repo.get_head_commit().unwrap();
        assert_eq!(commit.hash.len(), 40);
    }

    #[test]
    fn test_log_displays_author() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "data").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "John Doe <john@example.com>", "msg").unwrap();

        let repo = Repository::open(&path).unwrap();
        let commit = repo.get_head_commit().unwrap();
        assert!(commit.author.contains("John Doe"));
    }
}

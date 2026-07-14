use crate::core::repository::Repository;
use crate::error::Result;
use std::path::Path;

pub fn execute(repo_path: &Path, commit_hash: &str) -> Result<()> {
    let mut repo = Repository::open(repo_path)?;

    // Verify the commit exists
    let _ = repo.load_object(commit_hash)?;

    repo.checkout_commit(commit_hash)?;
    println!("HEAD is now at {}", &commit_hash[..12.min(commit_hash.len())]);

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
    fn test_checkout_previous_commit() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "v1").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        let _c1 = commit::execute(&path, "Author", "first").unwrap();

        fs::write(path.join("file.txt"), "v2").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "second").unwrap();

        // Get the first commit hash
        let repo = Repository::open(&path).unwrap();
        let history = repo.get_commit_history().unwrap();
        let first_hash = history.last().unwrap().hash.clone();

        execute(&path, &first_hash).unwrap();

        let content = fs::read(path.join("file.txt")).unwrap();
        assert_eq!(content, b"v1");
    }

    #[test]
    fn test_checkout_invalid_hash() {
        let (_tmp, path) = setup();
        assert!(execute(&path, "0000000000000000000000000000000000000000").is_err());
    }

    #[test]
    fn test_checkout_restores_files() {
        let (_tmp, path) = setup();
        fs::write(path.join("a.txt"), "first_a").unwrap();
        fs::write(path.join("b.txt"), "first_b").unwrap();
        add::execute(&path, &vec!["a.txt".to_string(), "b.txt".to_string()]).unwrap();
        let _c1 = commit::execute(&path, "Author", "first").unwrap();

        // Remove b.txt, add c.txt
        fs::remove_file(path.join("b.txt")).unwrap();
        fs::write(path.join("c.txt"), "c_content").unwrap();
        add::execute(&path, &vec!["c.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "second").unwrap();

        // Checkout first commit
        let repo = Repository::open(&path).unwrap();
        let history = repo.get_commit_history().unwrap();
        let first_hash = history.last().unwrap().hash.clone();
        execute(&path, &first_hash).unwrap();

        assert!(path.join("a.txt").exists());
        assert!(path.join("b.txt").exists());
        assert!(!path.join("c.txt").exists());
    }

    #[test]
    fn test_checkout_updates_head() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "v1").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        let _c1 = commit::execute(&path, "Author", "first").unwrap();

        fs::write(path.join("file.txt"), "v2").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "second").unwrap();

        let repo = Repository::open(&path).unwrap();
        let history = repo.get_commit_history().unwrap();
        let first_hash = history.last().unwrap().hash.clone();

        execute(&path, &first_hash).unwrap();

        let repo = Repository::open(&path).unwrap();
        let current = repo.get_head_commit().unwrap();
        assert_eq!(current.hash, first_hash);
    }
}

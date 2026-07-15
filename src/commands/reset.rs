use crate::core::repository::Repository;
use crate::error::Result;
use std::path::Path;

pub fn execute(repo_path: &Path, target: &str, hard: bool) -> Result<()> {
    let mut repo = Repository::open(repo_path)?;

    if hard {
        repo.reset_hard(target)?;
        println!("HEAD is now at {} (hard reset)", target);
    } else {
        repo.reset_soft(target)?;
        println!("HEAD is now at {} (soft reset)", target);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{add, commit, init};
    use std::fs;

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        init::execute(&path).unwrap();
        (tmp, path)
    }

    fn get_head_hash(path: &Path) -> String {
        let repo = Repository::open(path).unwrap();
        repo.get_head_commit_hash().unwrap()
    }

    #[test]
    fn test_reset_soft_moves_head() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "v1").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "first").unwrap();
        let c1_hash = get_head_hash(&path);

        fs::write(path.join("file.txt"), "v2").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "second").unwrap();

        // Soft reset back to first commit
        execute(&path, &c1_hash, false).unwrap();

        let head = get_head_hash(&path);
        assert_eq!(head, c1_hash);
    }

    #[test]
    fn test_reset_soft_preserves_index() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "v1").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "first").unwrap();
        let c1_hash = get_head_hash(&path);

        // Add changes to staging but don't commit yet
        fs::write(path.join("file.txt"), "v2").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();

        // Soft reset to first commit — index should still have the staged changes
        execute(&path, &c1_hash, false).unwrap();

        let repo = Repository::open(&path).unwrap();
        // After soft reset, index still has the file staged (not cleared)
        assert!(!repo.index.is_empty(), "soft reset should preserve index");
        // Working tree is untouched
        let content = fs::read(path.join("file.txt")).unwrap();
        assert_eq!(content, b"v2");
    }

    #[test]
    fn test_reset_hard_restores_working_tree() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "v1").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "first").unwrap();
        let c1_hash = get_head_hash(&path);

        fs::write(path.join("file.txt"), "v2").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "second").unwrap();

        // Hard reset back to first commit
        execute(&path, &c1_hash, true).unwrap();

        let head = get_head_hash(&path);
        assert_eq!(head, c1_hash);
        let repo = Repository::open(&path).unwrap();
        assert!(repo.index.is_empty());
        let content = fs::read(path.join("file.txt")).unwrap();
        assert_eq!(content, b"v1");
    }

    #[test]
    fn test_reset_to_branch() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "first").unwrap();
        let commit_hash = get_head_hash(&path);

        // Reset using branch name
        execute(&path, "main", false).unwrap();

        let head = get_head_hash(&path);
        assert_eq!(head, commit_hash);
    }

    #[test]
    fn test_reset_nonexistent_target() {
        let (_tmp, path) = setup();
        assert!(execute(&path, "nonexistent", false).is_err());
    }
}

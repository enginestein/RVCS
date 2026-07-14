use crate::core::repository::Repository;
use crate::error::Result;
use std::path::Path;

pub fn execute(repo_path: &Path, name: &str) -> Result<()> {
    let mut repo = Repository::open(repo_path)?;
    let old_branch = repo.get_current_branch().unwrap_or_default();

    repo.switch_branch(name)?;

    let new_branch = repo.get_current_branch().unwrap_or_default();
    println!("Switched from '{}' to '{}'", old_branch, new_branch);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{add, branch, commit, init};
    use std::fs;

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        init::execute(&path).unwrap();
        (tmp, path)
    }

    #[test]
    fn test_switch_branch() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "v1").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "first").unwrap();

        branch::create(&path, "dev").unwrap();
        execute(&path, "main").unwrap();

        let repo = Repository::open(&path).unwrap();
        assert_eq!(repo.get_current_branch(), Some("main".to_string()));
    }

    #[test]
    fn test_switch_to_nonexistent_branch() {
        let (_tmp, path) = setup();
        assert!(execute(&path, "nope").is_err());
    }

    #[test]
    fn test_switch_restores_files() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "main content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "main commit").unwrap();

        branch::create(&path, "dev").unwrap();
        execute(&path, "dev").unwrap();

        fs::write(path.join("file.txt"), "dev content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "dev commit").unwrap();

        execute(&path, "main").unwrap();
        let content = fs::read(path.join("file.txt")).unwrap();
        assert_eq!(content, b"main content");
    }

    #[test]
    fn test_switch_and_back() {
        let (_tmp, path) = setup();
        fs::write(path.join("a.txt"), "on main").unwrap();
        add::execute(&path, &vec!["a.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "main").unwrap();

        branch::create(&path, "feature").unwrap();
        execute(&path, "feature").unwrap();

        fs::write(path.join("a.txt"), "on feature").unwrap();
        fs::write(path.join("b.txt"), "new in feature").unwrap();
        add::execute(&path, &vec!["a.txt".to_string(), "b.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "feature work").unwrap();

        execute(&path, "main").unwrap();
        let content = fs::read(path.join("a.txt")).unwrap();
        assert_eq!(content, b"on main");
        assert!(!path.join("b.txt").exists());

        execute(&path, "feature").unwrap();
        let content = fs::read(path.join("a.txt")).unwrap();
        assert_eq!(content, b"on feature");
        assert!(path.join("b.txt").exists());
    }
}

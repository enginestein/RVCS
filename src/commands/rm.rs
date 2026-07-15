use crate::core::repository::Repository;
use crate::error::{RvcsError, Result};
use crate::utils::color::Color;
use std::fs;
use std::path::Path;

pub fn execute(repo_path: &Path, files: &[String], staged: bool) -> Result<()> {
    let mut repo = Repository::open(repo_path)?;
    let c = Color::new();

    if files.is_empty() {
        return Err(RvcsError::Other("No files specified for rm".to_string()));
    }

    for file in files {
        let path = Path::new(file);

        if staged {
            repo.remove_from_staging(path);
            println!("  {} Removed from staging: {}", c.green("●"), file);
        } else {
            repo.remove_from_staging(path);
            let full_path = repo.root.join(path);
            if full_path.exists() {
                fs::remove_file(&full_path)?;
            }
            println!("  {} Removed: {}", c.red("●"), file);
        }
    }

    repo.save_index()?;
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

    #[test]
    fn test_rm_file() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "initial").unwrap();

        execute(&path, &vec!["file.txt".to_string()], false).unwrap();
        assert!(!path.join("file.txt").exists());

        let repo = Repository::open(&path).unwrap();
        assert!(repo.index.get_entry(Path::new("file.txt")).is_none());
    }

    #[test]
    fn test_rm_staged_only() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();

        execute(&path, &vec!["file.txt".to_string()], true).unwrap();
        assert!(path.join("file.txt").exists());

        let repo = Repository::open(&path).unwrap();
        assert!(repo.index.get_entry(Path::new("file.txt")).is_none());
    }

    #[test]
    fn test_rm_multiple_files() {
        let (_tmp, path) = setup();
        fs::write(path.join("a.txt"), "a").unwrap();
        fs::write(path.join("b.txt"), "b").unwrap();
        add::execute(&path, &vec!["a.txt".to_string(), "b.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "initial").unwrap();

        execute(&path, &vec!["a.txt".to_string(), "b.txt".to_string()], false).unwrap();
        assert!(!path.join("a.txt").exists());
        assert!(!path.join("b.txt").exists());
    }

    #[test]
    fn test_rm_no_files_error() {
        let (_tmp, path) = setup();
        assert!(execute(&path, &vec![], false).is_err());
    }

    #[test]
    fn test_rm_nonexistent_file() {
        let (_tmp, path) = setup();
        // Should not error — we just remove from staging if present
        let result = execute(&path, &vec!["nope.txt".to_string()], false);
        assert!(result.is_ok());
    }
}

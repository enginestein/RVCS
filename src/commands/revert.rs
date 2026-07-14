use crate::core::repository::Repository;
use crate::error::Result;
use std::path::Path;

pub fn execute(repo_path: &Path, files: &[String], from_branch: Option<&str>) -> Result<()> {
    let mut repo = Repository::open(repo_path)?;

    if let Some(branch) = from_branch {
        repo.revert_from_branch(branch)?;
        println!("Restored all files from branch '{}'", branch);
    } else if files.is_empty() {
        repo.revert_all()?;
        println!("Reverted all files to last commit");
    } else {
        for file in files {
            let path = Path::new(file);
            repo.revert_file(path)?;
            println!("Reverted: {}", file);
        }
    }

    repo.save_index()?;
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
    fn test_revert_single_file() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "original").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "first").unwrap();

        fs::write(path.join("file.txt"), "modified").unwrap();
        execute(&path, &vec!["file.txt".to_string()], None).unwrap();

        let content = fs::read(path.join("file.txt")).unwrap();
        assert_eq!(content, b"original");
    }

    #[test]
    fn test_revert_all_files() {
        let (_tmp, path) = setup();
        fs::write(path.join("a.txt"), "original_a").unwrap();
        fs::write(path.join("b.txt"), "original_b").unwrap();
        add::execute(&path, &vec!["a.txt".to_string(), "b.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "first").unwrap();

        fs::write(path.join("a.txt"), "modified_a");
        fs::write(path.join("b.txt"), "modified_b");

        execute(&path, &vec![], None).unwrap();

        assert_eq!(fs::read(path.join("a.txt")).unwrap(), b"original_a");
        assert_eq!(fs::read(path.join("b.txt")).unwrap(), b"original_b");
    }

    #[test]
    fn test_revert_removes_new_file() {
        let (_tmp, path) = setup();
        fs::write(path.join("existing.txt"), "content").unwrap();
        add::execute(&path, &vec!["existing.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "first").unwrap();

        fs::write(path.join("new.txt"), "new content").unwrap();
        execute(&path, &vec![], None).unwrap();

        assert!(!path.join("new.txt").exists());
    }

    #[test]
    fn test_revert_restores_deleted_file() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "first").unwrap();

        fs::remove_file(path.join("file.txt")).unwrap();
        execute(&path, &vec!["file.txt".to_string()], None).unwrap();

        assert!(path.join("file.txt").exists());
        let content = fs::read(path.join("file.txt")).unwrap();
        assert_eq!(content, b"content");
    }

    #[test]
    fn test_revert_multiple_files() {
        let (_tmp, path) = setup();
        fs::write(path.join("a.txt"), "a_v1").unwrap();
        fs::write(path.join("b.txt"), "b_v1").unwrap();
        add::execute(&path, &vec!["a.txt".to_string(), "b.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "first").unwrap();

        fs::write(path.join("a.txt"), "a_v2");
        fs::write(path.join("b.txt"), "b_v2");

        execute(&path, &vec!["a.txt".to_string(), "b.txt".to_string()], None).unwrap();

        assert_eq!(fs::read(path.join("a.txt")).unwrap(), b"a_v1");
        assert_eq!(fs::read(path.join("b.txt")).unwrap(), b"b_v1");
    }

    #[test]
    fn test_revert_no_commits() {
        let (_tmp, path) = setup();
        assert!(execute(&path, &vec!["file.txt".to_string()], None).is_err());
    }

    #[test]
    fn test_revert_from_branch() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "safe content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "safe state").unwrap();

        branch::create(&path, "safe").unwrap();

        // Make dangerous changes
        fs::write(path.join("file.txt"), "broken!").unwrap();
        fs::write(path.join("malicious.txt"), "bad stuff").unwrap();

        // Restore from safe branch
        execute(&path, &vec![], Some("safe")).unwrap();

        let content = fs::read(path.join("file.txt")).unwrap();
        assert_eq!(content, b"safe content");
        assert!(!path.join("malicious.txt").exists());
    }

    #[test]
    fn test_revert_from_branch_partial() {
        let (_tmp, path) = setup();
        fs::write(path.join("a.txt"), "a_safe").unwrap();
        fs::write(path.join("b.txt"), "b_safe").unwrap();
        add::execute(&path, &vec!["a.txt".to_string(), "b.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "safe").unwrap();

        branch::create(&path, "safe").unwrap();

        // Modify on main
        fs::write(path.join("a.txt"), "a_modified").unwrap();
        fs::write(path.join("c.txt"), "c_new").unwrap();

        // Restore from safe branch
        execute(&path, &vec![], Some("safe")).unwrap();

        assert_eq!(fs::read(path.join("a.txt")).unwrap(), b"a_safe");
        assert_eq!(fs::read(path.join("b.txt")).unwrap(), b"b_safe");
        assert!(!path.join("c.txt").exists());
    }

    #[test]
    fn test_revert_from_nonexistent_branch() {
        let (_tmp, path) = setup();
        assert!(execute(&path, &vec![], Some("nope")).is_err());
    }
}

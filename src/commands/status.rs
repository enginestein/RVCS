use crate::core::repository::{FileStatus, Repository};
use crate::error::Result;
use std::path::Path;

pub fn execute(repo_path: &Path) -> Result<()> {
    let repo = Repository::open(repo_path)?;
    let files = repo.get_working_tree_files()?;

    let mut new_files = Vec::new();
    let mut modified_files = Vec::new();
    let mut deleted_files = Vec::new();

    for file in &files {
        match repo.get_file_status(file)? {
            FileStatus::New => new_files.push(file.clone()),
            FileStatus::Modified => modified_files.push(file.clone()),
            FileStatus::Deleted => deleted_files.push(file.clone()),
            FileStatus::Unchanged => {}
        }
    }

    // Check for files in index but not in working tree (deleted)
    for (path, _) in repo.index.entries_sorted() {
        let full_path = repo.root.join(path);
        if !full_path.exists() && !deleted_files.contains(path) {
            deleted_files.push(path.clone());
        }
    }

    if new_files.is_empty() && modified_files.is_empty() && deleted_files.is_empty() {
        println!("nothing to commit, working tree clean");
        return Ok(());
    }

    if !new_files.is_empty() {
        println!("Changes staged for commit:");
        for file in &new_files {
            println!("\tnew file:   {}", file.display());
        }
    }

    if !modified_files.is_empty() {
        println!("Changes staged for commit:");
        for file in &modified_files {
            println!("\tmodified:   {}", file.display());
        }
    }

    if !deleted_files.is_empty() {
        println!("Changes not staged for commit:");
        for file in &deleted_files {
            println!("\tdeleted:    {}", file.display());
        }
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
    fn test_status_clean() {
        let (_tmp, path) = setup();
        let result = execute(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_new_file() {
        let (_tmp, path) = setup();
        fs::write(path.join("new.txt"), "content").unwrap();
        let result = execute(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_after_add() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        let result = execute(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_after_commit() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "msg").unwrap();

        let result = execute(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_modified_file() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "v1").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "msg").unwrap();

        fs::write(path.join("file.txt"), "v2").unwrap();
        let result = execute(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_unchanged_after_add() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();

        let repo = crate::core::repository::Repository::open(&path).unwrap();
        let status = repo.get_file_status(&std::path::Path::new("file.txt")).unwrap();
        assert_eq!(status, crate::core::repository::FileStatus::Unchanged);
    }

    #[test]
    fn test_status_deleted_in_index() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "init").unwrap();

        // file exists in index but removed in wd
        fs::remove_file(path.join("file.txt")).unwrap();
        // this should not panic or fail
        let result = execute(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_new_file_unstaged() {
        let (_tmp, path) = setup();
        fs::write(path.join("new.txt"), "new file").unwrap();
        let repo = crate::core::repository::Repository::open(&path).unwrap();
        let status = repo.get_file_status(&std::path::Path::new("new.txt")).unwrap();
        assert_eq!(status, crate::core::repository::FileStatus::New);
    }
}

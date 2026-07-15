use crate::core::repository::Repository;
use crate::error::Result;
use crate::utils::color::Color;
use std::path::Path;

pub fn execute(repo_path: &Path, files: &[String]) -> Result<()> {
    let mut repo = Repository::open(repo_path)?;
    let c = Color::new();

    if files.is_empty() {
        let working_files = repo.get_working_tree_files()?;
        for file in working_files {
            match repo.get_file_status(&file) {
                Ok(status) => {
                    use crate::core::repository::FileStatus;
                    if matches!(status, FileStatus::New | FileStatus::Modified) {
                        repo.add_file(&file)?;
                        println!("  {} {}", c.green("●"), file.display());
                    }
                }
                Err(_) => {}
            }
        }
    } else {
        for file in files {
            let path = Path::new(file);
            repo.add_file(path)?;
            println!("  {} {}", c.green("●"), file);
        }
    }

    repo.save_index()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init;
    use std::fs;

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        init::execute(&path).unwrap();
        (tmp, path)
    }

    #[test]
    fn test_add_specific_file() {
        let (_tmp, path) = setup();
        fs::write(path.join("test.txt"), "content").unwrap();
        execute(&path, &vec!["test.txt".to_string()]).unwrap();

        let repo = Repository::open(&path).unwrap();
        assert!(repo.index.get_entry(Path::new("test.txt")).is_some());
    }

    #[test]
    fn test_add_all_files() {
        let (_tmp, path) = setup();
        fs::write(path.join("a.txt"), "a").unwrap();
        fs::write(path.join("b.txt"), "b").unwrap();
        execute(&path, &vec![]).unwrap();

        let repo = Repository::open(&path).unwrap();
        assert!(repo.index.get_entry(Path::new("a.txt")).is_some());
        assert!(repo.index.get_entry(Path::new("b.txt")).is_some());
    }

    #[test]
    fn test_add_nonexistent_file() {
        let (_tmp, path) = setup();
        assert!(execute(&path, &vec!["nope.txt".to_string()]).is_err());
    }

    #[test]
    fn test_add_modifies_index() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.rs"), "fn main() {}").unwrap();
        execute(&path, &vec!["file.rs".to_string()]).unwrap();

        let repo = Repository::open(&path).unwrap();
        let entry = repo.index.get_entry(Path::new("file.rs")).unwrap();
        assert_eq!(entry.size, 12);
    }

    #[test]
    fn test_add_subdirectory_file() {
        let (_tmp, path) = setup();
        fs::create_dir_all(path.join("src")).unwrap();
        fs::write(path.join("src/main.rs"), "fn main() {}").unwrap();
        execute(&path, &vec!["src/main.rs".to_string()]).unwrap();

        let repo = Repository::open(&path).unwrap();
        assert!(repo.index.get_entry(Path::new("src/main.rs")).is_some());
    }

    #[test]
    fn test_add_absolute_path() {
        let (_tmp, path) = setup();
        let abs_path = path.join("absolute.txt");
        fs::write(&abs_path, "absolute path file").unwrap();
        execute(&path, &vec![abs_path.to_str().unwrap().to_string()]).unwrap();

        let repo = Repository::open(&path).unwrap();
        assert!(repo.index.get_entry(Path::new("absolute.txt")).is_some());
    }

    #[test]
    fn test_add_all_files_respects_rvcsignore() {
        let (_tmp, path) = setup();
        fs::write(path.join(".rvcsignore"), "ignored.txt\n").unwrap();
        fs::write(path.join("tracked.txt"), "track me").unwrap();
        fs::write(path.join("ignored.txt"), "ignore me").unwrap();

        execute(&path, &vec![]).unwrap();

        let repo = Repository::open(&path).unwrap();
        assert!(repo.index.get_entry(Path::new("tracked.txt")).is_some());
        assert!(repo.index.get_entry(Path::new("ignored.txt")).is_none());
    }

    #[test]
    fn test_add_after_modification_updates_hash() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "v1").unwrap();
        execute(&path, &vec!["file.txt".to_string()]).unwrap();

        let repo1 = Repository::open(&path).unwrap();
        let hash_before = repo1.index.get_entry(Path::new("file.txt")).unwrap().hash.clone();

        fs::write(path.join("file.txt"), "v2").unwrap();
        execute(&path, &vec!["file.txt".to_string()]).unwrap();

        let repo2 = Repository::open(&path).unwrap();
        let hash_after = repo2.index.get_entry(Path::new("file.txt")).unwrap().hash.clone();
        assert_ne!(hash_before, hash_after);
    }
}

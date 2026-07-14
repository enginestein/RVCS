use crate::core::repository::Repository;
use crate::error::Result;
use std::path::Path;

pub fn execute(path: &Path) -> Result<()> {
    let repo = Repository::init(path)?;
    println!("Initialized empty rvcs repository in {}", repo.rvcs_dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        execute(&path).unwrap();
        (tmp, path)
    }

    #[test]
    fn test_init_creates_directory() {
        let (_tmp, path) = setup();
        assert!(path.join(".rvcs").is_dir());
        assert!(path.join(".rvcs/objects").is_dir());
        assert!(path.join(".rvcs/refs").is_dir());
    }

    #[test]
    fn test_init_already_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        execute(&path).unwrap();
        assert!(execute(&path).is_err());
    }

    #[test]
    fn test_init_creates_head() {
        let (_tmp, path) = setup();
        let head = fs::read_to_string(path.join(".rvcs/HEAD")).unwrap();
        assert_eq!(head, "ref: refs/main\n");
    }

    #[test]
    fn test_init_creates_index() {
        let (_tmp, path) = setup();
        assert!(path.join(".rvcs/index").exists());
    }
}

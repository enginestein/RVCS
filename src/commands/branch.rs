use crate::core::repository::Repository;
use crate::error::Result;
use crate::utils::color::Color;
use std::path::Path;

pub fn create(repo_path: &Path, name: &str) -> Result<()> {
    let repo = Repository::open(repo_path)?;
    repo.create_branch(name)?;
    println!("Created branch '{}'", name);
    Ok(())
}

pub fn list(repo_path: &Path) -> Result<()> {
    let repo = Repository::open(repo_path)?;
    let branches = repo.list_branches()?;
    let c = Color::new();

    if branches.is_empty() {
        println!("{}", c.yellow("No branches yet"));
        return Ok(());
    }

    for (name, is_current) in &branches {
        if *is_current {
            println!("* {} {}", c.green("●"), c.bold(&c.green(name)));
        } else {
            println!("  {}", name);
        }
    }

    Ok(())
}

pub fn delete(repo_path: &Path, name: &str) -> Result<()> {
    let repo = Repository::open(repo_path)?;
    repo.delete_branch(name)?;
    println!("Deleted branch '{}'", name);
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
    fn test_create_branch() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "initial").unwrap();

        create(&path, "safe").unwrap();
        let repo = Repository::open(&path).unwrap();
        let branches = repo.list_branches().unwrap();
        assert!(branches.iter().any(|(n, _)| n == "safe"));
    }

    #[test]
    fn test_create_duplicate_branch() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "initial").unwrap();

        create(&path, "safe").unwrap();
        assert!(create(&path, "safe").is_err());
    }

    #[test]
    fn test_list_branches() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "initial").unwrap();

        create(&path, "dev").unwrap();
        let repo = Repository::open(&path).unwrap();
        let branches = repo.list_branches().unwrap();
        assert_eq!(branches.len(), 2);
        assert!(branches.iter().any(|(n, c)| n == "main" && *c));
        assert!(branches.iter().any(|(n, _)| n == "dev"));
    }

    #[test]
    fn test_delete_branch() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "initial").unwrap();

        create(&path, "temp").unwrap();
        delete(&path, "temp").unwrap();

        let repo = Repository::open(&path).unwrap();
        let branches = repo.list_branches().unwrap();
        assert!(!branches.iter().any(|(n, _)| n == "temp"));
    }

    #[test]
    fn test_delete_current_branch_fails() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "initial").unwrap();

        assert!(delete(&path, "main").is_err());
    }

    #[test]
    fn test_delete_nonexistent_branch() {
        let (_tmp, path) = setup();
        assert!(delete(&path, "nope").is_err());
    }

    #[test]
    fn test_invalid_branch_name() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "initial").unwrap();

        assert!(create(&path, "bad/name").is_err());
        assert!(create(&path, "bad name").is_err());
        assert!(create(&path, "-bad").is_err());
    }
}

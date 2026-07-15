use crate::core::repository::Repository;
use crate::error::{RvcsError, Result};
use crate::utils::color::Color;
use std::fs;
use std::path::Path;

pub fn create(repo_path: &Path, name: &str, target: Option<&str>) -> Result<()> {
    let repo = Repository::open(repo_path)?;
    let c = Color::new();

    if name.contains('/') || name.contains(' ') {
        return Err(RvcsError::Other(format!("Invalid tag name: '{}'", name)));
    }

    let tags_dir = repo.rvcs_dir.join("refs").join("tags");
    fs::create_dir_all(&tags_dir)?;

    let tag_path = tags_dir.join(name);
    if tag_path.exists() {
        return Err(RvcsError::Other(format!("Tag '{}' already exists", name)));
    }

    let hash = match target {
        Some(t) => repo.resolve_ref(t)?,
        None => repo.get_head_commit_hash()?,
    };

    fs::write(&tag_path, format!("{}\n", hash))?;
    println!("{} Created tag '{}' -> {}", c.green("✓"), c.bold(name), c.yellow(&hash[..12]));
    Ok(())
}

pub fn list(repo_path: &Path) -> Result<()> {
    let repo = Repository::open(repo_path)?;
    let c = Color::new();
    let tags_dir = repo.rvcs_dir.join("refs").join("tags");

    if !tags_dir.exists() {
        println!("{} No tags yet", c.yellow("●"));
        return Ok(());
    }

    let mut tags = Vec::new();
    for entry in fs::read_dir(&tags_dir)? {
        let entry = entry?;
        if entry.path().is_file() {
            let name = entry.file_name().to_str().unwrap().to_string();
            let hash = fs::read_to_string(entry.path())?;
            tags.push((name, hash.trim().to_string()));
        }
    }

    tags.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, hash) in &tags {
        println!("{}  {} -> {}", c.yellow("●"), c.bold(name), c.cyan(&hash[..12]));
    }

    Ok(())
}

pub fn delete(repo_path: &Path, name: &str) -> Result<()> {
    let c = Color::new();
    let repo = Repository::open(repo_path)?;
    let tag_path = repo.rvcs_dir.join("refs").join("tags").join(name);

    if !tag_path.exists() {
        return Err(RvcsError::Other(format!("Tag '{}' not found", name)));
    }

    fs::remove_file(&tag_path)?;
    println!("{} Deleted tag '{}'", c.red("✓"), c.bold(name));
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
    fn test_create_and_list_tags() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "initial").unwrap();

        create(&path, "v1.0", None).unwrap();

        let repo = Repository::open(&path).unwrap();
        let tag_path = repo.rvcs_dir.join("refs").join("tags").join("v1.0");
        assert!(tag_path.exists());
    }

    #[test]
    fn test_create_tag_at_specific_commit() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "v1").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "first").unwrap();

        let repo = Repository::open(&path).unwrap();
        let hash = repo.get_head_commit_hash().unwrap();

        create(&path, "v1.0", Some(&hash)).unwrap();

        let tag_hash = fs::read_to_string(repo.rvcs_dir.join("refs").join("tags").join("v1.0")).unwrap();
        assert_eq!(tag_hash.trim(), hash);
    }

    #[test]
    fn test_delete_tag() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "initial").unwrap();

        create(&path, "v1.0", None).unwrap();
        delete(&path, "v1.0").unwrap();

        let repo = Repository::open(&path).unwrap();
        let tag_path = repo.rvcs_dir.join("refs").join("tags").join("v1.0");
        assert!(!tag_path.exists());
    }

    #[test]
    fn test_duplicate_tag_fails() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "initial").unwrap();

        create(&path, "v1.0", None).unwrap();
        assert!(create(&path, "v1.0", None).is_err());
    }

    #[test]
    fn test_delete_nonexistent_tag() {
        let (_tmp, path) = setup();
        assert!(delete(&path, "nope").is_err());
    }

    #[test]
    fn test_invalid_tag_name() {
        let (_tmp, path) = setup();
        assert!(create(&path, "bad tag", None).is_err());
        assert!(create(&path, "bad/tag", None).is_err());
    }
}

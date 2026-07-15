use crate::core::repository::Repository;
use crate::error::{RvcsError, Result};
use crate::utils::color::Color;
use std::fs;
use std::path::Path;

pub fn push(repo_path: &Path) -> Result<()> {
    let mut repo = Repository::open(repo_path)?;
    let c = Color::new();

    if repo.index.is_empty() {
        println!("{} No staged changes to stash", c.yellow("●"));
        return Ok(());
    }

    let stash_dir = repo.rvcs_dir.join("refs").join("stash");
    fs::create_dir_all(&stash_dir)?;

    let stash_index = stash_dir.read_dir().map(|e| e.count()).unwrap_or(0);
    let stash_name = format!("stash@{{{}}}", stash_index);
    let stash_path = stash_dir.join(&stash_name);
    fs::create_dir_all(&stash_path)?;

    let head_hash = repo.get_head_commit_hash().ok();

    let mut stash_data = String::new();
    if let Some(ref h) = head_hash {
        stash_data.push_str(&format!("hash:{}\n", h));
    }
    stash_data.push_str(&format!("index_size:{}\n", repo.index.entries.len()));

    for (path, entry) in repo.index.entries_sorted() {
        let obj = repo.load_object(&entry.hash)?;
        let blob_path = stash_path.join(&entry.hash);
        fs::write(&blob_path, &obj.content)?;
        stash_data.push_str(&format!("entry:{}:{}\n", path.display(), entry.hash));
    }

    fs::write(stash_path.join("index"), &stash_data)?;

    repo.index.clear();
    repo.save_index()?;

    if let Some(ref h) = head_hash {
        let obj = repo.load_object(h)?;
        let commit = crate::core::commit::Commit::from_object(&obj)?;
        repo.revert_all_at_commit(&commit)?;
    }

    println!("{} Saved working directory {} ({} entries)", c.green("✓"), c.bold(&stash_name), stash_index);
    Ok(())
}

pub fn list(repo_path: &Path) -> Result<()> {
    let c = Color::new();
    let repo = Repository::open(repo_path)?;
    let stash_dir = repo.rvcs_dir.join("refs").join("stash");

    if !stash_dir.exists() {
        println!("{} No stashes found", c.yellow("●"));
        return Ok(());
    }

    let mut stashes: Vec<_> = fs::read_dir(&stash_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_str().unwrap().to_string())
        .collect();
    stashes.sort();

    if stashes.is_empty() {
        println!("{} No stashes found", c.yellow("●"));
        return Ok(());
    }

    for stash in &stashes {
        let index_path = stash_dir.join(stash).join("index");
        let size = fs::read_to_string(&index_path)
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("index_size:"))
                    .map(|l| l.strip_prefix("index_size:").unwrap_or("0").to_string())
            })
            .unwrap_or_else(|| "0".to_string());
        println!("  {} {} ({} entries)", c.yellow("●"), c.bold(stash), size);
    }

    Ok(())
}

pub fn pop(repo_path: &Path) -> Result<()> {
    let repo = Repository::open(repo_path)?;
    let c = Color::new();
    let stash_dir = repo.rvcs_dir.join("refs").join("stash");

    if !stash_dir.exists() {
        return Err(RvcsError::Other("No stashes to pop".to_string()));
    }

    let mut stashes: Vec<_> = fs::read_dir(&stash_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    stashes.sort();

    if stashes.is_empty() {
        return Err(RvcsError::Other("No stashes to pop".to_string()));
    }

    let latest = &stashes[stashes.len() - 1];
    let stash_name = latest.file_name().unwrap().to_str().unwrap().to_string();

    let index_content = fs::read_to_string(latest.join("index"))?;

    for line in index_content.lines() {
        if let Some(entry_info) = line.strip_prefix("entry:") {
            if let Some((rel_path, blob_hash)) = entry_info.split_once(':') {
                let blob_path = latest.join(blob_hash);
                let path = Path::new(rel_path);
                let content = fs::read(&blob_path)?;
                let full_path = repo.root.join(path);
                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&full_path, &content)?;
            }
        }
    }

    fs::remove_dir_all(latest)?;
    println!("{} Restored {} (removed)", c.green("✓"), c.bold(&stash_name));
    Ok(())
}

pub fn drop_stash(repo_path: &Path, name: &str) -> Result<()> {
    let c = Color::new();
    let repo = Repository::open(repo_path)?;
    let stash_dir = repo.rvcs_dir.join("refs").join("stash");

    if !stash_dir.exists() {
        return Err(RvcsError::Other("No stashes found".to_string()));
    }

    let stash_path = stash_dir.join(name);
    if !stash_path.exists() {
        return Err(RvcsError::Other(format!("Stash '{}' not found", name)));
    }

    fs::remove_dir_all(&stash_path)?;
    println!("{} Dropped {} {}", c.red("✓"), c.bold(name), c.yellow("(removed)"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{add, init};
    use std::fs;

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        init::execute(&path).unwrap();
        (tmp, path)
    }

    #[test]
    fn test_stash_no_changes() {
        let (_tmp, path) = setup();
        let result = push(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stash_push_list() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();

        push(&path).unwrap();

        let repo = Repository::open(&path).unwrap();
        assert!(repo.index.is_empty());
    }

    #[test]
    fn test_stash_pop_empty() {
        let (_tmp, path) = setup();
        assert!(pop(&path).is_err());
    }

    #[test]
    fn test_drop_nonexistent_stash() {
        let (_tmp, path) = setup();
        assert!(drop_stash(&path, "stash@{0}").is_err());
    }
}

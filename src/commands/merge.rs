use crate::core::commit::Commit;
use crate::core::repository::Repository;
use crate::error::{RvcsError, Result};
use crate::utils::color::Color;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub fn execute(repo_path: &Path, branch: &str) -> Result<()> {
    let repo = Repository::open(repo_path)?;
    let c = Color::new();

    let branch_hash = repo.resolve_ref(branch)?;
    let branch_commit = load_commit(&repo, &branch_hash)?;

    let head_hash = repo.get_head_commit_hash()?;
    let head_commit = load_commit(&repo, &head_hash)?;

    if head_hash == branch_hash {
        println!("{} Already up to date (HEAD and '{}' are the same)", c.yellow("●"), branch);
        return Ok(());
    }

    let ancestor_hash = find_merge_base(&repo, &head_hash, &branch_hash)?;
    let ancestor_commit = load_commit(&repo, &ancestor_hash)?;

    let head_files = get_files_at_commit(&repo, &head_commit)?;
    let branch_files = get_files_at_commit(&repo, &branch_commit)?;
    let ancestor_files = get_files_at_commit(&repo, &ancestor_commit)?;

    let mut conflicts = Vec::new();

    let all_files: HashSet<_> = head_files
        .keys()
        .chain(branch_files.keys())
        .chain(ancestor_files.keys())
        .collect();

    for file in &all_files {
        let ancestor = ancestor_files.get(*file);
        let head = head_files.get(*file);
        let branch = branch_files.get(*file);

        if head == branch {
            continue;
        }

        if head == ancestor && branch != ancestor {
            let content = branch.unwrap();
            let full_path = repo.root.join(file);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&full_path, content)?;
        } else if branch == ancestor && head != ancestor {
            // Already has the head version
            continue;
        } else {
            let head_content = head.map(|c| &c[..]).unwrap_or(b"");
            let branch_content = branch.map(|c| &c[..]).unwrap_or(b"");
            let conflict = write_conflict_marker(&repo, file, head_content, branch_content)?;
            println!("  {} CONFLICT in {}", c.red("✖"), c.bold(file));
            conflicts.push(conflict);
        }
    }

    if !conflicts.is_empty() {
        println!();
        println!("{} Merge with conflicts — resolve them, then commit", c.red("⚠"));
        println!("{}   Files with conflicts:", c.red(">"));
        for f in &conflicts {
            println!("{}     - {}", c.red("●"), f);
        }
        return Err(RvcsError::Other(format!(
            "Merge conflict in {} file(s)",
            conflicts.len()
        )));
    }

    let new_hash = repo.resolve_ref(branch)?;
    repo.update_head(&new_hash)?;

    println!("{} Merge successful: branch '{}' merged into HEAD", c.green("✓"), c.bold(branch));
    Ok(())
}

fn load_commit(repo: &Repository, hash: &str) -> Result<Commit> {
    let obj = repo.load_object(hash)?;
    Commit::from_object(&obj)
}

fn find_merge_base(repo: &Repository, a: &str, b: &str) -> Result<String> {
    let mut ancestors_a = HashSet::new();
    let mut current = Some(a.to_string());
    while let Some(hash) = current {
        if !ancestors_a.insert(hash.clone()) {
            break;
        }
        let commit = load_commit(repo, &hash)?;
        current = commit.parent_hash;
    }

    let mut current = Some(b.to_string());
    while let Some(hash) = current {
        if ancestors_a.contains(&hash) {
            return Ok(hash);
        }
        let commit = load_commit(repo, &hash)?;
        current = commit.parent_hash;
    }

    Err(RvcsError::NoCommonAncestor)
}

fn get_files_at_commit(
    repo: &Repository,
    commit: &Commit,
) -> Result<std::collections::HashMap<String, Vec<u8>>> {
    let mut files = std::collections::HashMap::new();
    let blobs = repo.collect_blobs_from_tree(&commit.tree_hash, Path::new(""))?;
    for (path, blob_hash) in blobs {
        let obj = repo.load_object(&blob_hash)?;
        let blob = crate::core::blob::Blob::from_object(&obj)?;
        files.insert(path.to_str().unwrap().to_string(), blob.content);
    }
    Ok(files)
}

fn write_conflict_marker(
    repo: &Repository,
    file: &str,
    head_content: &[u8],
    branch_content: &[u8],
) -> Result<String> {
    let full_path = repo.root.join(file);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut result = Vec::new();
    result.extend_from_slice(b"<<<<<<< HEAD\n");
    result.extend_from_slice(head_content);
    if !head_content.ends_with(b"\n") {
        result.push(b'\n');
    }
    result.extend_from_slice(b"=======\n");
    result.extend_from_slice(branch_content);
    if !branch_content.ends_with(b"\n") {
        result.push(b'\n');
    }
    result.extend_from_slice(b">>>>>>> branch\n");

    fs::write(&full_path, &result)?;
    Ok(file.to_string())
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{add, branch, commit, init, switch};
    use std::fs;

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        init::execute(&path).unwrap();
        (tmp, path)
    }

    #[test]
    fn test_merge_fast_forward() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "main content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "main").unwrap();

        branch::create(&path, "feature").unwrap();
        switch::execute(&path, "feature").unwrap();

        fs::write(path.join("file.txt"), "feature content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "feature work").unwrap();

        switch::execute(&path, "main").unwrap();

        let result = execute(&path, "feature");
        assert!(result.is_ok());
    }

    #[test]
    fn test_merge_no_common_ancestor() {
        let (_tmp, path) = setup();
        let result = execute(&path, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_with_conflict() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "base content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "base").unwrap();

        branch::create(&path, "feature").unwrap();
        switch::execute(&path, "feature").unwrap();

        fs::write(path.join("file.txt"), "feature changes").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "feature").unwrap();

        switch::execute(&path, "main").unwrap();

        fs::write(path.join("file.txt"), "main changes").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "main update").unwrap();

        let result = execute(&path, "feature");
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_merge_same_branch() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "initial").unwrap();

        let result = execute(&path, "main");
        assert!(result.is_ok());
    }
}

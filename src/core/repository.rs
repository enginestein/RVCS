use crate::core::blob::Blob;
use crate::core::commit::Commit;
use crate::core::index::Index;
use crate::core::object::{ObjectType, StoredObject};
use crate::core::tree::{Tree, TreeEntry, TreeEntryType};
use crate::error::{RvcsError, Result};
use crate::utils::helpers::is_ignored;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Repository {
    pub root: PathBuf,
    pub rvcs_dir: PathBuf,
    pub index: Index,
}

impl Repository {
    pub fn init(path: &Path) -> Result<Self> {
        let rvcs_dir = path.join(".rvcs");
        if rvcs_dir.exists() {
            return Err(RvcsError::RepositoryExists(path.to_path_buf()));
        }

        fs::create_dir_all(&rvcs_dir)?;
        fs::create_dir_all(rvcs_dir.join("objects"))?;
        fs::create_dir_all(rvcs_dir.join("refs"))?;

        let index = Index::new();
        index.save(path)?;

        fs::write(rvcs_dir.join("HEAD"), "ref: refs/main\n")?;

        Ok(Self {
            root: path.to_path_buf(),
            rvcs_dir,
            index,
        })
    }

    pub fn open(path: &Path) -> Result<Self> {
        let root = crate::utils::helpers::find_repo_root(path)?;
        let rvcs_dir = root.join(".rvcs");
        let index = Index::load(&root)?;

        crate::utils::helpers::load_ignore_rules(&root);

        Ok(Self {
            root,
            rvcs_dir,
            index,
        })
    }

    pub fn is_initialized(&self) -> bool {
        self.rvcs_dir.exists()
            && self.rvcs_dir.join("objects").is_dir()
            && self.rvcs_dir.join("refs").is_dir()
    }

    pub fn store_object(&self, obj: &StoredObject) -> Result<String> {
        let serialized = obj.serialize();
        let compressed = crate::core::object::compress(&serialized)?;

        let hash = match obj.obj_type {
            ObjectType::Blob => crate::utils::hash::hash_blob(&obj.content),
            ObjectType::Tree => crate::utils::hash::hash_tree(&obj.content),
            ObjectType::Commit => crate::utils::hash::hash_commit(&obj.content),
        };

        let dir = self.rvcs_dir.join("objects").join(&hash[..2]);
        fs::create_dir_all(&dir)?;

        let obj_path = dir.join(&hash[2..]);
        if !obj_path.exists() {
            fs::write(&obj_path, &compressed)?;
        }

        Ok(hash)
    }

    pub fn load_object(&self, hash: &str) -> Result<StoredObject> {
        if hash.len() < 2 {
            return Err(RvcsError::InvalidHash(hash.to_string()));
        }
        let obj_path = self
            .rvcs_dir
            .join("objects")
            .join(&hash[..2])
            .join(&hash[2..]);

        if !obj_path.exists() {
            return Err(RvcsError::InvalidHash(hash.to_string()));
        }

        let compressed = fs::read(&obj_path)?;
        let data = crate::core::object::decompress(&compressed)?;
        StoredObject::deserialize(&data)
    }

    pub fn save_index(&mut self) -> Result<()> {
        self.index.save(&self.root)
    }

    pub fn add_file(&mut self, path: &Path) -> Result<()> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };

        if !full_path.exists() {
            return Err(RvcsError::FileNotFound(full_path));
        }

        let blob = Blob::from_file(&full_path)?;
        let relative = crate::utils::helpers::relative_path(&self.root, &full_path)?;
        let metadata = fs::metadata(&full_path)?;

        self.store_object(&blob.to_object())?;
        self.index
            .add_entry(relative, blob.hash, metadata.len());
        Ok(())
    }

    pub fn remove_from_staging(&mut self, path: &Path) {
        self.index.remove_entry(path);
    }

    pub fn build_tree(&self) -> Result<Tree> {
        // Group index entries by their immediate parent directory
        let mut dir_entries: std::collections::HashMap<String, Vec<TreeEntry>> =
            std::collections::HashMap::new();

        for (path, entry) in self.index.entries_sorted() {
            let components: Vec<_> = path.components().collect();
            let (parent_dir, file_name) = if components.len() <= 1 {
                ("".to_string(), path.to_str().unwrap().to_string())
            } else {
                let parent: PathBuf = components[..components.len() - 1].iter().collect();
                let name = components.last().unwrap().as_os_str().to_str().unwrap().to_string();
                (parent.to_str().unwrap().to_string(), name)
            };

            dir_entries
                .entry(parent_dir)
                .or_default()
                .push(TreeEntry {
                    name: file_name,
                    hash: entry.hash.clone(),
                    entry_type: TreeEntryType::Blob,
                });
        }

        self.build_recursive_tree(&Path::new(""), &dir_entries)
    }

    fn build_recursive_tree(
        &self,
        dir: &Path,
        dir_entries: &std::collections::HashMap<String, Vec<TreeEntry>>,
    ) -> Result<Tree> {
        let dir_key = if dir == Path::new("") {
            "".to_string()
        } else {
            dir.to_str().unwrap().to_string()
        };

        let mut entries = dir_entries.get(&dir_key).cloned().unwrap_or_default();
        let dir_prefix = if dir_key.is_empty() {
            String::new()
        } else {
            format!("{}/", dir_key)
        };

        // Find subdirectories
        let mut subdirs = std::collections::BTreeSet::new();
        for key in dir_entries.keys() {
            if key.is_empty() || key == &dir_key {
                continue;
            }
            if let Some(relative) = key.strip_prefix(&dir_prefix) {
                if let Some(subdir) = relative.split('/').next() {
                    if !subdir.is_empty() {
                        subdirs.insert(subdir.to_string());
                    }
                }
            }
        }

        for subdir in subdirs {
            let child_path = Path::new(&dir_prefix).join(&subdir);
            let child_tree = self.build_recursive_tree(&child_path, dir_entries)?;
            self.store_object(&child_tree.to_object())?;
            entries.push(TreeEntry {
                name: subdir,
                hash: child_tree.hash.clone(),
                entry_type: TreeEntryType::Tree,
            });
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Tree::new(entries))
    }

    pub fn commit_staged(&mut self, author: &str, message: &str) -> Result<Commit> {
        if self.index.is_empty() {
            return Err(RvcsError::NothingStaged);
        }

        let tree = self.build_tree()?;
        self.store_object(&tree.to_object())?;

        let parent_hash = self.get_head_commit_hash().ok();

        let commit = Commit::new(tree.hash.clone(), parent_hash, author.to_string(), message.to_string());
        self.store_object(&commit.to_object())?;

        self.update_head(&commit.hash)?;
        self.index.clear();
        self.save_index()?;

        Ok(commit)
    }

    pub fn get_head_commit_hash(&self) -> Result<String> {
        let head_path = self.rvcs_dir.join("HEAD");
        let head_content = fs::read_to_string(&head_path)?;

        if let Some(ref_path) = head_content.strip_prefix("ref: ") {
            let ref_path = self.rvcs_dir.join(ref_path.trim());
            if ref_path.exists() {
                let hash = fs::read_to_string(&ref_path)?;
                Ok(hash.trim().to_string())
            } else {
                Err(RvcsError::NoCommitsYet)
            }
        } else {
            Ok(head_content.trim().to_string())
        }
    }

    pub fn get_head_commit(&self) -> Result<Commit> {
        let hash = self.get_head_commit_hash()?;
        let obj = self.load_object(&hash)?;
        Commit::from_object(&obj)
    }

    pub fn update_head(&self, commit_hash: &str) -> Result<()> {
        let head_path = self.rvcs_dir.join("HEAD");
        let head_content = fs::read_to_string(&head_path)?;

        if let Some(ref_name) = head_content.strip_prefix("ref: ") {
            let ref_path = self.rvcs_dir.join(ref_name.trim());
            if let Some(parent) = ref_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&ref_path, format!("{}\n", commit_hash))?;
        }
        Ok(())
    }

    pub fn get_commit_history(&self) -> Result<Vec<Commit>> {
        let mut commits = Vec::new();

        match self.get_head_commit_hash() {
            Ok(mut current_hash) => {
                loop {
                    let obj = self.load_object(&current_hash)?;
                    let commit = Commit::from_object(&obj)?;
                    let parent = commit.parent_hash.clone();
                    commits.push(commit);

                    match parent {
                        Some(p) => current_hash = p,
                        None => break,
                    }
                }
            }
            Err(_) => {}
        }

        Ok(commits)
    }

    pub fn get_tree_entry_at_path(&self, tree_hash: &str, path: &Path) -> Result<TreeEntry> {
        let components: Vec<_> = path.components().collect();
        if components.is_empty() {
            return Err(RvcsError::FileNotFound(path.to_path_buf()));
        }

        let obj = self.load_object(tree_hash)?;
        let tree = Tree::from_object(&obj)?;

        let name = components[0].as_os_str().to_str().unwrap();
        let entry = tree
            .entries
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| RvcsError::FileNotFound(path.to_path_buf()))?;

        if components.len() == 1 {
            return Ok(entry.clone());
        }

        // Recurse into subtree
        if entry.entry_type != TreeEntryType::Tree {
            return Err(RvcsError::FileNotFound(path.to_path_buf()));
        }

        let remaining: PathBuf = components[1..].iter().collect();
        self.get_tree_entry_at_path(&entry.hash, &remaining)
    }

    pub fn get_file_content_at_commit(&self, commit: &Commit, path: &Path) -> Result<Vec<u8>> {
        let entry = self.get_tree_entry_at_path(&commit.tree_hash, path)?;

        if entry.entry_type != TreeEntryType::Blob {
            return Err(RvcsError::FileNotFound(path.to_path_buf()));
        }

        let blob_obj = self.load_object(&entry.hash)?;
        let blob = Blob::from_object(&blob_obj)?;
        Ok(blob.content)
    }

    pub fn get_working_tree_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_files(&self.root, &mut files)?;
        Ok(files)
    }

    fn collect_files(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.starts_with(&self.rvcs_dir) {
                    continue;
                }

                if is_ignored(&path) {
                    continue;
                }

                if path.is_dir() {
                    self.collect_files(&path, files)?;
                } else {
                    let relative = crate::utils::helpers::relative_path(&self.root, &path)?;
                    files.push(relative);
                }
            }
        }
        Ok(())
    }

    pub fn get_file_status(&self, path: &Path) -> Result<FileStatus> {
        let full_path = self.root.join(path);

        if !full_path.exists() {
            return Ok(FileStatus::Deleted);
        }

        let current_blob = Blob::from_file(&full_path)?;

        match self.index.get_entry(path) {
            Some(entry) => {
                if entry.hash == current_blob.hash {
                    Ok(FileStatus::Unchanged)
                } else {
                    Ok(FileStatus::Modified)
                }
            }
            None => Ok(FileStatus::New),
        }
    }

    pub fn compute_diff(&self, path: &Path) -> Result<DiffResult> {
        let full_path = self.root.join(path);

        let current_content = if full_path.exists() {
            fs::read(&full_path)?
        } else {
            Vec::new()
        };

        let old_content = match self.get_head_commit() {
            Ok(commit) => match self.get_file_content_at_commit(&commit, path) {
                Ok(content) => content,
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        };

        Ok(compute_line_diff(&old_content, &current_content))
    }

    pub fn compute_cached_diff(&self, path: &Path) -> Result<DiffResult> {
        let staged_content = match self.index.get_entry(path) {
            Some(entry) => {
                let obj = self.load_object(&entry.hash)?;
                let blob = Blob::from_object(&obj)?;
                blob.content
            }
            None => Vec::new(),
        };

        let old_content = match self.get_head_commit() {
            Ok(commit) => match self.get_file_content_at_commit(&commit, path) {
                Ok(content) => content,
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        };

        Ok(compute_line_diff(&old_content, &staged_content))
    }

    pub fn compute_cached_diff_for_index(&self) -> Result<Vec<(PathBuf, DiffResult)>> {
        let mut results = Vec::new();
        for (path, entry) in self.index.entries_sorted() {
            let staged_content = {
                let obj = self.load_object(&entry.hash)?;
                let blob = Blob::from_object(&obj)?;
                blob.content
            };

            let old_content = match self.get_head_commit() {
                Ok(ref commit) => match self.get_file_content_at_commit(commit, path) {
                    Ok(content) => content,
                    Err(_) => Vec::new(),
                },
                Err(_) => Vec::new(),
            };

            let diff = compute_line_diff(&old_content, &staged_content);
            if diff.additions > 0 || diff.deletions > 0 {
                results.push((path.clone(), diff));
            }
        }
        Ok(results)
    }

    pub fn revert_file(&mut self, path: &Path) -> Result<()> {
        let commit = self.get_head_commit()?;
        let content = self.get_file_content_at_commit(&commit, path)?;
        let full_path = self.root.join(path);

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&full_path, &content)?;

        self.add_file(path)?;

        Ok(())
    }

    pub fn revert_all(&mut self) -> Result<()> {
        let commit = self.get_head_commit()?;
        self.revert_all_at_commit(&commit)?;

        self.index.clear();
        self.save_index()?;

        Ok(())
    }

    pub fn checkout_commit(&mut self, commit_hash: &str) -> Result<()> {
        let obj = self.load_object(commit_hash)?;
        let commit = Commit::from_object(&obj)?;

        self.revert_all_at_commit(&commit)?;
        self.update_head(commit_hash)?;

        self.index.clear();
        self.save_index()?;

        Ok(())
    }

    pub fn collect_blobs_from_tree(&self, tree_hash: &str, prefix: &Path) -> Result<Vec<(PathBuf, String)>> {
        let obj = self.load_object(tree_hash)?;
        let tree = Tree::from_object(&obj)?;
        let mut result = Vec::new();

        for entry in &tree.entries {
            let path = prefix.join(&entry.name);
            match entry.entry_type {
                TreeEntryType::Blob => {
                    result.push((path, entry.hash.clone()));
                }
                TreeEntryType::Tree => {
                    let sub = self.collect_blobs_from_tree(&entry.hash, &path)?;
                    result.extend(sub);
                }
            }
        }

        Ok(result)
    }

    pub fn revert_all_at_commit(&self, commit: &Commit) -> Result<()> {
        let blobs = self.collect_blobs_from_tree(&commit.tree_hash, Path::new(""))?;

        for file in self.get_working_tree_files()? {
            let in_tree = blobs.iter().any(|(p, _)| p == &file);
            if !in_tree {
                let _ = fs::remove_file(self.root.join(&file));
            }
        }

        for (path, blob_hash) in &blobs {
            let blob_obj = self.load_object(blob_hash)?;
            let blob = Blob::from_object(&blob_obj)?;
            let file_path = self.root.join(path);

            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(&file_path, &blob.content)?;
        }

        Ok(())
    }

    // --- Branch operations ---

    pub fn get_current_branch(&self) -> Option<String> {
        let head_content = fs::read_to_string(self.rvcs_dir.join("HEAD")).ok()?;
        head_content
            .strip_prefix("ref: refs/")
            .map(|s| s.trim().to_string())
    }

    pub fn create_branch(&self, name: &str) -> Result<()> {
        if name.contains('/') || name.contains(' ') || name.starts_with('-') {
            return Err(RvcsError::BranchNotFound(format!(
                "Invalid branch name: '{}'",
                name
            )));
        }

        let refs_dir = self.rvcs_dir.join("refs");
        let branch_path = refs_dir.join(name);

        if branch_path.exists() {
            return Err(RvcsError::BranchExists(name.to_string()));
        }

        let head_hash = self.get_head_commit_hash()?;
        fs::write(&branch_path, format!("{}\n", head_hash))?;
        Ok(())
    }

    pub fn list_branches(&self) -> Result<Vec<(String, bool)>> {
        let refs_dir = self.rvcs_dir.join("refs");
        let current = self.get_current_branch();
        let mut branches = Vec::new();

        if refs_dir.exists() {
            for entry in fs::read_dir(&refs_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    let name = path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string();
                    let is_current = current.as_deref() == Some(&name);
                    branches.push((name, is_current));
                }
            }
        }

        branches.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(branches)
    }

    pub fn switch_branch(&mut self, name: &str) -> Result<()> {
        let refs_dir = self.rvcs_dir.join("refs");
        let branch_path = refs_dir.join(name);

        if !branch_path.exists() {
            return Err(RvcsError::BranchNotFound(name.to_string()));
        }

        let head_path = self.rvcs_dir.join("HEAD");
        fs::write(&head_path, format!("ref: refs/{}\n", name))?;

        // Checkout the branch's commit
        let commit_hash = self.get_head_commit_hash()?;
        let obj = self.load_object(&commit_hash)?;
        let commit = Commit::from_object(&obj)?;
        self.revert_all_at_commit(&commit)?;

        self.index.clear();
        self.save_index()?;

        Ok(())
    }

    pub fn delete_branch(&self, name: &str) -> Result<()> {
        let current = self.get_current_branch();
        if current.as_deref() == Some(name) {
            return Err(RvcsError::CannotDeleteCurrentBranch(name.to_string()));
        }

        let refs_dir = self.rvcs_dir.join("refs");
        let branch_path = refs_dir.join(name);

        if !branch_path.exists() {
            return Err(RvcsError::BranchNotFound(name.to_string()));
        }

        fs::remove_file(&branch_path)?;
        Ok(())
    }

    pub fn resolve_ref(&self, name_or_hash: &str) -> Result<String> {
        // First try as a branch name
        let branch_path = self.rvcs_dir.join("refs").join(name_or_hash);
        if branch_path.exists() {
            let hash = fs::read_to_string(&branch_path)?;
            return Ok(hash.trim().to_string());
        }

        // Try as a commit hash
        if name_or_hash.len() >= 4 {
            let objects_dir = self.rvcs_dir.join("objects");
            // Try to find a matching object by prefix
            if let Ok(entries) = fs::read_dir(&objects_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        let prefix = entry.file_name();
                        if let Ok(sub_entries) = fs::read_dir(entry.path()) {
                            for sub in sub_entries.flatten() {
                                let full_hash =
                                    format!("{}{}", prefix.to_str().unwrap(), sub.file_name().to_str().unwrap());
                                if full_hash.starts_with(name_or_hash) {
                                    return Ok(full_hash);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Try as exact hash
        if name_or_hash.len() == 40 {
            let obj_path = self
                .rvcs_dir
                .join("objects")
                .join(&name_or_hash[..2])
                .join(&name_or_hash[2..]);
            if obj_path.exists() {
                return Ok(name_or_hash.to_string());
            }
        }

        Err(RvcsError::BranchNotFound(name_or_hash.to_string()))
    }

    pub fn revert_from_branch(&mut self, branch_name: &str) -> Result<()> {
        let commit_hash = self.resolve_ref(branch_name)?;
        let obj = self.load_object(&commit_hash)?;
        let commit = Commit::from_object(&obj)?;
        self.revert_all_at_commit(&commit)?;

        self.index.clear();
        self.save_index()?;

        Ok(())
    }

    pub fn reset_soft(&mut self, target: &str) -> Result<()> {
        let commit_hash = self.resolve_ref(target)?;
        // Verify the commit exists
        let obj = self.load_object(&commit_hash)?;
        let _commit = Commit::from_object(&obj)?;

        // Move HEAD without touching index or working tree
        self.update_head(&commit_hash)?;
        Ok(())
    }

    pub fn reset_hard(&mut self, target: &str) -> Result<()> {
        let commit_hash = self.resolve_ref(target)?;
        let obj = self.load_object(&commit_hash)?;
        let commit = Commit::from_object(&obj)?;

        // Restore working tree to the target commit
        self.revert_all_at_commit(&commit)?;

        // Clear index
        self.index.clear();
        self.save_index()?;

        // Move HEAD
        self.update_head(&commit_hash)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileStatus {
    New,
    Modified,
    Deleted,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffResult {
    pub lines: Vec<DiffLine>,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub content: String,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffLineType {
    Added,
    Removed,
    Context,
}

pub fn compute_line_diff(old: &[u8], new: &[u8]) -> DiffResult {
    let old_lines: Vec<&[u8]> = old.split(|&b| b == b'\n').collect();
    let new_lines: Vec<&[u8]> = new.split(|&b| b == b'\n').collect();

    let mut lines = Vec::new();
    let mut additions = 0;
    let mut deletions = 0;

    let lcs = lcs_table(&old_lines, &new_lines);
    let mut diff_lines = backtrack_lcs(&lcs, &old_lines, &new_lines, old_lines.len(), new_lines.len());

    for line in &mut diff_lines {
        match line.line_type {
            DiffLineType::Added => additions += 1,
            DiffLineType::Removed => deletions += 1,
            DiffLineType::Context => {}
        }
    }

    lines.extend(diff_lines);

    DiffResult {
        lines,
        additions,
        deletions,
    }
}

fn lcs_table(a: &[&[u8]], b: &[&[u8]]) -> Vec<Vec<usize>> {
    let m = a.len();
    let n = b.len();
    let mut table = vec![vec![0usize; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                table[i][j] = table[i - 1][j - 1] + 1;
            } else {
                table[i][j] = table[i - 1][j].max(table[i][j - 1]);
            }
        }
    }

    table
}

fn backtrack_lcs(
    table: &[Vec<usize>],
    a: &[&[u8]],
    b: &[&[u8]],
    i: usize,
    j: usize,
) -> Vec<DiffLine> {
    let mut result = Vec::new();

    let mut stack: Vec<(usize, usize)> = Vec::new();
    stack.push((i, j));

    while let Some((ci, cj)) = stack.pop() {
        if ci == 0 && cj == 0 {
            continue;
        }

        if ci > 0 && cj > 0 && a[ci - 1] == b[cj - 1] {
            result.push(DiffLine {
                line_type: DiffLineType::Context,
                content: String::from_utf8_lossy(a[ci - 1]).to_string(),
                old_line: Some(ci),
                new_line: Some(cj),
            });
            stack.push((ci - 1, cj - 1));
        } else if cj > 0 && (ci == 0 || table[ci][cj - 1] >= table[ci - 1][cj]) {
            result.push(DiffLine {
                line_type: DiffLineType::Added,
                content: String::from_utf8_lossy(b[cj - 1]).to_string(),
                old_line: None,
                new_line: Some(cj),
            });
            stack.push((ci, cj - 1));
        } else if ci > 0 {
            result.push(DiffLine {
                line_type: DiffLineType::Removed,
                content: String::from_utf8_lossy(a[ci - 1]).to_string(),
                old_line: Some(ci),
                new_line: None,
            });
            stack.push((ci - 1, cj));
        }
    }

    result.reverse();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_repo() -> (tempfile::TempDir, PathBuf, Repository) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        let repo = Repository::init(&path).unwrap();
        (tmp, path, repo)
    }

    fn create_file(root: &Path, name: &str, content: &str) {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
    }

    #[test]
    fn test_repo_init() {
        let (_tmp, path, repo) = setup_repo();
        assert!(repo.is_initialized());
        assert!(path.join(".rvcs").is_dir());
        assert!(path.join(".rvcs/objects").is_dir());
        assert!(path.join(".rvcs/refs").is_dir());
    }

    #[test]
    fn test_repo_init_already_exists() {
        let tmp = tempfile::tempdir().unwrap();
        Repository::init(tmp.path()).unwrap();
        assert!(Repository::init(tmp.path()).is_err());
    }

    #[test]
    fn test_repo_open() {
        let (_tmp, path, _repo) = setup_repo();
        let opened = Repository::open(&path).unwrap();
        assert!(opened.is_initialized());
    }

    #[test]
    fn test_repo_open_not_repo() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(Repository::open(tmp.path()).is_err());
    }

    #[test]
    fn test_store_load_object() {
        let (_tmp, _path, mut repo) = setup_repo();
        let obj = StoredObject::new(ObjectType::Blob, b"hello".to_vec());
        let hash = repo.store_object(&obj).unwrap();
        let loaded = repo.load_object(&hash).unwrap();
        assert_eq!(loaded.content, b"hello");
    }

    #[test]
    fn test_add_file() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "test.txt", "content");
        repo.add_file(Path::new("test.txt")).unwrap();
        assert!(!repo.index.is_empty());
    }

    #[test]
    fn test_add_file_not_found() {
        let (_tmp, _path, mut repo) = setup_repo();
        assert!(repo.add_file(Path::new("nonexistent.txt")).is_err());
    }

    #[test]
    fn test_commit() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "test.txt", "hello world");
        repo.add_file(Path::new("test.txt")).unwrap();
        let commit = repo.commit_staged("Test Author", "Initial commit").unwrap();
        assert_eq!(commit.message, "Initial commit");
        assert!(commit.parent_hash.is_none());
    }

    #[test]
    fn test_commit_empty_staging() {
        let (_tmp, _path, mut repo) = setup_repo();
        assert!(repo.commit_staged("Author", "msg").is_err());
    }

    #[test]
    fn test_commit_chain() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "file.txt", "v1");
        repo.add_file(Path::new("file.txt")).unwrap();
        let c1 = repo.commit_staged("Author", "first").unwrap();

        create_file(&path, "file.txt", "v2");
        repo.add_file(Path::new("file.txt")).unwrap();
        let c2 = repo.commit_staged("Author", "second").unwrap();

        assert_eq!(c2.parent_hash, Some(c1.hash));
    }

    #[test]
    fn test_get_commit_history() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "a.txt", "first");
        repo.add_file(Path::new("a.txt")).unwrap();
        repo.commit_staged("Author", "first").unwrap();

        create_file(&path, "a.txt", "second");
        repo.add_file(Path::new("a.txt")).unwrap();
        repo.commit_staged("Author", "second").unwrap();

        let history = repo.get_commit_history().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].message, "second");
        assert_eq!(history[1].message, "first");
    }

    #[test]
    fn test_get_file_status() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "test.txt", "content");

        let status = repo.get_file_status(Path::new("test.txt")).unwrap();
        assert_eq!(status, FileStatus::New);

        repo.add_file(Path::new("test.txt")).unwrap();
        let status = repo.get_file_status(Path::new("test.txt")).unwrap();
        assert_eq!(status, FileStatus::Unchanged);

        create_file(&path, "test.txt", "modified");
        let status = repo.get_file_status(Path::new("test.txt")).unwrap();
        assert_eq!(status, FileStatus::Modified);
    }

    #[test]
    fn test_diff() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "file.txt", "line1\nline2\n");
        repo.add_file(Path::new("file.txt")).unwrap();
        repo.commit_staged("Author", "initial").unwrap();

        create_file(&path, "file.txt", "line1\nmodified\nline3\n");
        let diff = repo.compute_diff(Path::new("file.txt")).unwrap();
        assert!(diff.additions > 0 || diff.deletions > 0);
    }

    #[test]
    fn test_revert_file() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "file.txt", "original");
        repo.add_file(Path::new("file.txt")).unwrap();
        repo.commit_staged("Author", "first").unwrap();

        create_file(&path, "file.txt", "modified");
        repo.revert_file(Path::new("file.txt")).unwrap();

        let content = fs::read(path.join("file.txt")).unwrap();
        assert_eq!(content, b"original");
    }

    #[test]
    fn test_revert_all() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "a.txt", "content_a");
        create_file(&path, "b.txt", "content_b");
        repo.add_file(Path::new("a.txt")).unwrap();
        repo.add_file(Path::new("b.txt")).unwrap();
        repo.commit_staged("Author", "initial").unwrap();

        create_file(&path, "a.txt", "modified_a");
        create_file(&path, "c.txt", "new_c");

        repo.revert_all().unwrap();

        assert_eq!(fs::read(path.join("a.txt")).unwrap(), b"content_a");
        assert_eq!(fs::read(path.join("b.txt")).unwrap(), b"content_b");
        assert!(!path.join("c.txt").exists());
    }

    #[test]
    fn test_checkout_commit() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "file.txt", "v1");
        repo.add_file(Path::new("file.txt")).unwrap();
        let c1 = repo.commit_staged("Author", "first").unwrap();

        create_file(&path, "file.txt", "v2");
        repo.add_file(Path::new("file.txt")).unwrap();
        repo.commit_staged("Author", "second").unwrap();

        repo.checkout_commit(&c1.hash).unwrap();
        let content = fs::read(path.join("file.txt")).unwrap();
        assert_eq!(content, b"v1");
    }

    #[test]
    fn test_get_working_tree_files() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "a.txt", "a");
        create_file(&path, "b.txt", "b");
        fs::create_dir_all(path.join("sub")).unwrap();
        create_file(&path, "sub/c.txt", "c");

        let files = repo.get_working_tree_files().unwrap();
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_index_after_commit() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "file.txt", "content");
        repo.add_file(Path::new("file.txt")).unwrap();
        assert!(!repo.index.is_empty());
        repo.commit_staged("Author", "msg").unwrap();
        assert!(repo.index.is_empty());
    }

    #[test]
    fn test_compute_line_diff() {
        let old = b"line1\nline2\nline3";
        let new = b"line1\nmodified\nline3\nline4";
        let diff = compute_line_diff(old, new);
        assert!(diff.additions > 0);
        assert!(diff.deletions > 0);
    }

    #[test]
    fn test_compute_line_diff_identical() {
        let content = b"same content";
        let diff = compute_line_diff(content, content);
        assert_eq!(diff.additions, 0);
        assert_eq!(diff.deletions, 0);
    }

    #[test]
    fn test_compute_line_diff_empty_old() {
        let new = b"new line";
        let diff = compute_line_diff(b"", new);
        assert!(diff.additions > 0);
    }

    #[test]
    fn test_compute_line_diff_empty_new() {
        let old = b"removed line";
        let diff = compute_line_diff(old, b"");
        assert!(diff.deletions > 0);
    }

    #[test]
    fn test_file_status_deleted() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "file.txt", "content");
        repo.add_file(Path::new("file.txt")).unwrap();
        repo.commit_staged("Author", "msg").unwrap();

        fs::remove_file(path.join("file.txt")).unwrap();
        let status = repo.get_file_status(Path::new("file.txt")).unwrap();
        assert_eq!(status, FileStatus::Deleted);
    }

    #[test]
    fn test_remove_from_staging() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "keep.txt", "keep");
        create_file(&path, "remove.txt", "remove");
        repo.add_file(Path::new("keep.txt")).unwrap();
        repo.add_file(Path::new("remove.txt")).unwrap();
        assert_eq!(repo.index.entries.len(), 2);

        repo.remove_from_staging(Path::new("remove.txt"));
        assert_eq!(repo.index.entries.len(), 1);
        assert!(repo.index.get_entry(Path::new("keep.txt")).is_some());
        assert!(repo.index.get_entry(Path::new("remove.txt")).is_none());
    }

    #[test]
    fn test_get_head_hash_detached() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "file.txt", "some content");
        repo.add_file(Path::new("file.txt")).unwrap();
        let c = repo.commit_staged("Author", "msg").unwrap();

        fs::write(repo.rvcs_dir.join("HEAD"), format!("{}\n", c.hash)).unwrap();

        let hash = repo.get_head_commit_hash().unwrap();
        assert_eq!(hash, c.hash);
    }

    #[test]
    fn test_get_current_branch() {
        let (_tmp, path, mut repo) = setup_repo();
        assert_eq!(repo.get_current_branch(), Some("main".to_string()));
    }

    #[test]
    fn test_get_current_branch_detached() {
        let (_tmp, path, mut repo) = setup_repo();
        fs::write(repo.rvcs_dir.join("HEAD"), "abc123\n").unwrap();
        assert_eq!(repo.get_current_branch(), None);
    }

    #[test]
    fn test_resolve_branch_name() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "f.txt", "data");
        repo.add_file(Path::new("f.txt")).unwrap();
        let c = repo.commit_staged("Author", "msg").unwrap();
        repo.create_branch("test-branch").unwrap();

        assert_eq!(repo.resolve_ref("test-branch").unwrap(), c.hash);
    }

    #[test]
    fn test_resolve_short_hash() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "f.txt", "data");
        repo.add_file(Path::new("f.txt")).unwrap();
        let c = repo.commit_staged("Author", "msg").unwrap();

        let short: String = c.hash.chars().take(6).collect();
        let resolved = repo.resolve_ref(&short).unwrap();
        assert_eq!(resolved, c.hash);
    }

    #[test]
    fn test_resolve_ref_not_found() {
        let (_tmp, _path, repo) = setup_repo();
        assert!(repo.resolve_ref("nope").is_err());
    }

    #[test]
    fn test_build_nested_tree() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "a.txt", "a");
        create_file(&path, "dir/b.txt", "b");
        create_file(&path, "dir/sub/c.txt", "c");
        create_file(&path, "d.txt", "d");
        repo.add_file(Path::new("a.txt")).unwrap();
        repo.add_file(Path::new("dir/b.txt")).unwrap();
        repo.add_file(Path::new("dir/sub/c.txt")).unwrap();
        repo.add_file(Path::new("d.txt")).unwrap();

        let tree = repo.build_tree().unwrap();
        assert_eq!(tree.entries.len(), 3);

        let dir_entry = tree.entries.iter().find(|e| e.name == "dir").unwrap();
        assert_eq!(dir_entry.entry_type, TreeEntryType::Tree);

        let dir_tree = Tree::from_object(&repo.load_object(&dir_entry.hash).unwrap()).unwrap();
        assert_eq!(dir_tree.entries.len(), 2);
        let sub_entry = dir_tree.entries.iter().find(|e| e.name == "sub").unwrap();
        assert_eq!(sub_entry.entry_type, TreeEntryType::Tree);

        let sub_tree = Tree::from_object(&repo.load_object(&sub_entry.hash).unwrap()).unwrap();
        assert_eq!(sub_tree.entries.len(), 1);
        assert_eq!(sub_tree.entries[0].name, "c.txt");
    }

    #[test]
    fn test_get_file_content_at_nested_path() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "a/b.txt", "nested content");
        repo.add_file(Path::new("a/b.txt")).unwrap();
        let commit = repo.commit_staged("Author", "msg").unwrap();

        let content = repo.get_file_content_at_commit(&commit, Path::new("a/b.txt")).unwrap();
        assert_eq!(content, b"nested content");
    }

    #[test]
    fn test_reset_soft_does_not_clear_index() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "f.txt", "content");
        repo.add_file(Path::new("f.txt")).unwrap();
        let commit = repo.commit_staged("Author", "msg").unwrap();

        create_file(&path, "g.txt", "staged");
        repo.add_file(Path::new("g.txt")).unwrap();
        assert_eq!(repo.index.entries.len(), 1);

        repo.reset_soft(&commit.hash).unwrap();
        assert_eq!(repo.index.entries.len(), 1);
        let head = repo.get_head_commit_hash().unwrap();
        assert_eq!(head, commit.hash);
    }

    #[test]
    fn test_reset_hard_clears_index() {
        let (_tmp, path, mut repo) = setup_repo();
        create_file(&path, "f.txt", "content");
        repo.add_file(Path::new("f.txt")).unwrap();
        let commit = repo.commit_staged("Author", "msg").unwrap();

        create_file(&path, "g.txt", "staged");
        repo.add_file(Path::new("g.txt")).unwrap();

        repo.reset_hard(&commit.hash).unwrap();
        assert_eq!(repo.index.entries.len(), 0);
    }
}

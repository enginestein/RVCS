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
        let mut entries: Vec<TreeEntry> = self
            .index
            .entries_sorted()
            .into_iter()
            .map(|(_, entry)| TreeEntry {
                name: entry.path.file_name().unwrap().to_str().unwrap().to_string(),
                hash: entry.hash.clone(),
                entry_type: TreeEntryType::Blob,
            })
            .collect();

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

    pub fn get_file_content_at_commit(&self, commit: &Commit, path: &Path) -> Result<Vec<u8>> {
        let tree_obj = self.load_object(&commit.tree_hash)?;
        let tree = Tree::from_object(&tree_obj)?;

        let file_name = path.file_name().unwrap().to_str().unwrap();
        let entry = tree
            .entries
            .iter()
            .find(|e| e.name == file_name)
            .ok_or_else(|| RvcsError::FileNotFound(path.to_path_buf()))?;

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
        let tree_obj = self.load_object(&commit.tree_hash)?;
        let tree = Tree::from_object(&tree_obj)?;

        for entry in &tree.entries {
            if entry.entry_type == TreeEntryType::Blob {
                let blob_obj = self.load_object(&entry.hash)?;
                let blob = Blob::from_object(&blob_obj)?;
                let file_path = self.root.join(&entry.name);

                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                fs::write(&file_path, &blob.content)?;
            }
        }

        // Remove tracked files that no longer exist in the commit
        for file in self.get_working_tree_files()? {
            let in_tree = tree.entries.iter().any(|e| e.name == file.to_str().unwrap());
            if !in_tree {
                let _ = fs::remove_file(self.root.join(&file));
            }
        }

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

    fn revert_all_at_commit(&self, commit: &Commit) -> Result<()> {
        let tree_obj = self.load_object(&commit.tree_hash)?;
        let tree = Tree::from_object(&tree_obj)?;

        for file in self.get_working_tree_files()? {
            let in_tree = tree.entries.iter().any(|e| e.name == file.to_str().unwrap());
            if !in_tree {
                let _ = fs::remove_file(self.root.join(&file));
            }
        }

        for entry in &tree.entries {
            if entry.entry_type == TreeEntryType::Blob {
                let blob_obj = self.load_object(&entry.hash)?;
                let blob = Blob::from_object(&blob_obj)?;
                let file_path = self.root.join(&entry.name);

                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                fs::write(&file_path, &blob.content)?;
            }
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
}

use crate::error::{RvcsError, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct IndexEntry {
    pub path: PathBuf,
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct Index {
    pub entries: HashMap<PathBuf, IndexEntry>,
}

impl Index {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn add_entry(&mut self, path: PathBuf, hash: String, size: u64) {
        self.entries.insert(path.clone(), IndexEntry { path, hash, size });
    }

    pub fn remove_entry(&mut self, path: &Path) {
        self.entries.remove(path);
    }

    pub fn get_entry(&self, path: &Path) -> Option<&IndexEntry> {
        self.entries.get(path)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn entries_sorted(&self) -> Vec<(&PathBuf, &IndexEntry)> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        entries
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = String::new();
        let mut sorted_entries: Vec<_> = self.entries.iter().collect();
        sorted_entries.sort_by(|a, b| a.0.cmp(b.0));

        for (path, entry) in sorted_entries {
            data.push_str(&format!(
                "{}\t{}\t{}\n",
                entry.hash,
                entry.size,
                path.display()
            ));
        }
        data.into_bytes()
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let text = String::from_utf8(data.to_vec())
            .map_err(|e| RvcsError::Encoding(e.to_string()))?;
        let mut entries = HashMap::new();

        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() != 3 {
                return Err(RvcsError::IndexCorrupted);
            }
            let hash = parts[0].to_string();
            let size: u64 = parts[1]
                .parse()
                .map_err(|_| RvcsError::IndexCorrupted)?;
            let path = PathBuf::from(parts[2]);
            entries.insert(
                path.clone(),
                IndexEntry {
                    path,
                    hash,
                    size,
                },
            );
        }

        Ok(Self { entries })
    }

    pub fn save(&self, repo_path: &Path) -> Result<()> {
        let index_path = repo_path.join(".rvcs").join("index");
        let data = self.serialize();
        fs::write(&index_path, data)?;
        Ok(())
    }

    pub fn load(repo_path: &Path) -> Result<Self> {
        let index_path = repo_path.join(".rvcs").join("index");
        if !index_path.exists() {
            return Ok(Self::new());
        }
        let data = fs::read(&index_path)?;
        Self::deserialize(&data)
    }
}

impl Default for Index {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_new_empty() {
        let idx = Index::new();
        assert!(idx.is_empty());
    }

    #[test]
    fn test_index_add_entry() {
        let mut idx = Index::new();
        idx.add_entry(PathBuf::from("file.txt"), "abc123".into(), 100);
        assert!(!idx.is_empty());
        assert!(idx.get_entry(Path::new("file.txt")).is_some());
    }

    #[test]
    fn test_index_remove_entry() {
        let mut idx = Index::new();
        idx.add_entry(PathBuf::from("file.txt"), "abc123".into(), 100);
        idx.remove_entry(Path::new("file.txt"));
        assert!(idx.get_entry(Path::new("file.txt")).is_none());
    }

    #[test]
    fn test_index_clear() {
        let mut idx = Index::new();
        idx.add_entry(PathBuf::from("a.txt"), "aaa".into(), 10);
        idx.add_entry(PathBuf::from("b.txt"), "bbb".into(), 20);
        idx.clear();
        assert!(idx.is_empty());
    }

    #[test]
    fn test_index_serialize_deserialize() {
        let mut idx = Index::new();
        idx.add_entry(PathBuf::from("b.txt"), "bbb".into(), 20);
        idx.add_entry(PathBuf::from("a.txt"), "aaa".into(), 10);

        let serialized = idx.serialize();
        let restored = Index::deserialize(&serialized).unwrap();

        assert_eq!(restored.entries.len(), 2);
        let entry_a = restored.get_entry(Path::new("a.txt")).unwrap();
        assert_eq!(entry_a.hash, "aaa");
        assert_eq!(entry_a.size, 10);
    }

    #[test]
    fn test_index_save_load() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_path = tmp.path();
        fs::create_dir_all(repo_path.join(".rvcs")).unwrap();

        let mut idx = Index::new();
        idx.add_entry(PathBuf::from("test.rs"), "abc".into(), 50);
        idx.save(repo_path).unwrap();

        let loaded = Index::load(repo_path).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.get_entry(Path::new("test.rs")).is_some());
    }

    #[test]
    fn test_index_load_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let idx = Index::load(tmp.path()).unwrap();
        assert!(idx.is_empty());
    }

    #[test]
    fn test_index_entries_sorted() {
        let mut idx = Index::new();
        idx.add_entry(PathBuf::from("z.txt"), "1".into(), 1);
        idx.add_entry(PathBuf::from("a.txt"), "2".into(), 2);
        let sorted = idx.entries_sorted();
        assert_eq!(sorted[0].0, Path::new("a.txt"));
        assert_eq!(sorted[1].0, Path::new("z.txt"));
    }

    #[test]
    fn test_index_overwrite_entry() {
        let mut idx = Index::new();
        idx.add_entry(PathBuf::from("file.txt"), "old".into(), 100);
        idx.add_entry(PathBuf::from("file.txt"), "new".into(), 200);
        let entry = idx.get_entry(Path::new("file.txt")).unwrap();
        assert_eq!(entry.hash, "new");
        assert_eq!(entry.size, 200);
    }

    #[test]
    fn test_index_corrupted_data() {
        assert!(Index::deserialize(b"no tabs here").is_err());
    }
}

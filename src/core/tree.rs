use crate::core::object::{ObjectType, StoredObject};
use crate::error::{RvcsError, Result};
use crate::utils::hash::hash_tree;

#[derive(Debug, Clone, PartialEq)]
pub struct TreeEntry {
    pub name: String,
    pub hash: String,
    pub entry_type: TreeEntryType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TreeEntryType {
    Blob,
    Tree,
}

impl TreeEntryType {
    pub fn as_str(&self) -> &str {
        match self {
            TreeEntryType::Blob => "blob",
            TreeEntryType::Tree => "tree",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Tree {
    pub hash: String,
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    pub fn new(entries: Vec<TreeEntry>) -> Self {
        let raw = Self::serialize_entries(&entries);
        let hash = hash_tree(&raw);
        Self { hash, entries }
    }

    pub fn empty() -> Self {
        Self::new(vec![])
    }

    fn serialize_entries(entries: &[TreeEntry]) -> Vec<u8> {
        let mut data = Vec::new();
        for entry in entries {
            data.extend_from_slice(entry.entry_type.as_str().as_bytes());
            data.push(b' ');
            data.extend_from_slice(entry.hash.as_bytes());
            data.push(b' ');
            data.extend_from_slice(entry.name.as_bytes());
            data.push(b'\n');
        }
        data
    }

    pub fn to_object(&self) -> StoredObject {
        let content = Self::serialize_entries(&self.entries);
        StoredObject::new(ObjectType::Tree, content)
    }

    pub fn from_object(obj: &StoredObject) -> Result<Self> {
        if obj.obj_type != ObjectType::Tree {
            return Err(RvcsError::Encoding(format!(
                "Expected tree, got {}",
                obj.obj_type.as_str()
            )));
        }
        let entries = Self::parse_entries(&obj.content)?;
        let raw = Self::serialize_entries(&entries);
        let hash = hash_tree(&raw);
        Ok(Self { hash, entries })
    }

    fn parse_entries(data: &[u8]) -> Result<Vec<TreeEntry>> {
        let text = String::from_utf8(data.to_vec())
            .map_err(|e| RvcsError::Encoding(e.to_string()))?;
        let mut entries = Vec::new();
        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            if parts.len() != 3 {
                return Err(RvcsError::Encoding(format!("Invalid tree entry: {}", line)));
            }
            let entry_type = match parts[0] {
                "blob" => TreeEntryType::Blob,
                "tree" => TreeEntryType::Tree,
                other => {
                    return Err(RvcsError::Encoding(format!(
                        "Unknown entry type: {}",
                        other
                    )))
                }
            };
            entries.push(TreeEntry {
                name: parts[2].to_string(),
                hash: parts[1].to_string(),
                entry_type,
            });
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_empty() {
        let tree = Tree::empty();
        assert!(tree.entries.is_empty());
    }

    #[test]
    fn test_tree_with_entries() {
        let entries = vec![
            TreeEntry {
                name: "file.txt".to_string(),
                hash: "abc123".repeat(7).chars().take(40).collect::<String>(),
                entry_type: TreeEntryType::Blob,
            },
        ];
        let tree = Tree::new(entries.clone());
        assert_eq!(tree.entries, entries);
        assert_eq!(tree.hash.len(), 40);
    }

    #[test]
    fn test_tree_to_object() {
        let tree = Tree::empty();
        let obj = tree.to_object();
        assert_eq!(obj.obj_type, ObjectType::Tree);
    }

    #[test]
    fn test_tree_from_object() {
        let entries = vec![
            TreeEntry {
                name: "a.txt".to_string(),
                hash: "a".repeat(40),
                entry_type: TreeEntryType::Blob,
            },
            TreeEntry {
                name: "subdir".to_string(),
                hash: "b".repeat(40),
                entry_type: TreeEntryType::Tree,
            },
        ];
        let tree = Tree::new(entries.clone());
        let obj = tree.to_object();
        let restored = Tree::from_object(&obj).unwrap();
        assert_eq!(restored.entries.len(), 2);
        assert_eq!(restored.entries[0].name, "a.txt");
        assert_eq!(restored.entries[1].name, "subdir");
    }

    #[test]
    fn test_tree_from_wrong_object_type() {
        let obj = StoredObject::new(ObjectType::Blob, vec![]);
        assert!(Tree::from_object(&obj).is_err());
    }

    #[test]
    fn test_tree_deterministic_hash() {
        let entries = vec![TreeEntry {
            name: "x.txt".to_string(),
            hash: "c".repeat(40),
            entry_type: TreeEntryType::Blob,
        }];
        let t1 = Tree::new(entries.clone());
        let t2 = Tree::new(entries);
        assert_eq!(t1.hash, t2.hash);
    }

    #[test]
    fn test_tree_entry_type_str() {
        assert_eq!(TreeEntryType::Blob.as_str(), "blob");
        assert_eq!(TreeEntryType::Tree.as_str(), "tree");
    }

    #[test]
    fn test_tree_parse_invalid() {
        let obj = StoredObject::new(ObjectType::Tree, b"invalid entry format".to_vec());
        // Should still parse, just with bad data - one malformed line
        // Actually "invalid entry format" has no spaces after splitting, so it's 1 part
        let result = Tree::from_object(&obj);
        assert!(result.is_err());
    }
}

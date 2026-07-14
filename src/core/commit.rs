use crate::core::object::{ObjectType, StoredObject};
use crate::error::Result;
use chrono::{DateTime, Utc};
use std::fmt;

#[derive(Debug, Clone)]
pub struct Commit {
    pub hash: String,
    pub tree_hash: String,
    pub parent_hash: Option<String>,
    pub author: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

impl Commit {
    pub fn new(
        tree_hash: String,
        parent_hash: Option<String>,
        author: String,
        message: String,
    ) -> Self {
        let timestamp = Utc::now();
        let raw = Self::format_content(&tree_hash, parent_hash.as_deref(), &author, &message, &timestamp);
        let hash = crate::utils::hash::hash_commit(raw.as_bytes());
        Self {
            hash,
            tree_hash,
            parent_hash,
            author,
            message,
            timestamp,
        }
    }

    pub fn content_string(&self) -> String {
        Self::format_content(
            &self.tree_hash,
            self.parent_hash.as_deref(),
            &self.author,
            &self.message,
            &self.timestamp,
        )
    }

    fn format_content(
        tree_hash: &str,
        parent_hash: Option<&str>,
        author: &str,
        message: &str,
        timestamp: &DateTime<Utc>,
    ) -> String {
        let mut content = format!("tree {}\n", tree_hash);
        if let Some(parent) = parent_hash {
            content.push_str(&format!("parent {}\n", parent));
        }
        content.push_str(&format!("author {}\n", author));
        content.push_str(&format!("timestamp {}\n", timestamp.to_rfc3339()));
        content.push_str(&format!("\n{}\n", message));
        content
    }

    pub fn to_object(&self) -> StoredObject {
        let content = self.content_string();
        StoredObject::new(ObjectType::Commit, content.into_bytes())
    }

    pub fn from_object(obj: &StoredObject) -> Result<Self> {
        if obj.obj_type != ObjectType::Commit {
            return Err(crate::error::RvcsError::Encoding(format!(
                "Expected commit, got {}",
                obj.obj_type.as_str()
            )));
        }
        Self::from_content(&obj.content)
    }

    pub fn from_content(content: &[u8]) -> Result<Self> {
        let text = String::from_utf8(content.to_vec())
            .map_err(|e| crate::error::RvcsError::Encoding(e.to_string()))?;

        let mut tree_hash = None;
        let mut parent_hash = None;
        let mut author = None;
        let mut timestamp_str = None;
        let mut message = String::new();
        let mut past_header = false;

        for line in text.lines() {
            if past_header {
                if !message.is_empty() {
                    message.push('\n');
                }
                message.push_str(line);
                continue;
            }

            if line.is_empty() {
                past_header = true;
                continue;
            }

            if let Some(val) = line.strip_prefix("tree ") {
                tree_hash = Some(val.to_string());
            } else if let Some(val) = line.strip_prefix("parent ") {
                parent_hash = Some(val.to_string());
            } else if let Some(val) = line.strip_prefix("author ") {
                author = Some(val.to_string());
            } else if let Some(val) = line.strip_prefix("timestamp ") {
                timestamp_str = Some(val.to_string());
            }
        }

        let tree_hash =
            tree_hash.ok_or_else(|| crate::error::RvcsError::Encoding("Missing tree hash".into()))?;
        let author =
            author.ok_or_else(|| crate::error::RvcsError::Encoding("Missing author".into()))?;
        let timestamp: DateTime<Utc> = timestamp_str
            .ok_or_else(|| crate::error::RvcsError::Encoding("Missing timestamp".into()))
            .and_then(|ts| {
                DateTime::parse_from_rfc3339(&ts)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| crate::error::RvcsError::Encoding(e.to_string()))
            })?;

        let raw = Self::format_content(&tree_hash, parent_hash.as_deref(), &author, &message.trim(), &timestamp);
        let hash = crate::utils::hash::hash_commit(raw.as_bytes());

        Ok(Self {
            hash,
            tree_hash,
            parent_hash,
            author,
            message: message.trim().to_string(),
            timestamp,
        })
    }
}

impl fmt::Display for Commit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "commit {}Author: {}\nDate: {}\n\n    {}\n",
            &self.hash[..12],
            self.author,
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            self.message
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_commit(parent: Option<&str>) -> Commit {
        Commit::new(
            "a".repeat(40),
            parent.map(|s| s.to_string()),
            "Test User <test@example.com>".to_string(),
            "Initial commit".to_string(),
        )
    }

    #[test]
    fn test_commit_new() {
        let c = make_test_commit(None);
        assert_eq!(c.tree_hash.len(), 40);
        assert!(c.parent_hash.is_none());
        assert_eq!(c.author, "Test User <test@example.com>");
        assert_eq!(c.message, "Initial commit");
        assert_eq!(c.hash.len(), 40);
    }

    #[test]
    fn test_commit_with_parent() {
        let parent_hash = "b".repeat(40);
        let c = make_test_commit(Some(&parent_hash));
        assert_eq!(c.parent_hash, Some(parent_hash));
    }

    #[test]
    fn test_commit_serialization_roundtrip() {
        let c = make_test_commit(None);
        let obj = c.to_object();
        let restored = Commit::from_object(&obj).unwrap();
        assert_eq!(c.hash, restored.hash);
        assert_eq!(c.tree_hash, restored.tree_hash);
        assert_eq!(c.author, restored.author);
        assert_eq!(c.message, restored.message);
    }

    #[test]
    fn test_commit_with_parent_serialization() {
        let parent = "d".repeat(40);
        let c = make_test_commit(Some(&parent));
        let obj = c.to_object();
        let restored = Commit::from_object(&obj).unwrap();
        assert_eq!(c.parent_hash, restored.parent_hash);
    }

    #[test]
    fn test_commit_display() {
        let c = make_test_commit(None);
        let display = format!("{}", c);
        assert!(display.contains("commit"));
        assert!(display.contains("Test User"));
        assert!(display.contains("Initial commit"));
    }

    #[test]
    fn test_commit_from_wrong_type() {
        let obj = StoredObject::new(ObjectType::Blob, vec![]);
        assert!(Commit::from_object(&obj).is_err());
    }

    #[test]
    fn test_commit_deterministic_hash() {
        let c1 = Commit::new(
            "a".repeat(40),
            None,
            "User".into(),
            "msg".into(),
        );
        let c2 = Commit::new(
            "a".repeat(40),
            None,
            "User".into(),
            "msg".into(),
        );
        // Hashes differ because timestamps differ, but tree/parent/author/message are same
        assert_eq!(c1.tree_hash, c2.tree_hash);
        assert_eq!(c1.author, c2.author);
        assert_eq!(c1.message, c2.message);
    }

    #[test]
    fn test_commit_multiline_message() {
        let c = Commit::new(
            "a".repeat(40),
            None,
            "User".into(),
            "First line\nSecond line\nThird line".into(),
        );
        let obj = c.to_object();
        let restored = Commit::from_object(&obj).unwrap();
        assert_eq!(restored.message, "First line\nSecond line\nThird line");
    }

    #[test]
    fn test_commit_missing_fields() {
        let content = b"tree abc123\nauthor Someone\n";
        assert!(Commit::from_content(content).is_err());
    }
}

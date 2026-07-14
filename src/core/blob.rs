use crate::core::object::{ObjectType, StoredObject};
use crate::error::Result;
use crate::utils::hash::hash_blob;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Blob {
    pub hash: String,
    pub content: Vec<u8>,
}

impl Blob {
    pub fn from_content(content: Vec<u8>) -> Self {
        let hash = hash_blob(&content);
        Self { hash, content }
    }

    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read(path)?;
        Ok(Self::from_content(content))
    }

    pub fn to_object(&self) -> StoredObject {
        StoredObject::new(ObjectType::Blob, self.content.clone())
    }

    pub fn from_object(obj: &StoredObject) -> Result<Self> {
        if obj.obj_type != ObjectType::Blob {
            return Err(crate::error::RvcsError::Encoding(format!(
                "Expected blob, got {}",
                obj.obj_type.as_str()
            )));
        }
        Ok(Self {
            hash: hash_blob(&obj.content),
            content: obj.content.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_blob_from_content() {
        let blob = Blob::from_content(b"hello".to_vec());
        assert_eq!(blob.content, b"hello");
        assert_eq!(blob.hash.len(), 40);
    }

    #[test]
    fn test_blob_from_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.txt");
        fs::write(&file_path, "file content").unwrap();

        let blob = Blob::from_file(&file_path).unwrap();
        assert_eq!(blob.content, b"file content");
    }

    #[test]
    fn test_blob_to_object() {
        let blob = Blob::from_content(b"test".to_vec());
        let obj = blob.to_object();
        assert_eq!(obj.obj_type, ObjectType::Blob);
        assert_eq!(obj.content, b"test");
    }

    #[test]
    fn test_blob_from_object() {
        let obj = StoredObject::new(ObjectType::Blob, b"test".to_vec());
        let blob = Blob::from_object(&obj).unwrap();
        assert_eq!(blob.content, b"test");
    }

    #[test]
    fn test_blob_from_wrong_object_type() {
        let obj = StoredObject::new(ObjectType::Tree, b"test".to_vec());
        assert!(Blob::from_object(&obj).is_err());
    }

    #[test]
    fn test_blob_deterministic_hash() {
        let b1 = Blob::from_content(b"same".to_vec());
        let b2 = Blob::from_content(b"same".to_vec());
        assert_eq!(b1.hash, b2.hash);
    }

    #[test]
    fn test_blob_different_content_different_hash() {
        let b1 = Blob::from_content(b"one".to_vec());
        let b2 = Blob::from_content(b"two".to_vec());
        assert_ne!(b1.hash, b2.hash);
    }
}

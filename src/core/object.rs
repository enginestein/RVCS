use crate::error::{RvcsError, Result};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectType {
    Blob,
    Tree,
    Commit,
}

impl ObjectType {
    pub fn as_str(&self) -> &str {
        match self {
            ObjectType::Blob => "blob",
            ObjectType::Tree => "tree",
            ObjectType::Commit => "commit",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "blob" => Ok(ObjectType::Blob),
            "tree" => Ok(ObjectType::Tree),
            "commit" => Ok(ObjectType::Commit),
            _ => Err(RvcsError::InvalidHash(format!(
                "Unknown object type: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoredObject {
    pub obj_type: ObjectType,
    pub content: Vec<u8>,
}

impl StoredObject {
    pub fn new(obj_type: ObjectType, content: Vec<u8>) -> Self {
        Self { obj_type, content }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let header = format!("{} {}\0", self.obj_type.as_str(), self.content.len());
        let mut data = header.into_bytes();
        data.extend_from_slice(&self.content);
        data
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let null_pos = data
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| RvcsError::Encoding("Invalid object format: no null byte".into()))?;

        let header = String::from_utf8(data[..null_pos].to_vec())
            .map_err(|e| RvcsError::Encoding(e.to_string()))?;

        let parts: Vec<&str> = header.splitn(2, ' ').collect();
        if parts.len() != 2 {
            return Err(RvcsError::Encoding("Invalid object header".into()));
        }

        let obj_type = ObjectType::from_str(parts[0])?;
        let _size: usize = parts[1]
            .parse()
            .map_err(|e| RvcsError::Encoding(format!("Invalid size: {}", e)))?;

        let content = data[null_pos + 1..].to_vec();
        Ok(StoredObject::new(obj_type, content))
    }
}

pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .map_err(|e| RvcsError::Compression(e.to_string()))?;
    encoder
        .finish()
        .map_err(|e| RvcsError::Compression(e.to_string()))
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| RvcsError::Compression(e.to_string()))?;
    Ok(decompressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_type_conversion() {
        assert_eq!(ObjectType::Blob.as_str(), "blob");
        assert_eq!(ObjectType::Tree.as_str(), "tree");
        assert_eq!(ObjectType::Commit.as_str(), "commit");
    }

    #[test]
    fn test_object_type_from_str() {
        assert!(ObjectType::from_str("blob").is_ok());
        assert!(ObjectType::from_str("tree").is_ok());
        assert!(ObjectType::from_str("commit").is_ok());
        assert!(ObjectType::from_str("unknown").is_err());
    }

    #[test]
    fn test_stored_object_serialize_deserialize() {
        let obj = StoredObject::new(ObjectType::Blob, b"hello world".to_vec());
        let serialized = obj.serialize();
        let deserialized = StoredObject::deserialize(&serialized).unwrap();
        assert_eq!(obj.obj_type, deserialized.obj_type);
        assert_eq!(obj.content, deserialized.content);
    }

    #[test]
    fn test_compress_decompress() {
        let data = b"this is some test data that should compress well because it has repeated patterns repeated patterns repeated patterns";
        let compressed = compress(data).unwrap();
        assert!(compressed.len() < data.len());
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(data.to_vec(), decompressed);
    }

    #[test]
    fn test_compress_decompress_empty() {
        let data = b"";
        let compressed = compress(data).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(data.to_vec(), decompressed);
    }

    #[test]
    fn test_stored_object_serialize_format() {
        let obj = StoredObject::new(ObjectType::Blob, b"test".to_vec());
        let serialized = obj.serialize();
        let header_end = serialized.iter().position(|&b| b == 0).unwrap();
        let header = String::from_utf8(serialized[..header_end].to_vec()).unwrap();
        assert_eq!(header, "blob 4");
    }

    #[test]
    fn test_deserialize_invalid_data() {
        assert!(StoredObject::deserialize(b"").is_err());
        assert!(StoredObject::deserialize(b"no null byte here").is_err());
        assert!(StoredObject::deserialize(b"unknown 4\0data").is_err());
    }
}

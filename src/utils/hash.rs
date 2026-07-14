use sha1::{Digest, Sha1};

pub fn hash_content(content: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(content);
    let result = hasher.finalize();
    hex::encode(result)
}

pub fn hash_blob(content: &[u8]) -> String {
    let header = format!("blob {}\0", content.len());
    let mut hasher = Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(content);
    let result = hasher.finalize();
    hex::encode(result)
}

pub fn hash_tree(content: &[u8]) -> String {
    let header = format!("tree {}\0", content.len());
    let mut hasher = Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(content);
    let result = hasher.finalize();
    hex::encode(result)
}

pub fn hash_commit(content: &[u8]) -> String {
    let header = format!("commit {}\0", content.len());
    let mut hasher = Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(content);
    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_content_deterministic() {
        let data = b"hello world";
        let h1 = hash_content(data);
        let h2 = hash_content(data);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_content_unique() {
        let h1 = hash_content(b"hello");
        let h2 = hash_content(b"world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_blob_format() {
        let content = b"test content";
        let hash = hash_blob(content);
        assert_eq!(hash.len(), 40);
    }

    #[test]
    fn test_hash_tree_format() {
        let content = b"tree content";
        let hash = hash_tree(content);
        assert_eq!(hash.len(), 40);
    }

    #[test]
    fn test_hash_commit_format() {
        let content = b"commit message";
        let hash = hash_commit(content);
        assert_eq!(hash.len(), 40);
    }

    #[test]
    fn test_blob_tree_commit_different_hashes() {
        let data = b"same data";
        let b = hash_blob(data);
        let t = hash_tree(data);
        let c = hash_commit(data);
        assert_ne!(b, t);
        assert_ne!(t, c);
        assert_ne!(b, c);
    }

    #[test]
    fn test_empty_content() {
        let hash = hash_content(b"");
        assert_eq!(hash.len(), 40);
    }
}

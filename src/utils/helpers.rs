use crate::error::{RvcsError, Result};
use std::path::{Path, PathBuf};

pub fn find_repo_root(start: &Path) -> Result<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".rvcs").is_dir() {
            return Ok(current);
        }
        if !current.pop() {
            return Err(RvcsError::NotRepository);
        }
    }
}

pub fn expand_path(path: &str) -> Result<PathBuf> {
    let p = PathBuf::from(shellexpand(path));
    Ok(p)
}

fn shellexpand(path: &str) -> String {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return format!("{}{}", home.display(), &path[1..]);
        }
    }
    path.to_string()
}

pub fn relative_path(base: &Path, path: &Path) -> Result<PathBuf> {
    path.strip_prefix(base)
        .map(|p| p.to_path_buf())
        .map_err(|_| RvcsError::PathError(format!("{} is not under {}", path.display(), base.display())))
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

pub fn is_ignored(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.starts_with('.') || name == "target" || name == "node_modules" || name == "__pycache__"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path(Path::new("a/b/../c")), PathBuf::from("a/c"));
        assert_eq!(normalize_path(Path::new("a/./b")), PathBuf::from("a/b"));
        assert_eq!(
            normalize_path(Path::new("a/b/../../c")),
            PathBuf::from("c")
        );
    }

    #[test]
    fn test_is_ignored() {
        assert!(is_ignored(Path::new(".hidden")));
        assert!(is_ignored(Path::new("target")));
        assert!(is_ignored(Path::new("node_modules")));
        assert!(!is_ignored(Path::new("src")));
        assert!(!is_ignored(Path::new("main.rs")));
    }

    #[test]
    fn test_relative_path() {
        let base = Path::new("/home/user/project");
        let path = Path::new("/home/user/project/src/main.rs");
        let rel = relative_path(base, path).unwrap();
        assert_eq!(rel, PathBuf::from("src/main.rs"));
    }

    #[test]
    fn test_relative_path_error() {
        let base = Path::new("/home/user/project");
        let path = Path::new("/other/path/file.rs");
        assert!(relative_path(base, path).is_err());
    }

    #[test]
    fn test_expand_path_tilde() {
        let expanded = expand_path("~/test").unwrap();
        assert!(!expanded.to_str().unwrap().starts_with('~'));
    }

    #[test]
    fn test_find_repo_root() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_dir = tmp.path().join("myrepo");
        fs::create_dir_all(repo_dir.join(".rvcs")).unwrap();
        fs::create_dir_all(repo_dir.join("src")).unwrap();

        let root = find_repo_root(&repo_dir.join("src")).unwrap();
        assert_eq!(root, repo_dir);
    }

    #[test]
    fn test_find_repo_root_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(find_repo_root(tmp.path()).is_err());
    }
}

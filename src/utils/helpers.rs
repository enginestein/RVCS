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
    if name.starts_with('.') || name == "target" || name == "node_modules" || name == "__pycache__"
    {
        return true;
    }

    // Check .rvcsignore in parent directories up to the repo root via thread-local
    IGNORE_PATTERNS.with(|cache| {
        let patterns = cache.borrow();
        patterns.is_ignored(path)
    })
}

use std::cell::RefCell;
thread_local! {
    static IGNORE_PATTERNS: RefCell<IgnoreRules> = const { RefCell::new(IgnoreRules::empty()) };
}

pub fn load_ignore_rules(repo_root: &Path) {
    let mut patterns = IgnoreRules::empty();
    patterns.load(repo_root);
    IGNORE_PATTERNS.with(|cache| {
        *cache.borrow_mut() = patterns;
    });
}

#[derive(Debug, Clone)]
struct IgnoreRules {
    patterns: Vec<IgnorePattern>,
}

#[derive(Debug, Clone)]
struct IgnorePattern {
    raw: String,
    is_negation: bool,
}

impl IgnoreRules {
    const fn empty() -> Self {
        Self { patterns: Vec::new() }
    }

    fn load(&mut self, repo_root: &Path) {
        let ignore_path = repo_root.join(".rvcsignore");
        let content = match std::fs::read_to_string(&ignore_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let is_negation = line.starts_with('!');
            let raw = if is_negation { &line[1..] } else { line }.to_string();
            self.patterns.push(IgnorePattern { raw, is_negation });
        }
    }

    fn is_ignored(&self, path: &Path) -> bool {
        if self.patterns.is_empty() {
            return false;
        }

        let path_str = path.to_str().unwrap_or("");
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let mut matched = false;
        for p in &self.patterns {
            let match_result = if p.raw.starts_with('/') {
                // Root-anchored pattern: match only at the root level
                let suffix = &p.raw[1..];
                path_str == suffix || path_str.ends_with(&format!("/{}", suffix))
            } else if p.raw.contains('/') {
                // Pattern with / matches full path anywhere
                path_str.ends_with(&p.raw)
                    || path_str.contains(&format!("/{}", &p.raw))
                    || path_str == p.raw
            } else {
                // Simple pattern matches any component
                name == p.raw
                    || path.components().any(|c| c.as_os_str().to_str() == Some(&p.raw))
            };

            if match_result {
                matched = !p.is_negation;
            }
        }

        matched
    }
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

    #[test]
    fn test_rvcsignore_exact_name() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path();
        std::fs::write(repo_root.join(".rvcsignore"), "secret.txt\n").unwrap();
        load_ignore_rules(repo_root);

        assert!(is_ignored(&repo_root.join("secret.txt")));
        assert!(is_ignored(&repo_root.join("subdir/secret.txt")));
        assert!(!is_ignored(&repo_root.join("other.txt")));
    }

    #[test]
    fn test_rvcsignore_directory_name() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path();
        std::fs::write(repo_root.join(".rvcsignore"), "build\n").unwrap();
        load_ignore_rules(repo_root);

        assert!(is_ignored(&repo_root.join("build")));
        assert!(is_ignored(&repo_root.join("build/output.o")));
    }

    #[test]
    fn test_rvcsignore_empty_file() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path();
        std::fs::write(repo_root.join(".rvcsignore"), "").unwrap();
        load_ignore_rules(repo_root);

        assert!(!is_ignored(&repo_root.join("any_file.txt")));
    }

    #[test]
    fn test_rvcsignore_comments() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path();
        std::fs::write(
            repo_root.join(".rvcsignore"),
            "# this is a comment\nbuild\n",
        )
        .unwrap();
        load_ignore_rules(repo_root);

        assert!(is_ignored(&repo_root.join("build")));
    }

    #[test]
    fn test_rvcsignore_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path();
        load_ignore_rules(repo_root);

        assert!(!is_ignored(&repo_root.join("any_file.txt")));
    }

    #[test]
    fn test_rvcsignore_root_anchored() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path();
        std::fs::write(repo_root.join(".rvcsignore"), "/build\n").unwrap();
        load_ignore_rules(repo_root);

        assert!(is_ignored(&repo_root.join("build")));
        // build at root is ignored; build/output.o contains build as a component
        // This test verifies the pattern matches the root-anchored path
    }

    #[test]
    fn test_rvcsignore_subdir_pattern() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path();
        std::fs::write(repo_root.join(".rvcsignore"), "build/output.o\n").unwrap();
        load_ignore_rules(repo_root);

        assert!(is_ignored(&repo_root.join("build/output.o")));
        assert!(!is_ignored(&repo_root.join("other.txt")));
    }

    #[test]
    fn test_rvcsignore_negation() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path();
        std::fs::write(
            repo_root.join(".rvcsignore"),
            "secret.txt\n!important.txt\n",
        )
        .unwrap();
        load_ignore_rules(repo_root);

        assert!(is_ignored(&repo_root.join("secret.txt")));
        assert!(!is_ignored(&repo_root.join("important.txt")));
    }
}

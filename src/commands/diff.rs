use crate::core::repository::{DiffLineType, Repository};
use crate::error::Result;
use std::path::Path;

pub fn execute(repo_path: &Path, file_path: Option<&str>) -> Result<()> {
    let repo = Repository::open(repo_path)?;

    if let Some(path) = file_path {
        show_file_diff(&repo, Path::new(path))?;
    } else {
        let files = repo.get_working_tree_files()?;
        let mut has_diff = false;

        for file in &files {
            let diff = repo.compute_diff(file)?;
            if diff.additions > 0 || diff.deletions > 0 {
                has_diff = true;
                print_file_diff_header(file);
                print_diff_lines(&diff);
            }
        }

        if !has_diff {
            println!("No changes");
        }
    }

    Ok(())
}

fn show_file_diff(repo: &Repository, path: &Path) -> Result<()> {
    let diff = repo.compute_diff(path)?;

    if diff.additions == 0 && diff.deletions == 0 {
        println!("No changes to {}", path.display());
        return Ok(());
    }

    print_file_diff_header(path);
    print_diff_lines(&diff);
    Ok(())
}

fn print_file_diff_header(path: &Path) {
    println!("diff --rvcs/{}", path.display());
}

fn print_diff_lines(diff: &crate::core::repository::DiffResult) {
    for line in &diff.lines {
        match line.line_type {
            DiffLineType::Added => {
                if let Some(n) = line.new_line {
                    println!("+\t{}: {}", n, line.content);
                } else {
                    println!("+\t{}", line.content);
                }
            }
            DiffLineType::Removed => {
                if let Some(n) = line.old_line {
                    println!("-\t{}: {}", n, line.content);
                } else {
                    println!("-\t{}", line.content);
                }
            }
            DiffLineType::Context => {
                if let (Some(o), Some(n)) = (line.old_line, line.new_line) {
                    println!(" \t{}:{}", o, n);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{init, add, commit};
    use std::fs;

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        init::execute(&path).unwrap();
        (tmp, path)
    }

    #[test]
    fn test_diff_no_changes() {
        let (_tmp, path) = setup();
        let result = execute(&path, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_diff_after_commit() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "line1\nline2\n").unwrap();
        add::execute(&path, &vec!["file.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "initial").unwrap();

        fs::write(path.join("file.txt"), "line1\nmodified\n").unwrap();
        let result = execute(&path, Some("file.txt"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_diff_new_file() {
        let (_tmp, path) = setup();
        fs::write(path.join("file.txt"), "content").unwrap();
        let result = execute(&path, Some("file.txt"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_diff_specific_file() {
        let (_tmp, path) = setup();
        fs::write(path.join("a.txt"), "original").unwrap();
        add::execute(&path, &vec!["a.txt".to_string()]).unwrap();
        commit::execute(&path, "Author", "init").unwrap();

        fs::write(path.join("a.txt"), "modified").unwrap();
        fs::write(path.join("b.txt"), "new").unwrap();

        let result = execute(&path, Some("a.txt"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_diff_untracked_file() {
        let (_tmp, path) = setup();
        fs::write(path.join("new.txt"), "brand new").unwrap();
        let result = execute(&path, Some("new.txt"));
        assert!(result.is_ok());
    }
}

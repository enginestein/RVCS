use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RvcsError {
    #[error("Not a valid rvcs repository (missing .rvcs directory)")]
    NotRepository,

    #[error("Repository already exists at '{0}'")]
    RepositoryExists(PathBuf),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Nothing to commit (working tree clean)")]
    NothingToCommit,

    #[error("No such commit: {0}")]
    NoSuchCommit(String),

    #[error("No commits yet")]
    NoCommitsYet,

    #[error("Nothing added to staging area")]
    NothingStaged,

    #[error("Cannot revert: {0}")]
    RevertError(String),

    #[error("Branch '{0}' already exists")]
    BranchExists(String),

    #[error("Branch '{0}' not found")]
    BranchNotFound(String),

    #[error("Cannot delete the current branch '{0}'")]
    CannotDeleteCurrentBranch(String),

    #[error("Cannot switch to branch '{0}': uncommitted changes would be lost")]
    UncommittedChanges(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Compression error: {0}")]
    Compression(String),

    #[error("Encoding error: {0}")]
    Encoding(String),

    #[error("Invalid object hash: {0}")]
    InvalidHash(String),

    #[error("Index corrupted")]
    IndexCorrupted,

    #[error("Path error: {0}")]
    PathError(String),

    #[error("Cannot find a common ancestor to merge")]
    NoCommonAncestor,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, RvcsError>;

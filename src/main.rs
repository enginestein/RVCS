use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rvcs")]
#[command(about = "A locally running CLI-first Version Control System")]
#[command(version = "0.3.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Initialize a new rvcs repository")]
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    #[command(about = "Add file contents to the staging area")]
    Add {
        #[arg(help = "Files to add (empty for all)")]
        files: Vec<String>,
    },

    #[command(about = "Record changes to the repository")]
    Commit {
        #[arg(short, long, default_value = "User <user@rvcs.local>")]
        author: String,

        #[arg(short, long)]
        message: String,
    },

    #[command(about = "Show the working tree status")]
    Status,

    #[command(about = "Show commit logs")]
    Log,

    #[command(about = "Show changes between commits and working tree")]
    Diff {
        #[arg(help = "Specific file to diff")]
        file: Option<String>,

        #[arg(long, help = "Show changes staged for commit (vs last commit)")]
        cached: bool,
    },

    #[command(about = "Reset HEAD to a specific state")]
    Reset {
        #[arg(help = "Commit hash or branch name to reset to")]
        target: String,

        #[arg(long, help = "Reset index and working tree (discards changes)")]
        hard: bool,
    },

    #[command(about = "Revert working tree changes")]
    Revert {
        #[arg(help = "Files to revert (empty for all)")]
        files: Vec<String>,

        #[arg(long, help = "Restore from a specific branch")]
        from: Option<String>,
    },

    #[command(about = "Checkout a specific commit or branch")]
    Checkout {
        #[arg(help = "Commit hash or branch name to checkout")]
        target: String,
    },

    #[command(about = "Manage branches")]
    Branch {
        #[command(subcommand)]
        action: BranchAction,
    },

    #[command(about = "Switch to a different branch")]
    Switch {
        #[arg(help = "Branch name to switch to")]
        name: String,
    },

    #[command(about = "Remove files from the working tree and/or staging area")]
    Rm {
        #[arg(help = "Files to remove")]
        files: Vec<String>,

        #[arg(long, help = "Only remove from staging, keep working tree file")]
        staged: bool,
    },

    #[command(about = "Create, list, or delete tags")]
    Tag {
        #[command(subcommand)]
        action: TagAction,
    },

    #[command(about = "Stash changes away")]
    Stash {
        #[command(subcommand)]
        action: StashAction,
    },

    #[command(about = "Merge a branch into the current HEAD")]
    Merge {
        #[arg(help = "Branch name to merge from")]
        branch: String,
    },
}

#[derive(Subcommand)]
enum BranchAction {
    #[command(about = "Create a new branch at current HEAD")]
    Create {
        #[arg(help = "Branch name")]
        name: String,
    },

    #[command(about = "List all branches")]
    List,

    #[command(about = "Delete a branch")]
    Delete {
        #[arg(help = "Branch name to delete")]
        name: String,
    },
}

#[derive(Subcommand)]
enum TagAction {
    #[command(about = "Create a new tag")]
    Create {
        #[arg(help = "Tag name")]
        name: String,

        #[arg(help = "Commit hash or branch to tag (default: HEAD)")]
        target: Option<String>,
    },

    #[command(about = "List all tags")]
    List,

    #[command(about = "Delete a tag")]
    Delete {
        #[arg(help = "Tag name to delete")]
        name: String,
    },
}

#[derive(Subcommand)]
enum StashAction {
    #[command(about = "Push changes onto the stash")]
    Push,

    #[command(about = "List stashes")]
    List,

    #[command(about = "Pop and restore the latest stash")]
    Pop,

    #[command(about = "Drop a specific stash")]
    Drop {
        #[arg(help = "Stash name to drop (e.g. stash@{0})")]
        name: String,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;

    match cli.command {
        Commands::Init { path } => {
            let path = if path == PathBuf::from(".") {
                cwd
            } else {
                path
            };
            rvcs::commands::init::execute(&path)?;
        }
        Commands::Add { files } => {
            rvcs::commands::add::execute(&cwd, &files)?;
        }
        Commands::Commit { author, message } => {
            rvcs::commands::commit::execute(&cwd, &author, &message)?;
        }
        Commands::Status => {
            rvcs::commands::status::execute(&cwd)?;
        }
        Commands::Log => {
            rvcs::commands::log::execute(&cwd)?;
        }
        Commands::Diff { file, cached } => {
            rvcs::commands::diff::execute(&cwd, file.as_deref(), cached)?;
        }
        Commands::Reset { target, hard } => {
            rvcs::commands::reset::execute(&cwd, &target, hard)?;
        }
        Commands::Revert { files, from } => {
            rvcs::commands::revert::execute(&cwd, &files, from.as_deref())?;
        }
        Commands::Checkout { target } => {
            // Try as branch first, then as commit hash
            let repo = rvcs::core::repository::Repository::open(&cwd);
            if let Ok(repo) = &repo {
                let branch_path = repo.rvcs_dir.join("refs").join(&target);
                if branch_path.exists() {
                    rvcs::commands::switch::execute(&cwd, &target)?;
                } else {
                    rvcs::commands::checkout::execute(&cwd, &target)?;
                }
            } else {
                rvcs::commands::checkout::execute(&cwd, &target)?;
            }
        }
        Commands::Branch { action } => match action {
            BranchAction::Create { name } => {
                rvcs::commands::branch::create(&cwd, &name)?;
            }
            BranchAction::List => {
                rvcs::commands::branch::list(&cwd)?;
            }
            BranchAction::Delete { name } => {
                rvcs::commands::branch::delete(&cwd, &name)?;
            }
        },
        Commands::Switch { name } => {
            rvcs::commands::switch::execute(&cwd, &name)?;
        }
        Commands::Rm { files, staged } => {
            rvcs::commands::rm::execute(&cwd, &files, staged)?;
        }
        Commands::Tag { action } => match action {
            TagAction::Create { name, target } => {
                rvcs::commands::tag::create(&cwd, &name, target.as_deref())?;
            }
            TagAction::List => {
                rvcs::commands::tag::list(&cwd)?;
            }
            TagAction::Delete { name } => {
                rvcs::commands::tag::delete(&cwd, &name)?;
            }
        },
        Commands::Stash { action } => match action {
            StashAction::Push => {
                rvcs::commands::stash::push(&cwd)?;
            }
            StashAction::List => {
                rvcs::commands::stash::list(&cwd)?;
            }
            StashAction::Pop => {
                rvcs::commands::stash::pop(&cwd)?;
            }
            StashAction::Drop { name } => {
                rvcs::commands::stash::drop_stash(&cwd, &name)?;
            }
        },
        Commands::Merge { branch } => {
            rvcs::commands::merge::execute(&cwd, &branch)?;
        }
    }

    Ok(())
}

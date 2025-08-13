use anyhow::Result;
use clap::{Parser, Subcommand};
use gitlet_rs::{
    index::{self, IndexAction},
    repo,
};

#[derive(Debug, Parser)]
#[command(name = "gitlet")]
#[command(about = "A simple version control CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Initialize a new gitlet repository
    Init {
        /// Optional path to specify. Default to PWD.
        repo_dir: Option<String>,
    },

    /// Stage a file for commit
    Add { filepath: String },

    /// Unstage a file that is staged for commit
    Unstage { filepath: String },

    /// Stage a file for removal
    Rm {
        #[arg(long)]
        cached: bool, // Only untrack the file, leave it in working tree.
        filepath: String,
    },

    /// Display the status of the gitlet repository
    Status,
}

fn main() -> Result<()> {
    let args = Cli::parse();

    match args.command {
        Commands::Init { repo_dir } => repo::init(repo_dir)?,
        Commands::Add { filepath } => index::action(IndexAction::Add, &filepath)?,
        Commands::Unstage { filepath } => index::action(IndexAction::Unstage, &filepath)?,
        Commands::Rm { cached, filepath } => index::rm(cached, &filepath)?,
        Commands::Status => repo::status()?,
    }

    Ok(())
}

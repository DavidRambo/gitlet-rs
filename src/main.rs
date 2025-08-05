use clap::{Parser, Subcommand};
use gitlet_rs::repo;

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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    match args.command {
        Commands::Init { repo_dir } => repo::init(repo_dir)?,
    }

    Ok(())
}

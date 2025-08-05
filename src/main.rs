use std::fs;
use std::path::Path;

use clap::{Parser, Subcommand};

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
        Commands::Init { repo_dir } => {
            // If a repository directory was provided, then convert it to a Path,
            // otherwise, use the PWD.
            let repo_dir = match repo_dir {
                Some(dir) => dir.clone(),
                None => ".".to_string(),
            };
            let rpath = Path::new(&repo_dir);

            if rpath.join(".gitlet").exists() {
                return Err("A gitlet repository already exists in this directory".into());
            }

            if !rpath.exists() {
                fs::create_dir(rpath).expect("Failed to create directory for repository");
            }

            fs::create_dir(rpath.join(".gitlet"))?;
            fs::create_dir(rpath.join(".gitlet/blobs"))?;
            fs::create_dir(rpath.join(".gitlet/commits"))?;
            fs::create_dir(rpath.join(".gitlet/refs"))?;
            fs::create_dir(rpath.join(".gitlet/index"))?;
            fs::File::create(rpath.join(".gitlet/HEAD"))?;

            println!("Initialized empty Gitlet repository");
        }
    }

    Ok(())
}

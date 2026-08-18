use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "rf", version, about = "Update squad saves")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Update(UpdateArgs),
    Verify {
        path: PathBuf,
    },
    Inspect {
        path: PathBuf,
    },
    Download {
        #[arg(long, default_value = "downloaded")]
        output_dir: PathBuf,
    },
}

#[derive(Debug, clap::Args)]
pub struct UpdateArgs {
    #[arg(long)]
    pub data: Option<PathBuf>,
    #[arg(long)]
    pub squad_file: Option<PathBuf>,
    #[arg(long, default_value = "downloaded")]
    pub download_dir: PathBuf,
    #[arg(long, default_value = "output")]
    pub output_dir: PathBuf,
    #[arg(long, default_value = "backup")]
    pub backup_dir: PathBuf,
    #[arg(long)]
    pub dry_run: bool,
}

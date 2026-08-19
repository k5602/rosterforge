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
    Restore(RestoreArgs),
    Download {
        /// Squad platform to fetch from the EA manifest.
        #[arg(long, value_enum, default_value_t = TargetPlatform::Ps4)]
        platform: TargetPlatform,
        /// Fetch the FUT squads file instead of the major squads file.
        #[arg(long)]
        fut: bool,
        /// EA content base URL. Override when a new title year rotates it.
        #[arg(long, default_value_t = crate::download::DEFAULT_CONTENT_URL.to_owned())]
        content_url: String,
        #[arg(long, default_value = "downloaded")]
        output_dir: PathBuf,
    },
}

/// Platforms present in the EA roster manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TargetPlatform {
    Ps4,
    Xbox,
}

impl From<TargetPlatform> for rf::roster::Platform {
    fn from(platform: TargetPlatform) -> Self {
        match platform {
            TargetPlatform::Ps4 => rf::roster::Platform::Ps4,
            TargetPlatform::Xbox => rf::roster::Platform::Xbox,
        }
    }
}

#[derive(Debug, clap::Args)]
pub struct RestoreArgs {
    /// Backup container created by `rf update` (contains DATA and .source_sha256).
    pub backup: PathBuf,
    /// Save DATA file to overwrite with the backup copy.
    pub destination: PathBuf,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, clap::Args)]
pub struct UpdateArgs {
    #[arg(long)]
    pub data: Option<PathBuf>,
    #[arg(long)]
    pub squad_file: Option<PathBuf>,
    /// EA content base URL. Override when a new title year rotates it.
    #[arg(long, default_value_t = crate::download::DEFAULT_CONTENT_URL.to_owned())]
    pub content_url: String,
    #[arg(long, default_value = "downloaded")]
    pub download_dir: PathBuf,
    #[arg(long, default_value = "output")]
    pub output_dir: PathBuf,
    #[arg(long, default_value = "backup")]
    pub backup_dir: PathBuf,
    #[arg(long)]
    pub dry_run: bool,
}

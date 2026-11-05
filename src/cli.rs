use std::path::PathBuf;

use clap::{Parser, Subcommand};
use clap_complete::Shell;

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
        /// Squad platform to fetch from the EA manifest.
        #[arg(long, value_enum, default_value_t = TargetPlatform::Ps4)]
        platform: TargetPlatform,
        /// Fetch the FUT squads file instead of the major squads file.
        #[arg(long)]
        fut: bool,
        /// Game database year. Selects the default EA content URL.
        #[arg(long, value_enum, default_value = "fc27")]
        game: GameVersion,
        /// EA content base URL. Overrides --game.
        #[arg(long)]
        content_url: Option<String>,
        #[arg(long, default_value = "downloaded")]
        output_dir: PathBuf,
    },
    Restore(RestoreArgs),
    /// Generate shell completions for the given shell.
    Completions {
        shell: Shell,
    },
    /// Print a roff man page to stdout.
    #[command(hide = true)]
    Manpage,
}

/// Platforms present in the EA roster manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TargetPlatform {
    Ps4,
    Xbox,
}

/// EA FC title years with known content URLs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum GameVersion {
    #[value(name = "fc26")]
    Fc26,
    #[default]
    #[value(name = "fc27")]
    Fc27,
}

impl GameVersion {
    pub fn content_url(self) -> &'static str {
        match self {
            GameVersion::Fc26 => rf::download::FC26_CONTENT_URL,
            GameVersion::Fc27 => rf::download::FC27_CONTENT_URL,
        }
    }
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
    /// Game database year. Selects the default EA content URL.
    #[arg(long, value_enum, default_value = "fc27")]
    pub game: GameVersion,
    /// EA content base URL. Overrides --game.
    #[arg(long)]
    pub content_url: Option<String>,
    #[arg(long, default_value = "downloaded")]
    pub download_dir: PathBuf,
    #[arg(long, default_value = "output")]
    pub output_dir: PathBuf,
    #[arg(long, default_value = "backup")]
    pub backup_dir: PathBuf,
    #[arg(long)]
    pub dry_run: bool,
}

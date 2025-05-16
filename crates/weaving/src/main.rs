use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use resolve_path::PathResolveExt;
use weaver_lib::Weaver;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    Build {
        #[arg(short, long, default_value = ".")]
        config_path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.cmd {
        Commands::Build { config_path } => {
            let mut instance = Weaver::new(fs::canonicalize(config_path.resolve())?);

            instance.scan_content().scan_templates().build().await?;
        }
    }

    Ok(())
}

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use lime_ui::{run_app, AppOptions};

#[derive(Debug, Parser)]
#[command(name = "lime")]
#[command(about = "A clean, modern terminal text editor")]
struct Cli {
    /// File or directory to open
    path: Option<PathBuf>,

    /// Open very large files without confirmation/refusal
    #[arg(long)]
    force: bool,

    /// Path to a Lime config file
    #[arg(long)]
    config: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run_app(AppOptions::new(cli.path, cli.force, cli.config))
}

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub file: PathBuf,

    #[arg(short, long)]
    pub action: String,

    #[arg(short, long, default_value = "1")]
    pub recovering_level: usize,
}
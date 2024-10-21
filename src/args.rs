use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct Args {
    pub command: String,
    pub path: PathBuf,
}

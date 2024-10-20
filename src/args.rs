use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct Args {
    pub command: Command,
    pub path: PathBuf,
}

pub enum Command {
    Send,
    Recv,
}
use std::path::PathBuf;

use clap::Parser;
use clap_derive::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(value_enum, short, long)]
    action: Action,
    #[arg(short, long)]
    file: String,
}

impl Args {
    pub fn get_action(&self) -> Action {
        self.action
    }

    pub fn get_file(&self) -> PathBuf {
        PathBuf::from(&self.file)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, ValueEnum)]
pub enum Action {
    Send,
    Receive,
}

pub fn load_args() -> Args {
    Args::parse()
}

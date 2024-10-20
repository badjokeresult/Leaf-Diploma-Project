use std::path::PathBuf;

use clap::Parser;

use sentfile::SentFile;
use args::{Args, Command};

use client::{recv_content, send_content};

mod args;
mod sentfile;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let content = tokio::fs::read(&args.path).await.unwrap();

    match args.command {
        Command::Send => handle_send_request(content, &args.path).await,
        Command::Recv => handle_recv_request(content, &args.path).await,
    };
}

async fn handle_send_request(content: Vec<u8>, filepath: &PathBuf) {
    let content_len = content.len();
    let hashes = send_content(content).await;
    let sent_file = SentFile::new(hashes, content_len);
    sent_file.save_metadata(filepath).await;
}

async fn handle_recv_request(content: Vec<u8>, filepath: &PathBuf) {
    let sent_file = SentFile::from_metadata(&content);
    let file_content = recv_content(sent_file.hashes).await;
    tokio::fs::write(filepath, &file_content).await.unwrap();
}
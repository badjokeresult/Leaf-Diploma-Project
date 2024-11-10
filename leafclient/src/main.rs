mod args;
mod meta;

use std::path::PathBuf;

use clap::Parser;

use leaflibrary::*;
use args::Args;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let encryptor = KuznechikEncryptor::new(
        &dirs::home_dir().unwrap().join(".leaf").join("password.txt"),
        &dirs::home_dir().unwrap().join(".leaf").join("gamma.bin"),
    ).unwrap();

    match args.action.as_str() {
        "send" => handle_send(&args.file, &encryptor).await,
        "recv" => handle_recv(&args.file, &encryptor).await,
        _ => {},
    }
}

async fn handle_send(path: &PathBuf, encryptor: &KuznechikEncryptor) {
    todo!()
}

async fn handle_recv(path: &PathBuf, decryptor: &KuznechikEncryptor) {
    todo!()
}
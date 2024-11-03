use std::path::PathBuf;
use std::process;
use std::time::Duration;

use tokio::task;
use tokio::fs::File;
use daemonize::Daemonize;

use leaflibrary::BroadcastUdpServer;

#[tokio::main]
async fn main() {
    let working_dir = match dirs::home_dir() {
        Some(d) => d.join(".leaf"),
        None => {
            eprintln!("Error resolving home dir");
            process::exit(1);
        },
    };
    let stdout = get_file(&working_dir.join("log.txt")).await;
    let stderr = get_file(&working_dir.join("err.log")).await;

    let daemon = Daemonize::new()
        .pid_file("/tmp/leaf.pid")
        .working_directory(&working_dir)
        .user(1000)
        .group(1000)
        .umask(0o600)
        .stdout(stdout.into_std().await)
        .stderr(stderr.into_std().await);

    match daemon.start() {
        Ok(_) => {
            let server = BroadcastUdpServer::new(
                &working_dir.join("chunks"),
            ).await;
            let num_threads = num_cpus::get();
            for _ in 0..num_threads {
                let server_clone = server.clone();
                task::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    server_clone.listen().await;
                });
            };
        },
        Err(e) => {
            eprintln!("{}", e.to_string());
            process::exit(3);
        },
    }
}

async fn get_file(filename: &PathBuf) -> File {
    match File::open(filename).await {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}", e.to_string());
            match File::create(filename).await {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("{}", e.to_string());
                    process::exit(2);
                },
            }
        },
    }
}
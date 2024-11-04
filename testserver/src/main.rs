use tokio::task;
use leaflibrary::BroadcastUdpServer;

#[tokio::main]
async fn main() {
    let num_threads = num_cpus::get();
    println!("NUM_CPUS : {}", num_threads);

    let server = BroadcastUdpServer::new(&dirs::home_dir().unwrap().join(".leaf").join("chunks")).await;

    for _ in 0..num_threads {
        let server_clone = server.clone();
        task::spawn(async move {
            server_clone.listen().await;
        });
    };
}

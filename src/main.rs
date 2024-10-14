use tokio;
use tokio::task;
use tokio::task::JoinHandle;

use server::{BroadcastServerPeer, ServerPeer};

#[tokio::main]
async fn main() {
    let server_handle = init_server().await;

}

async fn init_server() -> JoinHandle<()> {
    let server = BroadcastServerPeer::new().await.unwrap();
    task::spawn(async move {
        server.listen().await;
    })
}

async fn init_client() -> JoinHandle<()> {
    let client =
}
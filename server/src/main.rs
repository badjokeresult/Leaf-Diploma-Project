mod peer;
mod storage;

use dirs::home_dir;

use peer::{BroadcastServerPeer, ServerPeer};

const DEFAULT_STORAGE_FILE_NAME: &str = "stor.bin";

#[tokio::main]
async fn main() {
    let home_dir = home_dir().unwrap().join("server").join(DEFAULT_STORAGE_FILE_NAME);

    let server = BroadcastServerPeer::new(home_dir).await;
    server.listen().await;
}
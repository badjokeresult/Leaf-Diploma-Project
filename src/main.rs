mod server_peer;
mod storage;

use dirs::home_dir;

use server_peer::{BroadcastServerPeer, ServerPeer};

const DEFAULT_STORAGE_FILE_NAME: &str = "stor.bin";

fn main() {
    let home_dir = home_dir().unwrap().join("server").join(DEFAULT_STORAGE_FILE_NAME);
    let server = BroadcastServerPeer::new(home_dir);
    server.listen();
}
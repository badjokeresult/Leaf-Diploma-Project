mod peer;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub async fn send_file(file_content: Vec<u8>) -> Result<Vec<Vec<u8>>> {
    todo!()
}

pub async fn recv_file(parts_hashes: Vec<Vec<u8>>) -> Result<Vec<u8>> {
    todo!()
}

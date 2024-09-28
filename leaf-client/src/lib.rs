mod peer;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[no_mangle]
pub async extern "C" fn send_file(file_content: Vec<u8>) -> Result<Vec<Vec<u8>>> {
    todo!()
}

#[no_mangle]
pub async extern "C" fn recv_file(parts_hashes: Vec<Vec<u8>>) -> Result<Vec<u8>> {
    todo!()
}

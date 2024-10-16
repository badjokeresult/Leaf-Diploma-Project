use std::io::Error;

mod peer;

pub async fn send_content(content: Vec<u8>) -> Result<Vec<Vec<u8>>, Error> {
    Ok((vec![vec![0u8]]))
}

pub async fn recv_content(hashes: Vec<Vec<u8>>) -> Result<Vec<u8>, Error> {
    Ok(vec![0u8])
}
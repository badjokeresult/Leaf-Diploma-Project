use tokio::net::UdpSocket;

#[tokio::main]
async fn main() {
    let socket = UdpSocket::bind("192.168.124.1:62092").await.unwrap();
    socket.set_broadcast(true).unwrap();
    socket.send_to(b"Hello world", "192.168.124.255:62092").await.unwrap();
}

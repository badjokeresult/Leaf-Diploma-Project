use crate::parts::{FileParts, Parts};
use common::Message;
use tokio::net::UdpSocket;

pub trait Socket {
    async fn send(&self, parts: &mut FileParts) -> Result<(), Box<dyn std::error::Error>>;
    async fn recv(&self, parts: &mut FileParts) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct ClientSocket {
    socket: UdpSocket,
}

impl ClientSocket {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        socket.set_broadcast(true).unwrap();

        Ok(Self { socket })
    }
}

impl Socket for ClientSocket {
    async fn send(&self, parts: &mut FileParts) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = [0u8; 4096];
        let mut data = parts.get_data_cloned();
        let mut hashes = parts.get_hashes_cloned();

        for i in 0..data.len() {
            let req: Vec<u8> = Message::SendingReq(hashes[i].clone()).into();
            self.socket.send_to(&req, "255.255.255.255:62092").await?;
            while let Ok((sz, addr)) = self.socket.recv_from(&mut buf).await {
                let ack = Message::from(buf[..sz].to_vec());
                if let Message::SendingAck(h) = ack {
                    if h.iter().eq(&hashes[i]) {
                        let stream =
                            Message::generate_stream_for_chunk(&hashes[i], &data[i]).unwrap();
                        for msg in stream {
                            let message_bin: Vec<u8> = msg.into();
                            self.socket.send_to(&message_bin, addr).await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn recv(&self, parts: &mut FileParts) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = [0u8; 4096];
        let mut hashes = parts.get_hashes_cloned();
        let mut data = Vec::with_capacity(hashes.len());

        for i in 0..hashes.len() {
            let req: Vec<u8> = Message::RetrievingReq(hashes[i].clone()).into();
            self.socket.send_to(&req, "255.255.255.255:62092").await?;
            while let Ok((sz, addr)) = self.socket.recv_from(&mut buf).await {
                let ack = Message::from(buf[..sz].to_vec());
                if let Message::RetrievingAck(h) = ack {
                    if h.iter().eq(&hashes[i]) {
                        while let Ok((sz, addr)) = self.socket.recv_from(&mut buf).await {
                            let content = Message::from(buf[..sz].to_vec());
                            if let Message::ContentFilled(h, d) = content {
                                data.push(d);
                            }
                        }
                    }
                }
            }
        }

        parts.paste_data(data)?;
        Ok(())
    }
}

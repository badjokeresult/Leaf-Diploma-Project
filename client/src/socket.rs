use crate::parts::Parts;

pub trait Socket {
    async fn send(parts: impl Parts);
    async fn recv();
}
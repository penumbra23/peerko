use std::{net::SocketAddr, fmt::Display, error::Error};

#[derive(Clone, Debug)]
pub struct TransportError {
    pub error: String,
}

impl Error for TransportError {}

impl Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Transport err: {}", self.error)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TransportPacket {
    pub socket_addr: SocketAddr,
    pub data: Vec<u8>,
}

pub trait Transport {
    fn send(&self, packet: TransportPacket) -> Result<usize, TransportError>;
    fn recv(&self) -> Result<TransportPacket, TransportError>;
}

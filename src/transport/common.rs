use std::{net::SocketAddr};

#[derive(Clone)]
pub struct TransportError {
    pub error: String,
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

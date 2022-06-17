use std::net::{UdpSocket, SocketAddr};

use super::common::{Transport, TransportError, TransportPacket};

struct UdpTransport {
    socket: UdpSocket,
}

impl UdpTransport {
    pub fn new(addr: SocketAddr) -> Result<UdpTransport, TransportError> {
        let soc = UdpSocket::bind(addr).map_err(|err| TransportError{ error: err.to_string() })?;
        Ok(UdpTransport{
            socket: soc,
        })
    }
}

impl Transport for UdpTransport {
    fn send(&self, packet: TransportPacket) -> Result<usize, TransportError> {
        self.socket
        .send_to(
            packet.data.as_slice(),
            packet.socket_addr
        ).map_err(|err| TransportError{ error: err.to_string() })
    }

    fn recv(&self) -> Result<TransportPacket, TransportError> {
        let mut buf: [u8; 1000] = [0; 1000];
        let (byte_count, addr) = self.socket.recv_from(&mut buf).map_err(|err| TransportError{ error: err.to_string() })?;
        Ok(TransportPacket{
            data: Vec::from(&buf[..byte_count]),
            socket_addr: addr,
        })
    }
}
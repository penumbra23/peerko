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
        let mut buf: [u8; 1024] = [0; 1024];
        let (byte_count, addr) = self.socket.recv_from(&mut buf).map_err(|err| TransportError{ error: err.to_string() })?;
        Ok(TransportPacket{
            data: Vec::from(&buf[..byte_count]),
            socket_addr: addr,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use crate::transport::common::{Transport, TransportPacket};

    use super::UdpTransport;

    #[test]
    fn basic_send_recv() {
        let udp1 = UdpTransport::new("0.0.0.0:9232".parse().unwrap()).unwrap();
        let udp2 = UdpTransport::new("0.0.0.0:9233".parse().unwrap()).unwrap();

        thread::spawn(move || {
            let packet = udp1.recv().unwrap();
            assert_eq!(packet.data, vec![0x1]);
            udp1.send(TransportPacket { data: vec![0x2, 0x3], socket_addr: "127.0.0.1:9233".parse().unwrap() }).unwrap();

        });

        udp2.send(TransportPacket{ data: vec![0x1], socket_addr: "127.0.0.1:9232".parse().unwrap() }).unwrap();
        let packet = udp2.recv().unwrap();
        assert_eq!(packet.data, vec![0x2, 0x3]);
    }
}
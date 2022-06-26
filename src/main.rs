use std::net::{SocketAddr, UdpSocket};

use clap::Parser;

use crate::message::format::{Header, Message, MemberRequest, MessageType};
use crate::transport::udp::UdpTransport;
use crate::transport::common::{Transport, TransportPacket};

mod transport;
mod message;

#[derive(Parser, Debug)]
struct CliArgs {
    #[clap(long, value_parser, short = 'n')]
    name: String,

    #[clap(long, value_parser, short = 'p')]
    port: u16,

    #[clap(long, value_parser, short = 'c')]
    client_port: u16,
}

fn main() {
    println!("Hello, world!");
    let args = CliArgs::parse();
    
    let socket = UdpSocket::bind(SocketAddr::new("0.0.0.0".parse().unwrap(), args.port)).unwrap();

    let tx_sock = socket.try_clone().unwrap();
    let t = std::thread::spawn(move || {
        let mut buf: [u8; 1024] = [0; 1024];
        loop {
            let (bytes_read, peer) = socket.recv_from(&mut buf).unwrap();
            // let msg = Message::from(buf[..bytes_read].to_vec());
            match MessageType::from(buf[1] & 0x0F) {
                MessageType::Alive => println!("alive"),
                MessageType::MemberReq => {
                    let msg = Message::<MemberRequest>::from(buf[..bytes_read].to_vec());
                    let size = msg.header().msg_size();
                    println!("req for {:?}", msg.content().group_name().unwrap());
                },
                MessageType::MemberRes => todo!(),
                MessageType::Chat => todo!(),
            }
        }
    });

    std::thread::sleep(std::time::Duration::from_secs(2));

    let mut header = Header::new(1, message::format::MessageType::Alive, 0);
    let mut buf: Vec<u8> = header.into();
    tx_sock.send_to(&buf[..], SocketAddr::new("127.0.0.1".parse().unwrap(), args.client_port)).unwrap();

    header = Header::new(1, message::format::MessageType::MemberReq, 10);
    let msg = Message::new(header, MemberRequest::new("moja_grupa").unwrap());
    buf = msg.into();
    tx_sock.send_to(&buf[..], SocketAddr::new("127.0.0.1".parse().unwrap(), args.client_port)).unwrap();

    t.join().unwrap();
}

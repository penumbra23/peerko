use std::{net::{SocketAddr, UdpSocket}, collections::HashMap, sync::{Mutex, Arc}};

use clap::Parser;
use message::format::Empty;
use routing::router::Handler;
use transport::{udp::UdpTransport, common::{Transport, TransportPacket}};

use crate::{message::format::{MemberResponse, Header, Message, MemberRequest, MessageType}, routing::router::Router};

mod transport;
mod message;
mod routing;

#[derive(Parser, Debug)]
struct CliArgs {
    #[clap(long, value_parser, short = 'n')]
    name: String,

    #[clap(long, value_parser, short = 'g')]
    group: Option<String>,

    #[clap(long, value_parser, short = 'p')]
    port: u16,

    #[clap(long, value_parser, short = 'b')]
    bootstrap: Option<SocketAddr>,
}

fn main() {
    let args = CliArgs::parse();
    
    let socket = UdpTransport::new(SocketAddr::new("0.0.0.0".parse().unwrap(), args.port)).unwrap();

    let peer_map_lock = Arc::new(Mutex::new(HashMap::<String, Vec<SocketAddr>>::new()));
    let alive_peer_map_lock = peer_map_lock.clone();

    let tx_sock = socket.try_clone().unwrap();
    let alive_sock = socket.try_clone().unwrap();

    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let mut peer_map = alive_peer_map_lock.lock().unwrap();
    
            for (group, peer_list) in peer_map.iter() {
                for peer in peer_list.iter() {
                    let msg = Message::<Empty>::new(Header::new(1, MessageType::Alive, 0), Empty::new());
                    alive_sock.send(TransportPacket { socket_addr: *peer, data: msg.into() }).unwrap();
                }
            }
        }
    });

    let t = std::thread::spawn(move || {
        loop {
            let packet = match socket.recv() {
                Ok(p) => p,
                Err(err) => {
                    println!("error: {}", err);
                    continue;
                },
            };

            println!("msg from: {}", packet.socket_addr);
            std::thread::sleep(std::time::Duration::from_secs(3));

            let header_bytes = &packet.data[0..4];
            let header = Header::from(header_bytes.to_vec());

            match header.msg_type() {
                MessageType::Alive => println!("Alive"),
                MessageType::MemberReq => {
                    let msg = Message::<MemberRequest>::from(packet.data);
                    let group_name = msg.content().group_name().unwrap();
                    
                    let mut peer_map = peer_map_lock.lock().unwrap();

                    if !peer_map.contains_key(&group_name) {
                        peer_map.insert(group_name.clone(), Vec::new());
                    }

                    let peer_list = peer_map.get_mut(&group_name).unwrap();

                    if !peer_list.contains(&packet.socket_addr) {
                        peer_list.push(packet.socket_addr);
                    }

                    let res_msg = Message::<MemberResponse>::new(
                        Header::new(1, MessageType::MemberRes, 0),
                        MemberResponse::new(&group_name, peer_list.to_vec()).unwrap()
                    );

                    socket.send(TransportPacket { socket_addr: packet.socket_addr, data: res_msg.into() }).unwrap();
                },
                MessageType::MemberRes => {
                    let msg = Message::<MemberResponse>::from(packet.data);
                    let peers = msg.content().peers();
                    let group_name = msg.content().group_name().unwrap();

                    let mut peer_map = peer_map_lock.lock().unwrap();

                    if !peer_map.contains_key(&group_name) {
                        peer_map.insert(group_name.clone(), Vec::new());
                    }

                    let peer_list = peer_map.get_mut(&group_name).unwrap();

                    for peer in peers {
                        peer_list.push(peer);
                    }
                },
                MessageType::Chat => println!("Chat"),
            }
        }
    });

    std::thread::sleep(std::time::Duration::from_secs(2));

    if let Some(bootstrap) = args.bootstrap {
        let header = Header::new(1, message::format::MessageType::MemberReq, 10);
        let msg = Message::<MemberRequest>::new(header, MemberRequest::new(&args.group.unwrap()).unwrap());
        let buf: Vec<u8> = msg.into();
        tx_sock.send(TransportPacket {
            socket_addr: bootstrap,
            data: buf,
        }).unwrap();
    }

    t.join().unwrap();
}

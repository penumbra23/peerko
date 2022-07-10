use std::{net::{SocketAddr}, collections::HashMap, sync::{Mutex, Arc}};

use clap::Parser;
use message::format::{Empty, Chat};
use transport::{udp::UdpTransport, common::{Transport, TransportPacket}};

use crate::{message::format::{MemberResponse, Header, Message, MemberRequest, MessageType}};

mod transport;
mod message;

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

fn send_member_request(sock: &UdpTransport, peer: SocketAddr, group: &str) {
    let header = Header::new(1, message::format::MessageType::MemberReq, 10);
    let msg = Message::<MemberRequest>::new(header, MemberRequest::new(group).unwrap());
    let buf: Vec<u8> = msg.into();
    sock.send(TransportPacket {
        socket_addr: peer,
        data: buf,
    }).unwrap();
}

fn main() {
    let args = CliArgs::parse();
    
    let socket = UdpTransport::new(SocketAddr::new("0.0.0.0".parse().unwrap(), args.port)).unwrap();

    let peer_map_lock = Arc::new(Mutex::new(HashMap::<String, Vec<SocketAddr>>::new()));
    let alive_peer_map_lock = peer_map_lock.clone();
    let chat_peer_map_lock = peer_map_lock.clone();

    let tx_sock = socket.try_clone().unwrap();
    let alive_sock = socket.try_clone().unwrap();

    // Keep alive thread
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            let peer_map = alive_peer_map_lock.lock().unwrap();
    
            for (group, peer_list) in peer_map.iter() {
                for peer in peer_list.iter() {
                    let msg = Message::<Empty>::new(Header::new(1, MessageType::Alive, 0), Empty::new());
                    alive_sock.send(TransportPacket { socket_addr: *peer, data: msg.into() }).unwrap();
                }
            }
        }
    });

    // Handles response messages
    std::thread::spawn(move || {
        loop {
            let packet = match socket.recv() {
                Ok(p) => p,
                Err(err) => {
                    println!("error: {}", err);
                    continue;
                },
            };

            let header_bytes = &packet.data[0..4];
            let header = Header::from(header_bytes.to_vec());

            match header.msg_type() {
                MessageType::Alive => (),
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
                        if !peer_list.contains(&peer) {
                            peer_list.push(peer);
                        }
                    }
                },
                MessageType::Chat => {
                    let msg = Message::<Chat>::from(packet.data);
                    println!("{}", msg.content().msg().unwrap());
                },
            }
        }
    });

    if let Some(bootstrap) = args.bootstrap {
        send_member_request(&tx_sock, bootstrap, args.group.as_ref().unwrap());
    }

    // Input thread for reading the input and sending to other peers
    loop {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        
        let peer_map = chat_peer_map_lock.lock().unwrap();
        
        match line.trim() {
            "peers" => {
                println!("{:?}", peer_map);
                continue;
            },
            "req" => {
                if let Some(bootstrap) = args.bootstrap {
                    send_member_request(&tx_sock, bootstrap, args.group.as_ref().unwrap());
                };
                continue;
            },
            _ => (),
        }

        let header = Header::new(1, message::format::MessageType::Chat, line.len().try_into().unwrap());
        for (group, peer_list) in peer_map.iter() {
            for peer in peer_list.iter() {
                let msg = Message::<Chat>::new(header, Chat::new(&args.group.as_ref().unwrap(), line.as_str()).unwrap());
                tx_sock.send(TransportPacket {
                    socket_addr: *peer,
                    data: msg.into(),
                }).unwrap();
            }
        }
    }
}

use std::{net::{SocketAddr}, collections::HashMap, sync::{Mutex, Arc}};

use clap::Parser;
use message::format::{Chat};
use transport::{udp::UdpTransport, common::{Transport, TransportPacket}};

use crate::{message::format::{MemberResponse, Header, Message, MemberRequest, MessageType}};

mod transport;
mod message;

#[derive(Clone, Parser, Debug)]
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

fn send_member_request(sock: &UdpTransport, peer: SocketAddr, peer_id: &str, group: &str) {
    let header = Header::new(1, message::format::MessageType::MemberReq, 64);
    let msg = Message::<MemberRequest>::new(header, Some(MemberRequest::new(peer_id, group).unwrap()));
    let buf: Vec<u8> = msg.into();
    sock.send(TransportPacket {
        socket_addr: peer,
        data: buf,
    }).unwrap();
}

fn main() {
    let args = CliArgs::parse();
    
    let socket = UdpTransport::new(SocketAddr::new("0.0.0.0".parse().unwrap(), args.port)).unwrap();

    let peer_map_lock = Arc::new(Mutex::new(HashMap::<String, Vec<(String, SocketAddr)>>::new()));
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
                    let msg = Message::<Chat>::new(Header::new(1, MessageType::Alive, 0), None);
                    alive_sock.send(TransportPacket { socket_addr: peer.1, data: msg.into() }).unwrap();
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
            let header = Header::try_from(header_bytes.to_vec()).unwrap();

            match header.msg_type() {
                MessageType::Alive => (),
                MessageType::MemberReq => {
                    let msg = Message::<MemberRequest>::try_from(packet.data).unwrap();
                    let content = msg.content().unwrap();
                    let group_name = content.group_name();
                    let peer_id = content.peer_id();

                    let mut peer_map = peer_map_lock.lock().unwrap();

                    if !peer_map.contains_key(&group_name) {
                        peer_map.insert(group_name.clone(), Vec::new());
                    }

                    let peer_list = peer_map.get_mut(&group_name).unwrap();

                    if !peer_list.contains(&(peer_id.clone(), packet.socket_addr)) {
                        peer_list.push((peer_id, packet.socket_addr));
                    }

                    let peer_id = content.peer_id();
                    let response_peers = peer_list.clone().drain(..).filter(|s| s.0 != peer_id.clone()).collect();

                    let res_msg = Message::<MemberResponse>::new(
                        Header::new(1, MessageType::MemberRes, 0),
                        Some(MemberResponse::new(&group_name, response_peers).unwrap())
                    );

                    socket.send(TransportPacket { socket_addr: packet.socket_addr, data: res_msg.into() }).unwrap();
                },
                MessageType::MemberRes => {
                    let msg = Message::<MemberResponse>::try_from(packet.data).unwrap();

                    let content = msg.content().unwrap();
                    let peers = content.peers();
                    let group_name = content.group_name();

                    let mut peer_map = peer_map_lock.lock().unwrap();

                    if !peer_map.contains_key(&group_name) {
                        peer_map.insert(group_name.clone(), Vec::new());
                    }

                    let peer_list = peer_map.get_mut(&group_name).unwrap();

                    for peer in peers {
                        if !peer_list.contains(&peer.clone()) {
                            peer_list.push(peer.clone());
                        }
                    }
                },
                MessageType::Chat => {
                    let msg = Message::<Chat>::try_from(packet.data).unwrap();
                    let content = msg.content().unwrap();
                    println!("{}: {}", content.peer_id(), content.msg());
                },
            }
        }
    });

    if let Some(bootstrap) = args.bootstrap {
        send_member_request(&tx_sock, bootstrap, &args.name, args.group.as_ref().unwrap());
    }

    // Input thread for reading the input and sending to other peers
    loop {
        
        // Server mode
        if args.bootstrap.is_none() {
            continue;
        }

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
                    let peer_name = args.name.clone();
                    send_member_request(&tx_sock, bootstrap, &peer_name, args.group.as_ref().unwrap());
                };
                continue;
            },
            _ => (),
        }

        let chat_line = line.clone();

        let header = Header::new(1, message::format::MessageType::Chat, line.len().try_into().unwrap());
        for (group, peer_list) in peer_map.iter() {
            for peer in peer_list.iter() {
                let chat = Chat::new(args.name.clone(), &chat_line);
                let msg = Message::<Chat>::new(header, Some(chat));
                tx_sock.send(TransportPacket {
                    socket_addr: peer.1,
                    data: msg.into(),
                }).unwrap();
            }
        }
    }
}
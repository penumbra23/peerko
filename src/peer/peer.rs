use std::{net::SocketAddr, error::Error, fmt::Display, sync::{atomic::{AtomicBool, Ordering}, Arc, mpsc::{Receiver, channel, Sender}, Mutex}, collections::HashMap};

use crate::{transport::{udp::UdpTransport, common::{TransportPacket, Transport}}, message::{format::{Message, Chat, Header, MessageType, MemberRequest, MemberResponse, MessageContent, FormatError}, self}};

#[derive(Clone, Debug)]
struct PeerError {
    msg: String,
}

impl Display for PeerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Peer error: {}", self.msg)
    }
}

pub struct Peer {
    name: String,
    group: String,
    port: u16,
    bootstrap: Option<SocketAddr>,
    transport: UdpTransport,
    is_running: Arc<AtomicBool>,
    tx: Sender<String>,
    rx: Receiver<String>,
    peer_map: Arc<Mutex<HashMap::<String, Vec<(String, SocketAddr)>>>>,
}

impl Peer {
    pub fn new(name: String, group: String, port: u16, bootstrap: Option<SocketAddr>) -> Result<Peer, Box<dyn Error>> {
        let (tx, rx) = channel();
        let peer_map = Arc::new(Mutex::new(HashMap::<String, Vec<(String, SocketAddr)>>::new()));
        Ok(Peer {
            name,
            group,
            port,
            bootstrap,
            transport: UdpTransport::new(SocketAddr::new("0.0.0.0".parse().unwrap(), port)).unwrap(),
            is_running: Arc::new(AtomicBool::new(false)),
            rx, tx,
            peer_map,
        })
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    pub fn tx(&self) -> Sender<String> {
        self.tx.clone()
    }

    pub fn send_req(&self, peer_socket: SocketAddr) {
        let header = Header::new(1, message::format::MessageType::MemberReq, 64);
        let msg = Message::<MemberRequest>::new(header, Some(MemberRequest::new(&self.name.clone(), &self.group).unwrap()));
        let buf: Vec<u8> = msg.into();
        self.transport.send(TransportPacket {
            socket_addr: peer_socket,
            data: buf.clone(),
        }).unwrap();
    }

    pub fn run(&mut self) -> ! {
        let alive_peer_map_lock = self.peer_map.clone();
        let chat_peer_map_lock = self.peer_map.clone();
        let handler_peer_map_lock = self.peer_map.clone();

        let recv_sock = self.transport.try_clone().unwrap();
        let alive_sock = self.transport.try_clone().unwrap();
        let cmd_sock = self.transport.try_clone().unwrap();

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
                // Receive the packet
                let packet = match recv_sock.recv() {
                    Ok(p) => p,
                    Err(err) => {
                        println!("error: {}", err);
                        continue;
                    },
                };
    
                // Parse the header (first 4 bytes)
                let header_bytes = &packet.data[0..4];
                let header = Header::try_from(header_bytes.to_vec()).unwrap();
    
                // Route answer based on input
                match header.msg_type() {
                    // Alive should update the TTL inside the peer map (probably)
                    MessageType::Alive => (),
                    MessageType::MemberReq => {
                        let msg = Message::<MemberRequest>::try_from(packet.data).unwrap();
                        let content = msg.content().unwrap();
                        let group_name = content.group_name();
                        let peer_id = content.peer_id();
    
                        let mut peer_map = handler_peer_map_lock.lock().unwrap();
    
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
    
                        recv_sock.send(TransportPacket { socket_addr: packet.socket_addr, data: res_msg.into() }).unwrap();
                    },
                    MessageType::MemberRes => {
                        let msg = Message::<MemberResponse>::try_from(packet.data).unwrap();
    
                        let content = msg.content().unwrap();
                        let peers = content.peers();
                        let group_name = content.group_name();
    
                        let mut peer_map = handler_peer_map_lock.lock().unwrap();
    
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

        if let Some(bootstrap) = self.bootstrap {
            self.send_req(bootstrap);
        }
        
        loop {
            match self.rx.recv() {
                Ok(cmd_str) => {
                    match cmd_str.trim() {
                        "peers" => {
                            println!("{:?}", self.peer_map.lock().unwrap());
                            continue;
                        },
                        "req" => {
                            for (_group, peer_list) in self.peer_map.lock().unwrap().iter() {
                                for peer in peer_list.iter() {
                                    self.send_req(peer.1);
                                }
                            }
                            
                            continue;
                        },
                        _ => (),
                    }

                    let header = Header::new(1, message::format::MessageType::Chat, cmd_str.len().try_into().unwrap());
                    for (_group, peer_list) in self.peer_map.lock().unwrap().iter() {
                        for peer in peer_list.iter() {
                            let chat = Chat::new(self.name.clone(), &cmd_str);
                            let msg = Message::<Chat>::new(header, Some(chat));
                            cmd_sock.send(TransportPacket {
                                socket_addr: peer.1,
                                data: msg.into(),
                            }).unwrap();
                        }
                    }
                },
                Err(err) => println!("Error on recv: {}", err),
            }
        }
    }
}
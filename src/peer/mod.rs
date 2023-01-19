use std::{net::SocketAddr, error::Error, sync::{Arc, Mutex, LockResult}, collections::HashMap, time::{Duration, Instant}, ops::Add};

use crossbeam_channel::{unbounded, Sender, Receiver};

use crate::{transport::{udp::UdpTransport, common::{TransportPacket, Transport}}, message::{format::{Message, Chat, Header, MessageType, MemberRequest, MemberResponse, Alive}, self}};

use self::structures::{PeerId, NeighbourMap, NeighbourEntry};

mod structures;

static TTL_RENEWAL: Duration = std::time::Duration::from_secs(30);
pub trait LockResultExt {
    type Guard;

    fn ignore_poison(self) -> Self::Guard;
}

impl<Guard> LockResultExt for LockResult<Guard> {
    type Guard = Guard;

    fn ignore_poison(self) -> Guard {
        self.unwrap_or_else(|e| e.into_inner())
    }
}

/// Instance of a peer. 
/// Encapsulates the neighbour map, network transport and manages
/// communication with other peers inside the group.
pub struct Peer {
    name: PeerId,
    group: String,
    port: u16,
    bootstrap: Option<SocketAddr>,
    transport: UdpTransport,
    tx: Sender<String>,
    rx: Receiver<String>,
    peer_map: Arc<Mutex<HashMap::<String, NeighbourMap>>>,

    msg_tx: Sender<(String, String)>,
    msg_rx: Receiver<(String, String)>,
}

impl Peer {
    pub fn new(name: String, group: String, port: u16, bootstrap: Option<SocketAddr>) -> Result<Peer, Box<dyn Error>> {
        let (tx, rx) = unbounded();
        let (msg_tx, msg_rx) = unbounded();
        let peer_map = Arc::new(Mutex::new(HashMap::new()));
        Ok(Peer {
            name,
            group,
            port,
            bootstrap,
            transport: UdpTransport::new(SocketAddr::new("0.0.0.0".parse().unwrap(), port)).unwrap(),
            rx, tx,
            peer_map,
            msg_tx, msg_rx,
        })
    }

    /// Returns a sender for sending commands or messages to the peer.
    pub fn msg_sender(&self) -> Sender<String> {
        self.tx.clone()
    }

    /// Returns the receiver for capturing messages from other peers.
    pub fn msg_receiver(&self) -> Receiver<(PeerId, String)> {
        self.msg_rx.clone()
    }

    fn send_req(&self, peer_socket: SocketAddr) -> Result<(), Box<dyn Error>> {
        let header = Header::new(1, message::format::MessageType::MemberReq, 64);
        let msg = Message::<MemberRequest>::new(header, Some(MemberRequest::new(&self.name.clone(), &self.group)?));
        let buf: Vec<u8> = msg.into();
        self.transport.send(TransportPacket {
            socket_addr: peer_socket,
            data: buf,
        })?;
        Ok(())
    }

    /// After calling this method, the current thread blocks
    /// The peer listens for incoming messages or commands, sends requests to other peers
    /// and maintains the connection with neighbours.
    pub fn run(&mut self) -> ! {
        let cmd_sock = self.transport.try_clone().unwrap();

        // Thread for sending the Alive message to all neighbours
        self.run_keep_alive_thread();

        // Handler thread for incoming packets
        self.run_message_handler_thread();

        if let Some(bootstrap) = self.bootstrap {
            self.send_req(bootstrap);
        }

        let cmd_sender = self.msg_tx.clone();
        
        // The main thread catches the incoming commands from the msg_sender
        loop {
            match self.rx.recv() {
                Ok(cmd_str) => {
                    // Matching special commands:
                    // peers - returns a list of all neighbours
                    // req - send a MemberRequest to all peers to discover newly added ones
                    match cmd_str.trim() {
                        "peers" => {
                            cmd_sender.send((self.name.clone(), format!("{:?}", self.peer_map.lock().ignore_poison()))).unwrap();
                            continue;
                        },
                        "req" => {
                            for (_group, peer_list) in self.peer_map.lock().ignore_poison().iter() {
                                for peer in peer_list.iter() {
                                    self.send_req(*peer.addr());
                                }
                            }
                            // Send to bootstrap since he has a stable address
                            // Although this fights the purpose of the bootstrap peer,
                            // it's easier and faster to get a more stable connection
                            // The proper way would be to introduce "stable peers"
                            if let Some(bootstrap) = self.bootstrap {
                                self.send_req(bootstrap);
                            }
                            continue;
                        },
                        _ => (),
                    }

                    let header = Header::new(1, message::format::MessageType::Chat, cmd_str.len().try_into().unwrap());
                    for (_group, peer_list) in self.peer_map.lock().ignore_poison().iter() {
                        for peer in peer_list.iter() {
                            let chat = Chat::new(self.name.clone(), &cmd_str);
                            let msg = Message::<Chat>::new(header, Some(chat));
                            cmd_sock.send(TransportPacket {
                                socket_addr: *peer.addr(),
                                data: msg.into(),
                            }).unwrap();
                        }
                    }
                },
                Err(err) => println!("Error on recv: {}", err),
            }
        }
    }

    fn run_keep_alive_thread(&self) -> std::thread::JoinHandle<()> {
        let peer_map_lock = self.peer_map.clone();

        let alive_sock = self.transport.try_clone().unwrap();
        let alive_packet = Alive::new(self.name.clone());
        // Thread for sending the Alive message to all neighbours
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(5));
                let mut peer_map = peer_map_lock.lock().ignore_poison();

                for (_, peer_list) in peer_map.iter_mut() {
                    peer_list.remove_expired();
                    for peer in peer_list.iter() {
                        let msg = Message::<Alive>::new(Header::new(1, MessageType::Alive, 0), Some(alive_packet.clone()));
                        // TODO: log error
                        alive_sock.send(TransportPacket { socket_addr: *peer.addr(), data: msg.into() });
                    }
                }
            }
        })
    }

    fn run_message_handler_thread(&self) -> std::thread::JoinHandle<()> {
        let peer_map_lock = self.peer_map.clone();
        let recv_sock = self.transport.try_clone().unwrap();
        let msg_sender = self.msg_tx.clone();

        // Handler thread for incoming packets
        std::thread::spawn(move || {
            loop {
                // Receive the packet
                let packet = match recv_sock.recv() {
                    Ok(p) => p,
                    Err(_err) => {
                        // TODO: log error
                        continue;
                    },
                };
    
                // Parse the header (first 4 bytes)
                let header_bytes = &packet.data[0..4];
                let header = match Header::try_from(header_bytes.to_vec()) {
                    Ok(h) => h,
                    Err(_) => {
                        // TODO: log error
                        continue;
                    },
                };
    
                // Route answer based on input
                match header.msg_type() {
                    // Alive should update the TTL inside the peer map
                    MessageType::Alive => {
                        let msg = match Message::<Alive>::try_from(packet.data) {
                            Ok(msg) => msg,
                            Err(_) => {
                                // TODO: log
                                continue;
                            },
                        };

                        let content = msg.content().unwrap();
                        let peer_id = content.peer_id();

                        let mut group_map = peer_map_lock.lock().ignore_poison();

                        for (_, peer_list) in group_map.iter_mut() {
                            if peer_list.contains_peer(peer_id) {
                                peer_list.find_peer_mut(peer_id).unwrap().update_ttl(TTL_RENEWAL);
                            }
                        }
                    },
                    MessageType::MemberReq => {
                        let msg = match Message::<MemberRequest>::try_from(packet.data) {
                            Ok(msg) => msg,
                            Err(_) => {
                                // TODO: log
                                continue;
                            }
                        };

                        let content = msg.content().unwrap();
                        let group_name = content.group_name();
                        let peer_id = content.peer_id();
                        
                        let mut group_map = peer_map_lock.lock().ignore_poison();
                        
                        if !group_map.contains_key(group_name) {
                            group_map.insert(group_name.to_string(), NeighbourMap::new());
                        }
                        
                        let peer_list = group_map.get_mut(group_name).unwrap();
                        
                        if !peer_list.contains_peer(&peer_id) {
                            // Initial TTL is set to 2 minutes
                            let ttl = Instant::now().add(TTL_RENEWAL.add(Duration::from_secs(120)));
                            peer_list.insert(NeighbourEntry::new(peer_id, packet.socket_addr, ttl));
                        }
                        
                        let peer_id = content.peer_id();
                        let response_peers = peer_list
                            //.clone()
                            .iter()
                            .filter(|s| *s.id() != peer_id.clone())
                            .map(|e| (e.id().clone(), *e.addr()))
                            .collect();
                            
                        let res_msg = Message::<MemberResponse>::new(
                            Header::new(1, MessageType::MemberRes, 0),
                            Some(MemberResponse::new(group_name, response_peers).unwrap())
                        );
                        recv_sock.send(TransportPacket { socket_addr: packet.socket_addr, data: res_msg.into() }).unwrap();
                    },
                    MessageType::MemberRes => {
                        let msg = match Message::<MemberResponse>::try_from(packet.data) {
                            Ok(msg) => msg,
                            Err(_) => {
                                // TODO: log
                                continue;
                            },
                        };

                        let content = msg.content().unwrap();
                        let peers = content.peers();
                        let group_name = content.group_name();
    
                        let mut peer_map = peer_map_lock.lock().ignore_poison();
                        
                        if !peer_map.contains_key(&group_name) {
                            peer_map.insert(group_name.clone(), NeighbourMap::new());
                        }
    
                        let peer_list = peer_map.get_mut(&group_name).unwrap();
                        
                        for (peer_id, peer_addr) in peers {
                            if !peer_list.contains_peer(peer_id) {
                                let ttl = Instant::now().add(TTL_RENEWAL);
                                peer_list.insert(NeighbourEntry::new(peer_id.to_string(), *peer_addr, ttl));
                            }
                        }
                    },
                    MessageType::Chat => {
                        let msg = match Message::<Chat>::try_from(packet.data) {
                            Ok(msg) => msg,
                            Err(_) => {
                                // TODO: log
                                continue;
                            },
                        };
                        let content = msg.content().unwrap();
                        msg_sender.send((content.peer_id(), content.msg().to_string())).unwrap();
                    },
                }
            }
        })
    }
}
use std::{net::{SocketAddr, UdpSocket}, collections::HashMap, sync::{Mutex, Arc}};

use clap::Parser;
use message::format::MemberResponse;

use crate::message::format::{Header, Message, MemberRequest, MessageType};

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
    
    let socket = UdpSocket::bind(SocketAddr::new("0.0.0.0".parse().unwrap(), args.port)).unwrap();

    let peer_map = Arc::new(Mutex::new(HashMap::<String, Vec<SocketAddr>>::new()));

    let tx_sock = socket.try_clone().unwrap();

    let res_sock = socket.try_clone().unwrap();

    let t = std::thread::spawn(move || {
        let mut buf: [u8; 1024] = [0; 1024];
        loop {
            let (bytes_read, peer) = socket.recv_from(&mut buf).unwrap();
            
            match MessageType::from(buf[1] & 0x0F) {
                MessageType::Alive => println!("alive"),
                MessageType::MemberReq => {
                    let msg = Message::<MemberRequest>::from(buf[..bytes_read].to_vec());
                    
                    let mut map = peer_map.lock().unwrap();
                    let grp_name = msg.content().group_name().unwrap(); 

                    if !map.contains_key(&grp_name) {
                        map.insert(grp_name.clone(), Vec::new());
                    }

                    let peer_list = map.get_mut(&grp_name).unwrap();

                    if !peer_list.contains(&peer) {
                        peer_list.push(peer);
                    }

                    let header = Header::new(1, message::format::MessageType::MemberRes, 10);
                    let response = Message::new(header, MemberResponse::new(&grp_name, (&peer_list).to_vec()).unwrap());
                    let res_buf: Vec<u8> = response.into();
                    res_sock.send_to(&res_buf[..], peer).unwrap();
                    println!("sent answer {}", grp_name);
                },
                MessageType::MemberRes => {
                    let msg = Message::<MemberResponse>::from(buf[..bytes_read].to_vec());
                    let mut map = peer_map.lock().unwrap();
                    let grp_name = msg.content().group_name().unwrap();
                    println!("got answer {}", grp_name);

                    if !map.contains_key(&grp_name) {
                        map.insert(grp_name.clone(), Vec::new());
                    }

                    let peer_list = map.get_mut(&grp_name).unwrap();
                    for grp_peer in msg.content().peers() {
                        if !peer_list.contains(&grp_peer) {
                            peer_list.push(grp_peer);
                        }
                    }

                    println!("odgovor {:?}", peer_list);
                },
                MessageType::Chat => todo!(),
            }
        }
    });

    std::thread::sleep(std::time::Duration::from_secs(2));

    if let Some(bootstrap) = args.bootstrap {
        let header = Header::new(1, message::format::MessageType::MemberReq, 10);
        let msg = Message::new(header, MemberRequest::new(&args.group.unwrap()).unwrap());
        let buf: Vec<u8> = msg.into();
        tx_sock.send_to(&buf[..], bootstrap).unwrap();
    }

    t.join().unwrap();
}

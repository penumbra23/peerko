use std::net::SocketAddr;

use clap::Parser;
use peer::peer::Peer;

mod transport;
mod message;
mod peer;

#[derive(Clone, Parser, Debug)]
struct CliArgs {
    #[clap(long, value_parser, short = 'n')]
    name: String,

    #[clap(long, value_parser, short = 'g')]
    group: String,

    #[clap(long, value_parser, short = 'p')]
    port: u16,

    #[clap(long, value_parser, short = 'b')]
    bootstrap: Option<SocketAddr>,
}

fn main() {
    let args = CliArgs::parse();
    
    let mut peer = Peer::new(args.name, args.group, args.port, args.bootstrap).unwrap();
    let peer_cmd = peer.tx();

    std::thread::spawn(move||{
        peer.run();
    });

    // Input thread for reading the input and sending to other peers
    loop {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        peer_cmd.send(line).unwrap();
    }
}
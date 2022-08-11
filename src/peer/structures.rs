use std::{net::SocketAddr, time::{Instant, Duration}, ops::Add, fmt::Debug};

/// ID of the peer. Needs to be unique for each peer on the group.
pub type PeerId = String;

#[derive(Clone, PartialEq, Eq)]
pub struct NeighbourEntry {
    id: String,
    addr: SocketAddr,
    ttl: Instant,
}

impl NeighbourEntry {
    pub fn new(id: String, addr: SocketAddr, ttl: Instant) -> NeighbourEntry {
        NeighbourEntry { id, addr, ttl }
    }

    pub fn id(&self) -> &String {
        &self.id
    }

    pub fn addr(&self) -> &SocketAddr {
        &self.addr
    }

    pub fn ttl_expired(&self) -> bool {
        let now = Instant::now();
        self.ttl < now
    }

    pub fn update_ttl(&mut self, value: Duration) {
        self.ttl = self.ttl.add(value);
    }
}

impl Debug for NeighbourEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}:{:?}", self.id, self.addr, self.ttl)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct NeighbourMap {
    peers: Vec<NeighbourEntry>,
}

impl NeighbourMap {
    pub fn new() -> NeighbourMap {
        NeighbourMap { peers: vec![] }
    }

    pub fn iter(&self) -> NeighbourMapIterator {
        NeighbourMapIterator { peers: &self.peers, index: 0 }
    }

    pub fn contains_peer(&self, peer_id: &str) -> bool {
        self.peers.iter().find(|&peer| peer.id == peer_id).is_some()
    }

    pub fn insert(&mut self, peer: NeighbourEntry) {
        self.peers.push(peer)
    }

    pub fn find_peer(&mut self, peer_id: &str) -> Option<&mut NeighbourEntry> {
        self.peers.iter_mut().find(|p| p.id == peer_id)
    }

    pub fn remove_expired(&mut self) {
        while let Some(peer_index) = self.peers.iter().position(|e| e.ttl_expired()) {
            self.peers.remove(peer_index);
        }
    }

}

impl Debug for NeighbourMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.peers)
    }
}

pub struct NeighbourMapIterator<'a> {
    index: usize,
    peers: &'a [NeighbourEntry],
}

impl<'a> Iterator for NeighbourMapIterator<'a> {
    type Item = &'a NeighbourEntry;

    fn next(&mut self) -> Option<Self::Item> {
        match self.peers.get(self.index) {
            Some(element) => {
                self.index += 1;
                Some(element)
            },
            None => None,
        }
    }
}
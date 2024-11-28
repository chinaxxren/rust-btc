use std::net::SocketAddr;
use std::time::SystemTime;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::network::message::Message;

#[derive(Debug)]
pub struct PeerInfo {
    pub addr: SocketAddr,
    pub version: u32,
    pub best_height: u32,
    pub last_seen: SystemTime,
}

impl PeerInfo {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            version: 0,
            best_height: 0,
            last_seen: SystemTime::now(),
        }
    }

    pub fn update_last_seen(&mut self) {
        self.last_seen = SystemTime::now();
    }
}

#[derive(Debug)]
pub struct Peer {
    pub info: PeerInfo,
    pub stream: TcpStream,
    pub sender: mpsc::Sender<Message>,
}

impl Peer {
    pub fn new(addr: SocketAddr, stream: TcpStream) -> Self {
        let (tx, _) = mpsc::channel::<Message>(32);
        Self {
            info: PeerInfo::new(addr),
            stream,
            sender: tx,
        }
    }
}

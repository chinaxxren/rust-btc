use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time;
use tracing::info;

use crate::error::Result;
use crate::network::message::Message;
use crate::network::peer::Peer;
use crate::storage::Storage;

pub struct P2PNetwork {
    peers: Arc<RwLock<HashMap<SocketAddr, Peer>>>,
    storage: Arc<Storage>,
    listen_addr: SocketAddr,
    message_receiver: Arc<Mutex<mpsc::Receiver<Message>>>,
    message_sender: mpsc::Sender<Message>,
}

impl P2PNetwork {
    pub async fn new(listen_addr: SocketAddr, storage: Arc<Storage>) -> Result<Arc<Self>> {
        let (tx, rx) = mpsc::channel::<Message>(32);
        
        let network = Arc::new(Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            storage,
            listen_addr,
            message_receiver: Arc::new(Mutex::new(rx)),
            message_sender: tx,
        });

        Ok(network)
    }

    pub async fn start(&self) -> Result<()> {
        info!("启动P2P网络节点: {}", self.listen_addr);
        
        // 创建TCP监听器
        let listener = TcpListener::bind(self.listen_addr).await?;
        
        // 开始监听连接
        while let Ok((stream, addr)) = listener.accept().await {
            info!("接受新连接: {}", addr);
            
            // 创建新的对等节点
            let peer = Peer::new(addr, stream);
            
            // 将对等节点添加到列表中
            self.peers.write().await.insert(addr, peer);
            
            info!("新节点已添加: {}", addr);
        }
        
        Ok(())
    }

    pub async fn connect_to_peer(&self, addr: SocketAddr) -> Result<()> {
        let stream = TcpStream::connect(addr).await?;
        self.handle_connection(stream, addr).await
    }

    pub async fn get_peer_addresses(&self) -> Vec<SocketAddr> {
        let peers = self.peers.read().await;
        peers.keys().cloned().collect()
    }

    async fn handle_connection(&self, stream: TcpStream, addr: SocketAddr) -> Result<()> {
        let peer = Peer::new(addr, stream);
        self.peers.write().await.insert(addr, peer);
        Ok(())
    }

    async fn maintain_peers(&self) {
        let mut interval = time::interval(Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            let mut peers = self.peers.write().await;
            
            // 移除断开连接的节点
            for (addr, peer) in peers.iter_mut() {
                if let Ok(elapsed) = peer.info.last_seen.elapsed() {
                    if elapsed > Duration::from_secs(3600) {
                        info!("节点 {} 超时断开", addr);
                    }
                }
            }
        }
    }

    pub async fn broadcast_message(&self, message: Message) -> Result<()> {
        let peers = self.peers.read().await;
        
        for peer in peers.values() {
            peer.sender.send(message.clone()).await.ok();
        }
        
        Ok(())
    }
}

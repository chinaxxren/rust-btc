use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use crate::block::Block;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // Node discovery messages
    Ping,
    Pong,
    GetPeers,
    Peers(Vec<SocketAddr>),
    Disconnect,

    // Block synchronization messages
    NewBlock(Block),
    GetBlock(u64), // height
    Block(Block),
    GetBlockHeight,
    BlockHeight(u64),

    // Mining related messages
    MiningSuccess(Block),
    VerifyBlock(Block),
    BlockVerified(bool),
}

impl Message {
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    pub fn deserialize(data: &[u8]) -> Option<Self> {
        bincode::deserialize(data).ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMessage {
    pub message: Message,
    pub from: SocketAddr,
    pub timestamp: u64,
}

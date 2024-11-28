use serde::{Deserialize, Serialize};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub version: i32,
    pub prev_block_hash: String,
    pub merkle_root: String,
    pub timestamp: u64,
    pub bits: u32,
    pub nonce: u32,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInput {
    pub txid: String,
    pub vout: u32,
    pub script_sig: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutput {
    pub value: u64,
    pub script_pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletData {
    pub address: String,
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UTXOEntry {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
    pub script_pubkey: String,
    pub address: String,
}

impl Block {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self)
            .map_err(|e| e.into())
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data)
            .map_err(|e| e.into())
    }
}

impl WalletData {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self)
            .map_err(|e| e.into())
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data)
            .map_err(|e| e.into())
    }
}

impl UTXOEntry {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self)
            .map_err(|e| e.into())
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data)
            .map_err(|e| e.into())
    }
}

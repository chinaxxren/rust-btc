use chrono::Utc;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use std::error::Error;
use crate::transaction::Transaction;

const MINING_DIFFICULTY: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub timestamp: i64,
    pub transactions: Vec<Transaction>,
    pub prev_block_hash: String,
    pub hash: String,
    pub nonce: i32,
}

impl Block {
    pub fn new(transactions: Vec<Transaction>, prev_block_hash: String) -> Result<Block, Box<dyn Error>> {
        let mut block = Block {
            timestamp: Utc::now().timestamp(),
            transactions,
            prev_block_hash,
            hash: String::new(),
            nonce: 0,
        };
        
        block.set_hash()?;
        Ok(block)
    }
    
    pub fn set_hash(&mut self) -> Result<(), Box<dyn Error>> {
        let mut hasher = Sha256::new();
        
        // 计算区块头的哈希
        let encoded = bincode::serialize(&(
            self.timestamp,
            &self.transactions,
            &self.prev_block_hash,
            self.nonce
        ))?;
        
        hasher.update(&encoded);
        self.hash = hex::encode(hasher.finalize());
        Ok(())
    }
    
    pub fn mine_block(&mut self, difficulty: usize) -> Result<(), Box<dyn Error>> {
        println!("开始挖矿...");
        let target = "0".repeat(difficulty);
        
        while &self.hash[..difficulty] != target {
            self.nonce += 1;
            self.set_hash()?;
        }
        
        println!("找到新区块! 哈希: {}", self.hash);
        Ok(())
    }
    
    pub fn get_bytes(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut bytes = Vec::new();
        bytes.extend(&self.timestamp.to_be_bytes());
        
        for transaction in &self.transactions {
            bytes.extend(bincode::serialize(transaction)?);
        }
        
        bytes.extend(self.prev_block_hash.as_bytes());
        bytes.extend(self.hash.as_bytes());
        bytes.extend(&self.nonce.to_be_bytes());
        
        Ok(bytes)
    }

    pub fn new_genesis_block(coinbase: Transaction) -> Result<Block, Box<dyn Error>> {
        Block::new(vec![coinbase], "0".to_string())
    }
}

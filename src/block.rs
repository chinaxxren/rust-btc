use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use crate::transaction::Transaction;

const MINING_DIFFICULTY: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub timestamp: u64,
    pub transactions: Vec<Transaction>,
    pub prev_block_hash: String,
    pub hash: String,
    pub nonce: u64,
}

impl Block {
    pub fn new(transactions: Vec<Transaction>, prev_block_hash: String) -> Result<Block, Box<dyn Error>> {
        let mut block = Block {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs(),
            transactions,
            prev_block_hash,
            hash: String::new(),
            nonce: 0,
        };
        
        block.mine_block(MINING_DIFFICULTY)?;
        Ok(block)
    }
    
    pub fn new_genesis_block(miner_address: &str) -> Result<Block, Box<dyn Error>> {
        let coinbase = Transaction::new_coinbase(miner_address, "Genesis Block")?;
        Block::new(vec![coinbase], String::from("0"))
    }
    
    pub fn mine_block(&mut self, difficulty: usize) -> Result<(), Box<dyn Error>> {
        let target = "0".repeat(difficulty);
        
        while {
            self.hash = self.calculate_hash()?;
            !self.hash.starts_with(&target)
        } {
            self.nonce += 1;
        }
        
        Ok(())
    }
    
    fn calculate_hash(&self) -> Result<String, Box<dyn Error>> {
        let mut hasher = Sha256::new();
        hasher.update(self.prev_block_hash.as_bytes());
        hasher.update(&self.timestamp.to_be_bytes());
        
        // 序列化交易并更新哈希
        let tx_data = bincode::serialize(&self.transactions)?;
        hasher.update(&tx_data);
        
        hasher.update(&self.nonce.to_be_bytes());
        
        Ok(format!("{:x}", hasher.finalize()))
    }
}

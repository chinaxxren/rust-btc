use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

use ring::digest;
use serde::{Deserialize, Serialize};

use crate::transaction::Transaction;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Block {
    pub timestamp: u64,
    pub transactions: Vec<Transaction>,
    pub prev_block_hash: String,
    pub hash: String,
    pub nonce: u64,
}

impl Block {
    pub fn new(transactions: Vec<Transaction>, prev_block_hash: String) -> Result<Block, Box<dyn Error>> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();

        let mut block = Block {
            timestamp,
            transactions,
            prev_block_hash,
            hash: String::new(),
            nonce: 0,
        };

        block.hash = block.calculate_hash()?;
        Ok(block)
    }
    
    pub fn new_genesis_block(miner_address: &str) -> Result<Block, Box<dyn Error>> {
        let coinbase = Transaction::new_coinbase(miner_address, "Genesis Block")?;
        Block::new(vec![coinbase], String::from("0"))
    }
    
    pub fn mine_block(&mut self, difficulty: usize) -> Result<(), Box<dyn Error>> {
        let target = "0".repeat(difficulty);
        println!("Mining block...");
        while !self.hash.starts_with(&target) {
            self.nonce += 1;
            self.hash = self.calculate_hash()?;
        }
        println!("Block mined! Nonce: {}, Hash: {}", self.nonce, self.hash);
        
        Ok(())
    }
    
    fn calculate_hash(&self) -> Result<String, Box<dyn Error>> {
        let mut hasher = digest::Context::new(&digest::SHA256);
        
        let tx_data = bincode::serialize(&self.transactions)?;
        let data = format!(
            "{}{}",
            self.prev_block_hash,
            self.timestamp,
        );
        
        hasher.update(data.as_bytes());
        hasher.update(&tx_data);
        hasher.update(&self.nonce.to_be_bytes());
        
        let hash = hasher.finish();
        Ok(hex::encode(hash.as_ref()))
    }
    
    pub fn get_transactions(&self) -> &Vec<Transaction> {
        &self.transactions
    }
}

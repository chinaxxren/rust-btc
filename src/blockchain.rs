use std::error::Error;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::block::Block;
use crate::transaction::Transaction;

const BLOCKCHAIN_FILE: &str = "data/blockchain.dat";

#[derive(Debug, Serialize, Deserialize)]
pub struct Blockchain {
    blocks: Vec<Block>,
}

impl Blockchain {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        if let Ok(data) = fs::read(BLOCKCHAIN_FILE) {
            if let Ok(blockchain) = bincode::deserialize(&data) {
                return Ok(blockchain);
            }
        }

        Ok(Blockchain { blocks: Vec::new() })
    }

    pub fn add_block(&mut self, block: Block) -> Result<(), Box<dyn Error>> {
        self.blocks.push(block);
        self.save_to_file()?;
        Ok(())
    }

    pub fn get_last_hash(&self) -> Result<String, Box<dyn Error>> {
        Ok(self.blocks.last().map_or("0".to_string(), |block| block.hash.clone()))
    }

    pub fn height(&self) -> usize {
        self.blocks.len()
    }

    pub fn blocks(&self) -> Result<&[Block], Box<dyn Error>> {
        Ok(&self.blocks)
    }

    pub fn save_to_file(&self) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(self)?;
        fs::write(BLOCKCHAIN_FILE, data)?;
        Ok(())
    }

    pub fn load_from_file() -> Result<Self, Box<dyn Error>> {
        let data = fs::read(BLOCKCHAIN_FILE)?;
        let blockchain = bincode::deserialize(&data)?;
        Ok(blockchain)
    }

    pub fn find_transaction(&self, id: &str) -> Option<Transaction> {
        for block in &self.blocks {
            for tx in &block.transactions {
                if tx.id == id {
                    return Some(tx.clone());
                }
            }
        }
        None
    }
}

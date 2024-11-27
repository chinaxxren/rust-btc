use std::error::Error;
use std::fs;
use std::path::Path;
use bincode;

use crate::block::Block;
use crate::transaction::Transaction;
use crate::utxo::UTXOSet;

const BLOCKCHAIN_FILE: &str = "data/blockchain.db";
const MINING_DIFFICULTY: usize = 4;

pub struct Blockchain {
    blocks: Vec<Block>,
    current_hash: String,
}

impl Blockchain {
    pub fn new(miner_address: &str) -> Result<Blockchain, Box<dyn Error>> {
        let blocks = if let Ok(data) = fs::read(BLOCKCHAIN_FILE) {
            bincode::deserialize(&data)?
        } else {
            // 创建创世区块
            let genesis_block = Block::new_genesis_block(miner_address)?;
            vec![genesis_block]
        };
        
        let current_hash = blocks.last()
            .map(|b| b.hash.clone())
            .unwrap_or_default();
            
        let blockchain = Blockchain {
            blocks,
            current_hash,
        };
        
        blockchain.save()?;
        Ok(blockchain)
    }
    
    pub fn add_block(&mut self, transactions: Vec<Transaction>) -> Result<(), Box<dyn Error>> {
        let prev_hash = self.current_hash.clone();
        let mut new_block = Block::new(transactions, prev_hash)?;
        new_block.mine_block(MINING_DIFFICULTY)?;
        self.current_hash = new_block.hash.clone();
        self.blocks.push(new_block);
        self.save()
    }
    
    pub fn blocks(&self) -> Result<&Vec<Block>, Box<dyn Error>> {
        Ok(&self.blocks)
    }
    
    pub fn find_transaction(&self, id: &str) -> Result<Option<Transaction>, Box<dyn Error>> {
        for block in self.blocks.iter().rev() {
            for tx in &block.transactions {
                if tx.id == id {
                    return Ok(Some(tx.clone()));
                }
            }
        }
        Ok(None)
    }
    
    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(&self.blocks)?;
        if let Some(parent) = Path::new(BLOCKCHAIN_FILE).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(BLOCKCHAIN_FILE, data)?;
        Ok(())
    }
    
    pub fn cleanup() -> Result<(), Box<dyn Error>> {
        if Path::new(BLOCKCHAIN_FILE).exists() {
            fs::remove_file(BLOCKCHAIN_FILE)?;
        }
        Ok(())
    }
}

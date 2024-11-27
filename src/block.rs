use std::time::{SystemTime, UNIX_EPOCH};

use ring::digest;
use serde::{Deserialize, Serialize};
use tracing::{info, error, debug};

use crate::error::{Result, RustBtcError};
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
    pub fn new(transactions: Vec<Transaction>, prev_block_hash: String) -> Result<Block> {
        debug!("创建新区块，前置哈希: {}", prev_block_hash);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| RustBtcError::TimestampError(e.to_string()))?
            .as_secs();

        let mut block = Block {
            timestamp,
            transactions,
            prev_block_hash,
            hash: String::new(),
            nonce: 0,
        };

        block.hash = block.calculate_hash()?;
        info!("新区块创建成功，哈希: {}", block.hash);
        Ok(block)
    }
    
    pub fn new_genesis_block(miner_address: &str) -> Result<Block> {
        info!("创建创世区块，矿工地址: {}", miner_address);
        let coinbase = Transaction::new_coinbase(miner_address, "Genesis Block")
            .map_err(|e| RustBtcError::TransactionError(e.to_string()))?;
        let block = Block::new(vec![coinbase], String::from("0"))?;
        info!("创世区块创建成功，哈希: {}", block.hash);
        Ok(block)
    }
    
    pub fn mine_block(&mut self, difficulty: usize) -> Result<()> {
        let target = "0".repeat(difficulty);
        info!("开始挖矿，难度: {}", difficulty);
        debug!("目标前缀: {}", target);
        
        let mut attempts = 0;
        while !self.hash.starts_with(&target) {
            self.nonce += 1;
            attempts += 1;
            self.hash = self.calculate_hash()?;
            
            if attempts % 100000 == 0 {
                debug!("挖矿尝试次数: {}, 当前nonce: {}", attempts, self.nonce);
            }
        }
        
        info!("区块已挖出！Nonce: {}, Hash: {}", self.nonce, self.hash);
        Ok(())
    }

    pub fn calculate_hash(&self) -> Result<String> {
        let data = bincode::serialize(self)
            .map_err(|e| RustBtcError::SerializationError(e.to_string()))?;
            
        let hash = digest::digest(&digest::SHA256, &data);
        let hash_str = hex::encode(hash.as_ref());
        Ok(hash_str)
    }

    pub fn get_transactions(&self) -> &Vec<Transaction> {
        &self.transactions
    }

    pub fn verify_hash(&self) -> Result<bool> {
        debug!("验证区块哈希: {}", self.hash);
        let calculated_hash = self.calculate_hash()?;
        if calculated_hash != self.hash {
            error!("区块哈希验证失败，存储的哈希: {}, 计算的哈希: {}", 
                self.hash, calculated_hash);
            return Ok(false);
        }
        debug!("区块哈希验证通过");
        Ok(true)
    }

    pub fn is_valid(&self) -> Result<bool> {
        debug!("开始验证区块...");
        
        // 验证时间戳
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| RustBtcError::TimestampError(e.to_string()))?
            .as_secs();
            
        if self.timestamp > current_time {
            error!("区块时间戳 {} 大于当前时间 {}", self.timestamp, current_time);
            return Ok(false);
        }

        // 验证交易
        if self.transactions.is_empty() {
            error!("区块不包含任何交易");
            return Ok(false);
        }

        // 验证第一笔交易是否为coinbase交易
        if !self.transactions[0].is_coinbase() {
            error!("区块的第一笔交易不是coinbase交易");
            return Ok(false);
        }

        // 验证所有交易
        for (i, tx) in self.transactions.iter().enumerate() {
            debug!("验证第 {} 笔交易: {}", i, tx.id);
            if !tx.verify_transaction_data()? {
                error!("交易 {} 数据验证失败", tx.id);
                return Ok(false);
            }
        }

        info!("区块验证通过");
        Ok(true)
    }

    pub fn hash(&self) -> Result<String> {
        debug!("获取区块哈希: {}", self.hash);
        Ok(self.hash.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::wallet::Wallet;

    #[test]
    fn test_block_creation_and_mining() -> Result<()> {
        let wallet = Wallet::new().map_err(|e| RustBtcError::TransactionError(e.to_string()))?;
        let coinbase_tx = Transaction::new_coinbase(&wallet.get_address(), "Test Block")
            .map_err(|e| RustBtcError::TransactionError(e.to_string()))?;
        
        let mut block = Block::new(vec![coinbase_tx], String::new())?;
        block.mine_block(4)?;
        
        assert!(block.hash.starts_with("0000"), "Block should be mined with difficulty 4");
        assert!(block.is_valid()?, "Block should be valid");
        
        Ok(())
    }

    #[test]
    fn test_invalid_block() -> Result<()> {
        // 创建无效区块（没有交易）
        let block = Block::new(vec![], String::new())?;
        assert!(!block.is_valid().is_ok(), "Empty block should be invalid");
        
        Ok(())
    }

    #[test]
    fn test_genesis_block() -> Result<()> {
        let wallet = Wallet::new().map_err(|e| RustBtcError::TransactionError(e.to_string()))?;
        let block = Block::new_genesis_block(&wallet.get_address())?;
        
        assert!(block.is_valid()?, "Genesis block should be valid");
        assert_eq!(block.prev_block_hash, "0", "Genesis block should have '0' as previous hash");
        assert_eq!(block.transactions.len(), 1, "Genesis block should have exactly one transaction");
        assert!(block.transactions[0].is_coinbase(), "Genesis block should contain a coinbase transaction");
        
        Ok(())
    }
}

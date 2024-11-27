use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

use ring::digest;
use serde::{Deserialize, Serialize};

use crate::transaction::Transaction;

#[derive(Debug)]
pub enum BlockError {
    ValidationError(String),
    HashError(String),
    TimestampError(String),
    TransactionError(String),
}

impl std::error::Error for BlockError {}

impl std::fmt::Display for BlockError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BlockError::ValidationError(msg) => write!(f, "区块验证错误: {}", msg),
            BlockError::HashError(msg) => write!(f, "哈希错误: {}", msg),
            BlockError::TimestampError(msg) => write!(f, "时间戳错误: {}", msg),
            BlockError::TransactionError(msg) => write!(f, "交易错误: {}", msg),
        }
    }
}

type Result<T> = std::result::Result<T, BlockError>;

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
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| BlockError::TimestampError(e.to_string()))?
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
    
    pub fn new_genesis_block(miner_address: &str) -> Result<Block> {
        let coinbase = Transaction::new_coinbase(miner_address, "Genesis Block")
            .map_err(|e| BlockError::TransactionError(e.to_string()))?;
        Block::new(vec![coinbase], String::from("0"))
    }
    
    pub fn mine_block(&mut self, difficulty: usize) -> Result<()> {
        let target = "0".repeat(difficulty);
        println!("Mining block...");
        while !self.hash.starts_with(&target) {
            self.nonce += 1;
            self.hash = self.calculate_hash()?;
        }
        println!("Block mined! Nonce: {}, Hash: {}", self.nonce, self.hash);
        
        Ok(())
    }

    pub fn calculate_hash(&self) -> Result<String> {
        let mut hasher = digest::Context::new(&digest::SHA256);
        
        // 添加时间戳
        hasher.update(&self.timestamp.to_be_bytes());
        
        // 添加前一个区块的哈希
        hasher.update(self.prev_block_hash.as_bytes());
        
        // 添加交易
        for tx in &self.transactions {
            let tx_hash = tx.hash().map_err(|e| BlockError::HashError(e.to_string()))?;
            hasher.update(tx_hash.as_bytes());
        }
        
        // 添加nonce
        hasher.update(&self.nonce.to_be_bytes());
        
        let hash = hasher.finish();
        Ok(hex::encode(hash.as_ref()))
    }

    pub fn get_transactions(&self) -> &Vec<Transaction> {
        &self.transactions
    }

    pub fn is_valid(&self) -> Result<bool> {
        // 验证时间戳
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| BlockError::TimestampError(e.to_string()))?
            .as_secs();
        
        if self.timestamp > current_time {
            return Err(BlockError::ValidationError("区块时间戳在未来".to_string()));
        }

        // 验证哈希
        let calculated_hash = self.calculate_hash()?;
        if calculated_hash != self.hash {
            return Err(BlockError::ValidationError(format!(
                "区块哈希不匹配，计算得到: {}, 实际: {}",
                calculated_hash, self.hash
            )));
        }

        // 验证交易
        if self.transactions.is_empty() {
            return Err(BlockError::ValidationError("区块不包含任何交易".to_string()));
        }

        // 验证第一笔交易是否为coinbase交易
        if !self.transactions[0].is_coinbase() {
            return Err(BlockError::ValidationError("第一笔交易不是coinbase交易".to_string()));
        }

        // 验证其他交易
        for tx in self.transactions.iter().skip(1) {
            if tx.is_coinbase() {
                return Err(BlockError::ValidationError("非首笔交易不能是coinbase交易".to_string()));
            }
        }

        Ok(true)
    }

    pub fn hash(&self) -> Result<String> {
        Ok(self.hash.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::Wallet;

    #[test]
    fn test_block_creation_and_mining() -> Result<()> {
        let wallet = Wallet::new().map_err(|e| BlockError::TransactionError(e.to_string()))?;
        let coinbase_tx = Transaction::new_coinbase(&wallet.get_address(), "Test Block")
            .map_err(|e| BlockError::TransactionError(e.to_string()))?;
        
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
        let wallet = Wallet::new().map_err(|e| BlockError::TransactionError(e.to_string()))?;
        let block = Block::new_genesis_block(&wallet.get_address())?;
        
        assert!(block.is_valid()?, "Genesis block should be valid");
        assert_eq!(block.prev_block_hash, "0", "Genesis block should have '0' as previous hash");
        assert_eq!(block.transactions.len(), 1, "Genesis block should have exactly one transaction");
        assert!(block.transactions[0].is_coinbase(), "Genesis block should contain a coinbase transaction");
        
        Ok(())
    }
}

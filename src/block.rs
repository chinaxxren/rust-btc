use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use hex;
use tracing::{info, error, debug};

use crate::error::{Result, RustBtcError};
use crate::transaction::Transaction;
use crate::utxo::UTXOSet;

const MINING_DIFFICULTY: usize = 4;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Block {
    pub version: i32,
    pub timestamp: u64,
    pub transactions: Vec<Transaction>,
    pub prev_block_hash: String,
    pub merkle_root: String,
    pub hash: String,
    pub nonce: u64,
    pub height: u64,
    pub bits: u32,
}

impl Block {
    pub fn new(transactions: Vec<Transaction>, prev_block_hash: String) -> Result<Block> {
        debug!("创建新区块，前置哈希: {}", prev_block_hash);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| RustBtcError::TimestampError(e))?
            .as_secs();

        let merkle_root = Self::calculate_merkle_root(&transactions)?;
        
        let mut block = Block {
            version: 1,
            timestamp,
            transactions,
            prev_block_hash,
            merkle_root,
            hash: String::new(),
            nonce: 0,
            height: 0,
            bits: 0x1d00ffff, // Default difficulty bits
        };

        block.hash = block.calculate_hash()?;
        info!("新区块创建成功，哈希: {}", block.hash);
        Ok(block)
    }

    fn calculate_merkle_root(transactions: &[Transaction]) -> Result<String> {
        if transactions.is_empty() {
            return Ok(String::from("0000000000000000000000000000000000000000000000000000000000000000"));
        }

        let mut hashes: Vec<String> = transactions
            .iter()
            .map(|tx| tx.hash())
            .collect::<Result<_>>()?;

        while hashes.len() > 1 {
            let mut new_hashes = Vec::new();
            for chunk in hashes.chunks(2) {
                let mut hasher = Sha256::new();
                hasher.update(chunk[0].as_bytes());
                if chunk.len() > 1 {
                    hasher.update(chunk[1].as_bytes());
                } else {
                    hasher.update(chunk[0].as_bytes()); // If odd number, duplicate the last hash
                }
                let result = hex::encode(hasher.finalize());
                new_hashes.push(result);
            }
            hashes = new_hashes;
        }

        Ok(hashes[0].clone())
    }

    pub fn new_genesis_block(address: &str) -> Result<Block> {
        let coinbase = Transaction::new_coinbase(address, "Genesis Block")?;
        let transactions = vec![coinbase];
        let merkle_root = Self::calculate_merkle_root(&transactions)?;
        
        let mut block = Block {
            version: 1,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs(),
            transactions: transactions.clone(),
            prev_block_hash: String::from("0"),
            merkle_root,
            nonce: 0,
            hash: String::new(),
            height: 0,
            bits: 0x1d00ffff, // Default difficulty bits
        };
        
        block.mine_block(MINING_DIFFICULTY)?;
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
            .map_err(|e| RustBtcError::Serialization(e))?;
            
        let hash = Sha256::digest(&data);
        let hash_str = hex::encode(hash);
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
            .map_err(|e| RustBtcError::TimestampError(e))?
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

    pub fn validate(&self, utxo_set: &UTXOSet) -> Result<bool> {
        // 检查区块是否有交易
        if self.transactions.is_empty() {
            debug!("区块没有交易");
            return Ok(false);
        }

        // 检查第一个交易是否是 coinbase 交易
        if !self.transactions[0].is_coinbase() {
            debug!("第一个交易不是 coinbase 交易");
            return Ok(false);
        }

        // 验证区块哈希
        let hash = self.calculate_hash()?;
        if hash != self.hash {
            debug!("区块哈希不匹配");
            return Ok(false);
        }

        // 验证所有交易
        for tx in self.transactions.iter() {
            if !tx.verify(utxo_set)? {
                debug!("交易验证失败");
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub fn is_genesis(&self) -> bool {
        self.prev_block_hash == "0"
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::Wallet;

    fn create_test_wallet() -> Result<Wallet> {
        Wallet::new()
    }

    fn create_test_block(prev_hash: &str, nonce: u64) -> Result<Block> {
        let wallet = create_test_wallet()?;
        let address = wallet.get_address();
        let coinbase = Transaction::new_coinbase(&address, "Test Block")?;
        
        let mut block = Block {
            version: 1,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            transactions: vec![coinbase],
            prev_block_hash: prev_hash.to_string(),
            merkle_root: Block::calculate_merkle_root(&vec![coinbase])?,
            hash: String::new(),
            nonce,
            height: 0,
            bits: 0x1d00ffff, // Default difficulty bits
        };
        
        block.hash = block.calculate_hash()?;
        Ok(block)
    }

    #[test]
    fn test_block_creation_and_mining() -> Result<()> {
        let block = create_test_block("test_prev_hash", 0)?;
        
        // 验证区块字段
        assert!(!block.transactions.is_empty());
        assert_eq!(block.prev_block_hash, "test_prev_hash");
        assert_eq!(block.height, 0);
        
        // 验证挖矿
        let mut mining_block = block.clone();
        mining_block.mine_block(4)?;
        assert!(mining_block.validate(&UTXOSet::new())?);
        
        Ok(())
    }

    #[test]
    fn test_genesis_block() -> Result<()> {
        let wallet = create_test_wallet()?;
        let genesis = Block::new_genesis_block(&wallet.get_address())?;
        
        // 验证创世区块
        assert!(genesis.validate(&UTXOSet::new())?);
        assert!(genesis.is_genesis());
        assert_eq!(genesis.height, 0);
        assert_eq!(genesis.prev_block_hash, "0");
        
        Ok(())
    }

    #[test]
    fn test_invalid_block() -> Result<()> {
        // 创建一个无效区块（没有交易）
        let mut invalid_block = Block {
            version: 1,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            transactions: vec![],
            prev_block_hash: "test_prev_hash".to_string(),
            merkle_root: Block::calculate_merkle_root(&vec![])?,
            hash: String::new(),
            nonce: 0,
            height: 0,
            bits: 0x1d00ffff, // Default difficulty bits
        };
        
        invalid_block.hash = invalid_block.calculate_hash()?;
        assert!(!invalid_block.validate(&UTXOSet::new())?);
        
        Ok(())
    }
}

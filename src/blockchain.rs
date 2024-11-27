use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::block::{Block, BlockError};
use crate::transaction::Transaction;
use crate::utxo::UTXOSet;
use crate::wallet::Wallet;

const MAX_BLOCK_SIZE: usize = 1_000_000; // 1MB
const MAX_CHAIN_LENGTH: usize = 1_000_000;

#[derive(Debug)]
pub enum BlockchainError {
    SerializationError(String),
    ValidationError(String),
    InvalidBlock(String),
    InvalidChain(String),
    BlockNotFound(String),
    TransactionError(String),
    UTXOError(String),
    IOError(String),
}

impl fmt::Display for BlockchainError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BlockchainError::SerializationError(msg) => write!(f, "序列化错误: {}", msg),
            BlockchainError::ValidationError(msg) => write!(f, "验证错误: {}", msg),
            BlockchainError::InvalidBlock(msg) => write!(f, "无效区块: {}", msg),
            BlockchainError::InvalidChain(msg) => write!(f, "无效区块链: {}", msg),
            BlockchainError::BlockNotFound(msg) => write!(f, "区块未找到: {}", msg),
            BlockchainError::TransactionError(msg) => write!(f, "交易错误: {}", msg),
            BlockchainError::UTXOError(msg) => write!(f, "UTXO错误: {}", msg),
            BlockchainError::IOError(msg) => write!(f, "IO错误: {}", msg),
        }
    }
}

impl Error for BlockchainError {}

impl From<BlockError> for BlockchainError {
    fn from(error: BlockError) -> Self {
        match error {
            BlockError::ValidationError(msg) => BlockchainError::ValidationError(msg),
            BlockError::HashError(msg) => BlockchainError::ValidationError(msg),
            BlockError::TimestampError(msg) => BlockchainError::ValidationError(msg),
            BlockError::TransactionError(msg) => BlockchainError::TransactionError(msg),
        }
    }
}

impl From<Box<dyn Error>> for BlockchainError {
    fn from(error: Box<dyn Error>) -> Self {
        BlockchainError::ValidationError(error.to_string())
    }
}

type Result<T> = std::result::Result<T, BlockchainError>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Blockchain {
    blocks: Vec<Block>,
    current_hash: String,
}

impl Blockchain {
    pub fn new() -> Result<Self> {
        Ok(Self {
            blocks: Vec::new(),
            current_hash: String::new(),
        })
    }

    pub fn add_block(&mut self, block: Block) -> Result<()> {
        // 验证区块大小
        let block_size = bincode::serialize(&block)
            .map_err(|e| BlockchainError::SerializationError(e.to_string()))?
            .len();
        if block_size > MAX_BLOCK_SIZE {
            return Err(BlockchainError::InvalidBlock(format!(
                "区块大小 {} 超过最大限制 {}",
                block_size, MAX_BLOCK_SIZE
            )));
        }

        // 验证链长度
        if self.blocks.len() >= MAX_CHAIN_LENGTH {
            return Err(BlockchainError::InvalidChain(format!(
                "区块链长度 {} 超过最大限制 {}",
                self.blocks.len(), MAX_CHAIN_LENGTH
            )));
        }

        // 验证区块哈希
        if !self.current_hash.is_empty() && block.prev_block_hash != self.current_hash {
            return Err(BlockchainError::InvalidBlock(format!(
                "区块的前一个哈希 {} 与当前哈希 {} 不匹配",
                block.prev_block_hash, self.current_hash
            )));
        }

        // 验证区块
        if !block.is_valid().map_err(|e| BlockchainError::ValidationError(e.to_string()))? {
            return Err(BlockchainError::InvalidBlock("区块验证失败".to_string()));
        }

        // 验证区块中的所有交易
        for tx in &block.transactions {
            if !tx.verify(&UTXOSet::new())? {
                return Err(BlockchainError::InvalidBlock(format!(
                    "区块中的交易 {} 验证失败",
                    tx.hash()?
                )));
            }
        }

        // 更新当前哈希和区块
        self.current_hash = block.hash()
            .map_err(|e| BlockchainError::ValidationError(e.to_string()))?;
        self.blocks.push(block);

        Ok(())
    }

    pub fn get_block(&self, hash: &str) -> Result<&Block> {
        self.blocks
            .iter()
            .find(|block| block.hash().map(|h| h == hash).unwrap_or(false))
            .ok_or_else(|| BlockchainError::BlockNotFound(hash.to_string()))
    }

    pub fn get_last_hash(&self) -> Result<String> {
        if self.blocks.is_empty() {
            Ok(String::new())
        } else {
            Ok(self.current_hash.clone())
        }
    }

    pub fn save_to_file(&self) -> Result<()> {
        let data = bincode::serialize(&self)
            .map_err(|e| BlockchainError::SerializationError(e.to_string()))?;
        fs::write("blockchain.dat", data)
            .map_err(|e| BlockchainError::IOError(e.to_string()))?;
        Ok(())
    }

    pub fn load_from_file() -> Result<Self> {
        if !Path::new("blockchain.dat").exists() {
            return Ok(Self::new()?);
        }

        let data = fs::read("blockchain.dat")
            .map_err(|e| BlockchainError::IOError(e.to_string()))?;
        let blockchain = bincode::deserialize(&data)
            .map_err(|e| BlockchainError::SerializationError(e.to_string()))?;
        Ok(blockchain)
    }

    pub fn validate_chain(&self) -> Result<bool> {
        // 验证所有区块
        for (i, block) in self.blocks.iter().enumerate() {
            // 验证区块
            if !block.is_valid()? {
                return Err(BlockchainError::InvalidBlock(format!(
                    "区块 {} 验证失败",
                    i
                )));
            }

            // 验证区块哈希链接
            if i > 0 {
                let prev_block = &self.blocks[i - 1];
                if block.prev_block_hash != prev_block.hash()? {
                    return Err(BlockchainError::InvalidChain(format!(
                        "区块 {} 的前一个哈希与区块 {} 的哈希不匹配: {} != {}",
                        i,
                        i - 1,
                        block.prev_block_hash,
                        prev_block.hash()?
                    )));
                }
            }

            // 验证区块中的所有交易
            for tx in &block.transactions {
                if !tx.verify(&UTXOSet::new())
                    .map_err(|e| BlockchainError::TransactionError(e.to_string()))? {
                    return Err(BlockchainError::InvalidBlock(format!(
                        "区块 {} 中的交易 {} 验证失败",
                        i,
                        tx.hash().map_err(|e| BlockchainError::TransactionError(e.to_string()))?
                    )));
                }
            }
        }

        Ok(true)
    }

    pub fn get_block_height(&self) -> usize {
        self.blocks.len()
    }

    pub fn get_blocks_after(&self, hash: &str) -> Result<Vec<&Block>> {
        let start_index = self
            .blocks
            .iter()
            .position(|block| block.hash().map(|h| h == hash).unwrap_or(false))
            .ok_or_else(|| BlockchainError::BlockNotFound(hash.to_string()))?;

        Ok(self.blocks[start_index + 1..].iter().collect())
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

    pub fn blocks(&self) -> &Vec<Block> {
        &self.blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::Wallet;

    #[test]
    fn test_blockchain_basic_operations() -> Result<()> {
        let mut blockchain = Blockchain::new()?;
        assert_eq!(blockchain.get_block_height(), 0);

        // 创建创世区块
        let wallet = Wallet::new()?;
        let coinbase_tx = Transaction::new_coinbase(&wallet.get_address(), "Genesis Block")?;
        let mut genesis_block = Block::new(vec![coinbase_tx], String::new())?;
        genesis_block.mine_block(4)?;

        // 添加创世区块
        blockchain.add_block(genesis_block.clone())?;
        assert_eq!(blockchain.get_block_height(), 1);

        // 验证区块检索
        let hash = genesis_block.hash()?;
        let retrieved_block = blockchain.get_block(&hash)?;
        assert_eq!(retrieved_block.hash()?, hash);

        Ok(())
    }

    #[test]
    fn test_blockchain_invalid_block() -> Result<()> {
        let mut blockchain = Blockchain::new()?;

        // 创建无效区块（空交易）
        let invalid_block = Block::new(vec![], String::new())?;
        
        // 添加无效区块应该失败
        assert!(matches!(
            blockchain.add_block(invalid_block),
            Err(BlockchainError::InvalidBlock(_))
        ));

        Ok(())
    }

    #[test]
    fn test_blockchain_persistence() -> Result<()> {
        let mut blockchain = Blockchain::new()?;

        // 创建并添加区块
        let wallet = Wallet::new()?;
        let coinbase_tx = Transaction::new_coinbase(&wallet.get_address(), "Test Block")?;
        let mut block = Block::new(vec![coinbase_tx], String::new())?;
        block.mine_block(4)?;
        blockchain.add_block(block)?;

        // 保存区块链
        blockchain.save_to_file()?;

        // 加载区块链
        let loaded_blockchain = Blockchain::load_from_file()?;
        assert_eq!(
            blockchain.get_last_hash()?,
            loaded_blockchain.get_last_hash()?
        );

        Ok(())
    }
}

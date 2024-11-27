use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};

use crate::block::Block;
use crate::transaction::Transaction;
use crate::error::{Result, RustBtcError};

const MAX_BLOCK_SIZE: usize = 1_000_000; // 1MB
const MAX_CHAIN_LENGTH: usize = 1_000_000;

#[derive(Debug, Serialize, Deserialize)]
pub struct Blockchain {
    blocks: Vec<Block>,
    current_hash: String,
}

impl Blockchain {
    pub fn new() -> Result<Self> {
        info!("创建新的区块链");
        Ok(Blockchain {
            blocks: Vec::new(),
            current_hash: String::new(),
        })
    }

    pub fn add_block(&mut self, block: Block) -> Result<()> {
        debug!("开始添加新区块, 前置哈希: {}", block.prev_block_hash);
        
        let block_size = bincode::serialize(&block)
            .map_err(|e| RustBtcError::SerializationError(e.to_string()))?
            .len();
            
        if block_size > MAX_BLOCK_SIZE {
            error!("区块大小 {} 超过最大限制 {}", block_size, MAX_BLOCK_SIZE);
            return Err(RustBtcError::InvalidBlock(format!(
                "区块大小 {} 超过最大限制 {}",
                block_size, MAX_BLOCK_SIZE
            )));
        }
        
        if self.blocks.len() >= MAX_CHAIN_LENGTH {
            error!("区块链长度 {} 超过最大限制 {}", self.blocks.len(), MAX_CHAIN_LENGTH);
            return Err(RustBtcError::InvalidChain(format!(
                "区块链长度 {} 超过最大限制 {}",
                self.blocks.len(), MAX_CHAIN_LENGTH
            )));
        }
        
        if !self.blocks.is_empty() && block.prev_block_hash != self.current_hash {
            error!("区块的前置哈希 {} 与当前哈希 {} 不匹配", 
                block.prev_block_hash, self.current_hash);
            return Err(RustBtcError::InvalidBlock(format!(
                "区块的前置哈希 {} 与当前哈希 {} 不匹配",
                block.prev_block_hash, self.current_hash
            )));
        }

        self.current_hash = block.hash.clone();
        self.blocks.push(block);
        info!("成功添加新区块，当前区块链长度: {}", self.blocks.len());
        
        Ok(())
    }

    pub fn get_block(&self, hash: &str) -> Result<&Block> {
        debug!("查找哈希为 {} 的区块", hash);
        self.blocks
            .iter()
            .find(|block| block.hash == hash)
            .ok_or_else(|| {
                error!("未找到哈希为 {} 的区块", hash);
                RustBtcError::BlockNotFound(hash.to_string())
            })
    }

    pub fn get_last_hash(&self) -> Result<String> {
        if self.blocks.is_empty() {
            warn!("区块链为空，无法获取最后的哈希值");
            return Ok(String::new());
        }
        debug!("获取最后区块哈希: {}", self.current_hash);
        Ok(self.current_hash.clone())
    }

    pub fn save_to_file(&self) -> Result<()> {
        info!("开始保存区块链到文件");
        let data = bincode::serialize(self)
            .map_err(|e| RustBtcError::SerializationError(e.to_string()))?;
        fs::write("blockchain.dat", data)
            .map_err(|e| RustBtcError::IOError(e.to_string()))?;
        info!("区块链成功保存到文件");
        Ok(())
    }

    pub fn load_from_file() -> Result<Self> {
        info!("从文件加载区块链");
        if !Path::new("blockchain.dat").exists() {
            warn!("区块链文件不存在，创建新的区块链");
            return Self::new();
        }

        let data = fs::read("blockchain.dat")
            .map_err(|e| RustBtcError::IOError(e.to_string()))?;
        let blockchain = bincode::deserialize(&data)
            .map_err(|e| RustBtcError::DeserializationError(e.to_string()))?;
        info!("成功从文件加载区块链");
        Ok(blockchain)
    }

    pub fn validate_chain(&self) -> Result<bool> {
        info!("开始验证区块链");
        if self.blocks.is_empty() {
            warn!("区块链为空，验证通过");
            return Ok(true);
        }

        let mut prev_hash = String::new();
        for (i, block) in self.blocks.iter().enumerate() {
            debug!("验证第 {} 个区块", i + 1);
            
            // 验证区块哈希
            if !block.verify_hash()? {
                error!("区块 {} 哈希验证失败", i + 1);
                return Ok(false);
            }

            // 验证前置哈希
            if i > 0 && block.prev_block_hash != prev_hash {
                error!("区块 {} 的前置哈希不匹配", i + 1);
                return Ok(false);
            }

            prev_hash = block.hash.clone();
        }

        info!("区块链验证完成，验证通过");
        Ok(true)
    }

    pub fn get_block_height(&self) -> usize {
        debug!("获取区块链高度: {}", self.blocks.len());
        self.blocks.len()
    }

    pub fn get_blocks_after(&self, hash: &str) -> Result<Vec<&Block>> {
        debug!("获取哈希 {} 之后的所有区块", hash);
        let mut blocks = Vec::new();
        let mut found = false;

        for block in &self.blocks {
            if found {
                blocks.push(block);
            }
            if block.hash == hash {
                found = true;
            }
        }

        if !found && !hash.is_empty() {
            warn!("未找到哈希为 {} 的区块", hash);
            return Err(RustBtcError::BlockNotFound(hash.to_string()));
        }

        debug!("找到 {} 个后续区块", blocks.len());
        Ok(blocks)
    }

    pub fn find_transaction(&self, id: &str) -> Option<Transaction> {
        debug!("查找交易ID: {}", id);
        for block in self.blocks.iter().rev() {
            if let Some(tx) = block.transactions.iter().find(|tx| tx.id == id) {
                debug!("找到交易 {}", id);
                return Some(tx.clone());
            }
        }
        warn!("未找到交易 {}", id);
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
            Err(RustBtcError::InvalidBlock(_))
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

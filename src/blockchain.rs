use std::error::Error;
use std::fs;
use std::path::Path;
use bincode;

use crate::block::Block;
use crate::transaction::Transaction;
use crate::utxo::UTXOSet;

const DB_PATH: &str = "data/blockchain";
const MINING_DIFFICULTY: usize = 4;

#[derive(Debug)]
pub struct Blockchain {
    blocks: Vec<Block>,
    utxo_set: UTXOSet,
}

impl Blockchain {
    pub fn new(miner_address: &str) -> Result<Blockchain, Box<dyn Error>> {
        println!("创建新的区块链...");
        
        // 创建数据目录
        fs::create_dir_all(DB_PATH)?;
        
        // 创建创世区块
        let coinbase = Transaction::new_coinbase(miner_address, "创世区块")?;
        let genesis_block = Block::new_genesis_block(coinbase)?;
        
        // 创建区块链
        let mut blockchain = Blockchain {
            blocks: vec![genesis_block],
            utxo_set: UTXOSet::new()?,
        };
        
        // 更新UTXO集合
        {
            let blocks = &blockchain.blocks;
            blockchain.utxo_set.reindex(blocks)?;
        }
        
        // 保存区块链数据
        blockchain.save_to_disk()?;
        
        Ok(blockchain)
    }
    
    pub fn add_block(&mut self, transactions: Vec<Transaction>) -> Result<(), Box<dyn Error>> {
        println!("\n添加新区块...");
        
        // 创建新区块
        let mut new_block = Block::new(
            transactions,
            self.blocks.last().unwrap().hash.clone(),
        )?;
        
        // 挖矿
        new_block.mine_block(MINING_DIFFICULTY)?;
        
        // 添加区块
        self.blocks.push(new_block);
        
        // 更新UTXO集合
        {
            let blocks = &self.blocks;
            self.utxo_set.reindex(blocks)?;
        }
        
        // 保存区块链数据
        self.save_to_disk()?;
        
        println!("新区块已添加到区块链");
        Ok(())
    }
    
    pub fn get_balance(&self, address: &str) -> Result<i32, Box<dyn Error>> {
        self.utxo_set.get_balance(address)
    }
    
    pub fn get_utxo_set(&self) -> &UTXOSet {
        &self.utxo_set
    }
    
    fn save_to_disk(&self) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(&self.blocks)?;
        fs::write(format!("{}/blocks.dat", DB_PATH), data)?;
        Ok(())
    }
    
    pub fn load_from_disk() -> Result<Option<Blockchain>, Box<dyn Error>> {
        let path = format!("{}/blocks.dat", DB_PATH);
        
        if !Path::new(&path).exists() {
            return Ok(None);
        }
        
        let data = fs::read(&path)?;
        let blocks: Vec<Block> = bincode::deserialize(&data)?;
        
        let mut blockchain = Blockchain {
            blocks,
            utxo_set: UTXOSet::new()?,
        };
        
        // 更新UTXO集合
        {
            let blocks = &blockchain.blocks;
            blockchain.utxo_set.reindex(blocks)?;
        }
        
        Ok(Some(blockchain))
    }
    
    pub fn get_blocks(&self) -> Result<&Vec<Block>, Box<dyn Error>> {
        Ok(&self.blocks)
    }
    
    pub fn find_transaction(&self, txid: &str) -> Result<Option<Transaction>, Box<dyn Error>> {
        println!("查找交易: {}", txid);
        
        // 遍历所有区块
        for block in self.get_blocks()?.iter().rev() {
            // 遍历区块中的所有交易
            for tx in &block.transactions {
                if tx.id == txid {
                    println!("找到交易!");
                    return Ok(Some(tx.clone()));
                }
            }
        }
        
        println!("未找到交易");
        Ok(None)
    }
    
    pub fn cleanup() -> Result<(), Box<dyn Error>> {
        println!("清理数据...");
        
        // 如果区块链数据目录存在，删除它
        if Path::new(DB_PATH).exists() {
            fs::remove_dir_all(DB_PATH)?;
        }
        
        // 如果UTXO数据文件存在，删除它
        if Path::new("data/utxo.dat").exists() {
            fs::remove_file("data/utxo.dat")?;
        }
        
        println!("数据清理完成");
        Ok(())
    }
}

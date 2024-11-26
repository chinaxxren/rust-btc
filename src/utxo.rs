use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs;
use std::error::Error;
use crate::block::Block;
use crate::transaction::TxOutput;
use crate::blockchain::Blockchain;

const UTXO_DB_FILE: &str = "data/utxo.dat";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UTXOSet {
    utxos: HashMap<String, TxOutput>,
}

impl UTXOSet {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        // 确保目录存在
        if let Some(parent) = std::path::Path::new(UTXO_DB_FILE).parent() {
            fs::create_dir_all(parent)?;
        }
        
        // 尝试从文件加载现有的UTXO集
        if let Ok(data) = fs::read(UTXO_DB_FILE) {
            if let Ok(utxo_set) = bincode::deserialize(&data) {
                println!("从文件加载了现有的UTXO集");
                return Ok(utxo_set);
            }
        }
        
        println!("创建了新的UTXO集");
        Ok(UTXOSet {
            utxos: HashMap::new(),
        })
    }

    fn save(&self) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(self)?;
        fs::write(UTXO_DB_FILE, data)?;
        println!("保存了UTXO集到文件");
        Ok(())
    }

    pub fn update(&mut self, block: &Block) -> Result<(), Box<dyn Error>> {
        println!("\n更新UTXO集...");
        println!("当前UTXO数量: {}", self.utxos.len());
        
        // 处理区块中的每个交易
        for tx in &block.transactions {
            println!("处理交易: {}", tx.id);
            
            // 如果不是coinbase交易，删除已花费的输出
            if !tx.is_coinbase() {
                for input in &tx.inputs {
                    let key = format!("{}:{}", input.txid, input.vout);
                    self.utxos.remove(&key);
                    println!("删除已花费的UTXO: {}", key);
                }
            } else {
                println!("这是一个coinbase交易");
            }
            
            // 添加新的未花费输出
            for (idx, output) in tx.outputs.iter().enumerate() {
                let key = format!("{}:{}", tx.id, idx);
                println!("添加新的UTXO: {} -> {} 代币到地址 {}", key, output.value, output.pub_key_hash);
                self.utxos.insert(key, output.clone());
            }
        }
        
        println!("更新后UTXO数量: {}", self.utxos.len());
        self.save()?;
        Ok(())
    }

    pub fn get_balance(&self, address: &str) -> Result<i32, Box<dyn Error>> {
        let mut balance = 0;
        
        println!("\n计算地址 {} 的余额", address);
        println!("当前UTXO数量: {}", self.utxos.len());
        
        for (key, output) in &self.utxos {
            println!("检查UTXO {} -> {} 代币到地址 {}", key, output.value, output.pub_key_hash);
            if output.pub_key_hash == address {
                balance += output.value;
                println!("找到一个属于该地址的UTXO，余额增加 {}", output.value);
            }
        }
        
        println!("最终余额: {}", balance);
        Ok(balance)
    }

    pub fn find_spendable_outputs(&self, address: &str, amount: i32) -> Result<(i32, Vec<(String, i32)>), Box<dyn Error>> {
        let mut unspent_outputs = Vec::new();
        let mut accumulated = 0;
        
        for (utxo_key, output) in &self.utxos {
            if output.pub_key_hash == address {
                let parts: Vec<&str> = utxo_key.split(':').collect();
                if parts.len() != 2 {
                    continue;
                }
                
                let txid = parts[0].to_string();
                let vout = parts[1].parse::<i32>()?;
                
                accumulated += output.value;
                unspent_outputs.push((txid, vout));
                
                if accumulated >= amount {
                    break;
                }
            }
        }
        
        if accumulated < amount {
            return Err(format!("地址 {} 余额不足，需要 {} 代币", address, amount).into());
        }
        
        Ok((accumulated, unspent_outputs))
    }

    pub fn reindex(&mut self, blocks: &Vec<Block>) -> Result<(), Box<dyn Error>> {
        println!("重建UTXO集合...");
        self.utxos.clear();
        
        // 遍历所有区块
        for block in blocks.iter() {
            // 遍历区块中的所有交易
            for tx in &block.transactions {
                // 标记已使用的UTXO
                if !tx.is_coinbase() {
                    for input in &tx.inputs {
                        let key = format!("{}:{}", input.txid, input.vout);
                        self.utxos.remove(&key);
                    }
                }
                
                // 添加新的UTXO
                for (idx, output) in tx.outputs.iter().enumerate() {
                    let key = format!("{}:{}", tx.id, idx);
                    self.utxos.insert(key, output.clone());
                }
            }
        }
        
        // 保存UTXO集合
        self.save()?;
        
        println!("UTXO集合重建完成");
        Ok(())
    }
}

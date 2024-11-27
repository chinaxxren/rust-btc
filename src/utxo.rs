use std::collections::HashMap;
use std::fs;
use std::path::Path;

use bs58;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};

use crate::error::{Result, RustBtcError};
use crate::blockchain::Blockchain;
use crate::transaction::{Transaction, TxOutput, TxInput};

const UTXO_TREE_FILE: &str = "data/utxo.dat";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UTXOSet {
    utxos: HashMap<String, Vec<(usize, TxOutput)>>,
}

impl UTXOSet {
    pub fn new() -> Self {
        debug!("创建新的UTXO集");
        UTXOSet {
            utxos: HashMap::new(),
        }
    }

    pub fn update(&mut self, block_txs: &[Transaction]) -> Result<()> {
        debug!("更新UTXO集，处理 {} 笔交易", block_txs.len());
        
        for tx in block_txs {
            if !tx.is_coinbase() {
                debug!("处理非coinbase交易: {}", tx.id);
                // 移除已花费的输出
                for input in &tx.vin {
                    if let Some(outputs) = self.utxos.get_mut(&input.txid) {
                        debug!("移除已花费的UTXO: txid={}, vout={}", input.txid, input.vout);
                        outputs.retain(|(vout, _)| *vout != input.vout);
                        if outputs.is_empty() {
                            self.utxos.remove(&input.txid);
                        }
                    }
                }
            } else {
                debug!("处理coinbase交易: {}", tx.id);
            }

            // 添加新的未花费输出
            let mut outputs = Vec::new();
            for (vout, output) in tx.vout.iter().enumerate() {
                debug!("添加新的UTXO: txid={}, vout={}, value={}", 
                    tx.id, vout, output.value);
                outputs.push((vout, output.clone()));
            }
            self.utxos.insert(tx.id.clone(), outputs);
        }

        info!("UTXO集更新完成，当前包含 {} 个交易的UTXO", self.utxos.len());
        Ok(())
    }

    pub fn verify_input(&self, input: &TxInput) -> Result<bool> {
        debug!("验证交易输入: txid={}, vout={}", input.txid, input.vout);
        
        // 检查UTXO是否存在
        if let Some(outputs) = self.utxos.get(&input.txid) {
            if let Some((_, output)) = outputs.iter().find(|(vout, _)| *vout == input.vout) {
                debug!("找到对应的UTXO，金额: {}", output.value);
                
                // 验证金额
                if output.value != input.value {
                    error!("UTXO金额不匹配: 期望={}, 实际={}", 
                        input.value, output.value);
                    return Ok(false);
                }
                
                debug!("交易输入验证通过");
                return Ok(true);
            }
        }
        
        error!("未找到对应的UTXO: txid={}, vout={}", input.txid, input.vout);
        Ok(false)
    }

    pub fn exists_utxo(&self, txid: &str, vout: usize) -> Result<bool> {
        debug!("检查UTXO是否存在: txid={}, vout={}", txid, vout);
        if let Some(outputs) = self.utxos.get(txid) {
            Ok(outputs.iter().any(|(v, _)| *v == vout))
        } else {
            Ok(false)
        }
    }

    pub fn save(&self) -> Result<()> {
        info!("保存UTXO集到文件");
        
        // 确保目录存在
        if let Some(parent) = Path::new(UTXO_TREE_FILE).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| RustBtcError::IOError(e.to_string()))?;
        }
        
        let data = bincode::serialize(self)
            .map_err(|e| RustBtcError::SerializationError(e.to_string()))?;
            
        fs::write(UTXO_TREE_FILE, data)
            .map_err(|e| RustBtcError::IOError(e.to_string()))?;
            
        info!("UTXO集保存成功");
        Ok(())
    }

    pub fn load() -> Result<Self> {
        info!("从文件加载UTXO集");
        
        if !Path::new(UTXO_TREE_FILE).exists() {
            warn!("UTXO文件不存在，创建新的UTXO集");
            return Ok(Self::new());
        }

        let data = fs::read(UTXO_TREE_FILE)
            .map_err(|e| RustBtcError::IOError(e.to_string()))?;
            
        let utxo_set = bincode::deserialize(&data)
            .map_err(|e| RustBtcError::DeserializationError(e.to_string()))?;
            
        info!("UTXO集加载成功");
        Ok(utxo_set)
    }

    pub fn reindex(&mut self, blockchain: &Blockchain) -> Result<()> {
        info!("重建UTXO集索引");
        self.utxos.clear();
        
        // 遍历所有区块
        for block in blockchain.blocks() {
            debug!("处理区块: {}", block.hash);
            
            // 处理区块中的所有交易
            for tx in &block.transactions {
                debug!("处理交易: {}", tx.id);
                
                // 如果不是coinbase交易，移除已花费的输出
                if !tx.is_coinbase() {
                    for input in &tx.vin {
                        debug!("检查移除UTXO: txid={}, vout={}", input.txid, input.vout);
                        if let Some(outputs) = self.utxos.get_mut(&input.txid) {
                            debug!("移除已花费的UTXO: txid={}, vout={}", 
                                input.txid, input.vout);
                            outputs.retain(|(vout, _)| *vout != input.vout);
                            if outputs.is_empty() {
                                self.utxos.remove(&input.txid);
                            }
                        }
                    }
                }
                
                // 添加新的未花费输出
                let mut outputs = Vec::new();
                for (vout, output) in tx.vout.iter().enumerate() {
                    debug!("添加新的UTXO: txid={}, vout={}, value={}", 
                        tx.id, vout, output.value);
                    outputs.push((vout, output.clone()));
                }
                
                // 检查是否已存在相同ID的交易
                if self.utxos.contains_key(&tx.id) {
                    debug!("警告：发现重复的交易ID: {}", tx.id);
                    continue;
                }
                
                self.utxos.insert(tx.id.clone(), outputs);
            }
        }
        
        info!("UTXO集索引重建完成，当前包含 {} 个交易的UTXO", self.utxos.len());
        Ok(())
    }

    pub fn get_balance(&self, address: &str) -> Result<i64> {
        debug!("计算地址余额: {}", address);
        
        let mut balance = 0;
        let address_bytes = bs58::decode(address)
            .into_vec()
            .map_err(|e| RustBtcError::InvalidAddress(e.to_string()))?;

        for outputs in self.utxos.values() {
            for (_, output) in outputs {
                if output.pubkey_hash == address_bytes {
                    debug!("找到UTXO: value={}", output.value);
                    balance += output.value;
                }
            }
        }
        
        debug!("地址 {} 的余额为: {}", address, balance);
        Ok(balance)
    }

    pub fn find_spendable_outputs(&self, address: &str, amount: i64) -> Result<Vec<UTXOInfo>> {
        debug!("查找可花费的UTXO: address={}, amount={}", address, amount);
        
        let mut outputs = Vec::new();
        let mut accumulated = 0;
        
        let address_bytes = bs58::decode(address)
            .into_vec()
            .map_err(|e| RustBtcError::InvalidAddress(e.to_string()))?;
            
        'outer: for (txid, txouts) in &self.utxos {
            for (vout, output) in txouts {
                if output.pubkey_hash == address_bytes {
                    debug!("找到可用UTXO: txid={}, vout={}, value={}", 
                        txid, vout, output.value);
                        
                    accumulated += output.value;
                    outputs.push(UTXOInfo {
                        txid: txid.clone(),
                        vout: *vout,
                        value: output.value,
                    });
                    
                    if accumulated >= amount {
                        debug!("已收集足够的UTXO，总额: {}", accumulated);
                        break 'outer;
                    }
                }
            }
        }
        
        if accumulated < amount {
            warn!("可用UTXO总额 {} 不足支付 {}", accumulated, amount);
            return Err(RustBtcError::InsufficientFunds(format!(
                "可用余额 {} 不足支付 {}", accumulated, amount
            )));
        }
        
        info!("成功找到足够的UTXO，总额: {}", accumulated);
        Ok(outputs)
    }

    pub fn find_utxo(&self, txid: &str, vout: usize) -> Result<Option<TxOutput>> {
        debug!("查找指定的UTXO: txid={}, vout={}", txid, vout);
        if let Some(outputs) = self.utxos.get(txid) {
            if let Some((_, output)) = outputs.iter().find(|(v, _)| *v == vout) {
                debug!("找到UTXO，金额: {}", output.value);
                return Ok(Some(output.clone()));
            }
        }
        debug!("未找到指定的UTXO");
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct UTXOInfo {
    pub txid: String,
    pub vout: usize,
    pub value: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utxo_basic_operations() -> Result<()> {
        let mut utxo_set = UTXOSet::new();
        let test_address = "test_address";
        
        // Create a test transaction
        let mut tx = Transaction {
            id: "test_tx".to_string(),
            vin: vec![],
            vout: vec![
                TxOutput {
                    value: 50,
                    pubkey_hash: bs58::decode(test_address)
                        .into_vec()
                        .map_err(|e| RustBtcError::InvalidAddress(e.to_string()))?,
                }
            ],
        };

        // Update UTXO set with the transaction
        utxo_set.update(&[tx.clone()])?;

        // Verify UTXO exists
        assert!(utxo_set.exists_utxo(&tx.id, 0)?);

        // Check balance
        assert_eq!(utxo_set.get_balance(test_address)?, 50);

        Ok(())
    }

    #[test]
    fn test_find_spendable_outputs() -> Result<()> {
        let mut utxo_set = UTXOSet::new();
        let test_address = "test_address";
        
        // Create test transactions
        let tx1 = Transaction {
            id: "tx1".to_string(),
            vin: vec![],
            vout: vec![
                TxOutput {
                    value: 30,
                    pubkey_hash: bs58::decode(test_address)
                        .into_vec()
                        .map_err(|e| RustBtcError::InvalidAddress(e.to_string()))?,
                }
            ],
        };

        let tx2 = Transaction {
            id: "tx2".to_string(),
            vin: vec![],
            vout: vec![
                TxOutput {
                    value: 20,
                    pubkey_hash: bs58::decode(test_address)
                        .into_vec()
                        .map_err(|e| RustBtcError::InvalidAddress(e.to_string()))?,
                }
            ],
        };

        // Update UTXO set
        utxo_set.update(&[tx1, tx2])?;

        // Find spendable outputs for 40 coins
        let outputs = utxo_set.find_spendable_outputs(test_address, 40)?;
        assert_eq!(outputs.len(), 2);

        Ok(())
    }

    #[test]
    fn test_utxo_persistence() -> Result<()> {
        let mut utxo_set = UTXOSet::new();
        let test_address = "test_address";
        
        // Create a test transaction
        let tx = Transaction {
            id: "test_tx".to_string(),
            vin: vec![],
            vout: vec![
                TxOutput {
                    value: 50,
                    pubkey_hash: bs58::decode(test_address)
                        .into_vec()
                        .map_err(|e| RustBtcError::InvalidAddress(e.to_string()))?,
                }
            ],
        };

        // Update and save UTXO set
        utxo_set.update(&[tx])?;
        utxo_set.save()?;

        // Load UTXO set and verify data
        let loaded_utxo = UTXOSet::load()?;
        assert_eq!(loaded_utxo.get_balance(test_address)?, 50);

        Ok(())
    }
}

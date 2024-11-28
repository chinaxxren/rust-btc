use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::error::{Result, RustBtcError};
use crate::transaction::{Transaction, TxInput, TxOutput};

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
                .map_err(|e| RustBtcError::Io(e))?;
        }
        
        let data = bincode::serialize(self)
            .map_err(|e| RustBtcError::Serialization(e))?;
            
        fs::write(UTXO_TREE_FILE, data)
            .map_err(|e| RustBtcError::Io(e))?;
            
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
            .map_err(|e| RustBtcError::Io(e))?;
            
        let utxo_set = bincode::deserialize(&data)
            .map_err(|e| RustBtcError::DeserializationError(e.to_string()))?;
            
        info!("UTXO集加载成功");
        Ok(utxo_set)
    }

    pub fn reindex(&mut self, blockchain: &crate::blockchain::Blockchain) -> Result<()> {
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

    pub fn find_transaction_output(&self, txid: &str, vout: usize) -> Result<TxOutput> {
        debug!("查找交易输出: txid={}, vout={}", txid, vout);
        
        // 检查 UTXO 是否存在
        if !self.exists_utxo(txid, vout)? {
            return Err(RustBtcError::UTXONotFound(format!(
                "UTXO不存在: txid={}, vout={}",
                txid, vout
            )));
        }
        
        // 获取 UTXO
        let utxos = self.utxos.get(txid).ok_or_else(|| {
            RustBtcError::UTXONotFound(format!("UTXO不存在: txid={}", txid))
        })?;
        
        // 获取指定的输出
        let (_, output) = utxos.get(vout).ok_or_else(|| {
            RustBtcError::UTXONotFound(format!(
                "UTXO输出不存在: txid={}, vout={}",
                txid, vout
            ))
        })?;
        
        Ok(output.clone())
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

    fn create_test_wallet() -> Result<Wallet> {
        Wallet::new()
    }

    #[test]
    fn test_utxo_basic_operations() -> Result<()> {
        let mut utxo_set = UTXOSet::new();
        let wallet = create_test_wallet()?;
        let address = wallet.get_address();
        
        // 创建测试交易
        let tx = Transaction::new_coinbase(&address, "Test UTXO")?;
        
        // 添加 UTXO
        utxo_set.update(&[tx.clone()])?;
        
        // 验证 UTXO 已添加
        let utxos = utxo_set.find_spendable_outputs(&address, 50)?;
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].value, 50);
        
        Ok(())
    }

    #[test]
    fn test_utxo_persistence() -> Result<()> {
        let wallet = create_test_wallet()?;
        let address = wallet.get_address();
        
        // 创建并保存 UTXO 集
        {
            let mut utxo_set = UTXOSet::new();
            let tx = Transaction::new_coinbase(&address, "Test Persistence")?;
            utxo_set.update(&[tx])?;
            utxo_set.save()?;
        }
        
        // 加载并验证 UTXO 集
        {
            let utxo_set = UTXOSet::load()?;
            let utxos = utxo_set.find_spendable_outputs(&address, 50)?;
            assert_eq!(utxos.len(), 1);
            assert_eq!(utxos[0].value, 50);
        }
        
        Ok(())
    }

    #[test]
    fn test_find_spendable_outputs() -> Result<()> {
        let mut utxo_set = UTXOSet::new();
        let wallet = create_test_wallet()?;
        let address = wallet.get_address();
        
        // 创建多个测试交易
        for i in 0..3 {
            let tx = Transaction::new_coinbase(&address, &format!("Test {}", i))?;
            utxo_set.update(&[tx])?;
        }
        
        // 测试不同金额的查找
        let utxos = utxo_set.find_spendable_outputs(&address, 50)?;
        assert_eq!(utxos.len(), 1);  // 需要一个 UTXO 来满足 50 的金额
        
        let utxos = utxo_set.find_spendable_outputs(&address, 100)?;
        assert_eq!(utxos.len(), 2);  // 需要两个 UTXO 来满足 100 的金额
        
        let utxos = utxo_set.find_spendable_outputs(&address, 150)?;
        assert_eq!(utxos.len(), 3);  // 需要三个 UTXO 来满足 150 的金额
        
        Ok(())
    }
}

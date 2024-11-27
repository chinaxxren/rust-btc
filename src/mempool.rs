use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use rayon::prelude::*;

use crate::transaction::Transaction;
use crate::utxo::UTXOSet;

// 自定义错误类型
#[derive(Debug)]
pub enum MempoolError {
    LockError(String),
    ValidationError(String),
    UTXOError(String),
}

impl std::fmt::Display for MempoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MempoolError::LockError(msg) => write!(f, "Mempool lock error: {}", msg),
            MempoolError::ValidationError(msg) => write!(f, "Transaction validation error: {}", msg),
            MempoolError::UTXOError(msg) => write!(f, "UTXO error: {}", msg),
        }
    }
}

impl Error for MempoolError {}

type Result<T> = std::result::Result<T, MempoolError>;

pub struct Mempool {
    transactions: Arc<RwLock<HashMap<String, Transaction>>>,
    timestamps: Arc<RwLock<HashMap<String, u64>>>,
}

impl Mempool {
    pub fn new() -> Self {
        Mempool {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            timestamps: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // 批量添加交易
    pub fn add_transactions(&self, txs: Vec<Transaction>, utxo_set: &UTXOSet) -> Result<()> {
        let results: Vec<Result<()>> = txs.par_iter()
            .map(|tx| self.add_transaction(tx.clone(), utxo_set))
            .collect();

        // 检查所有结果
        for result in results {
            result?;
        }
        Ok(())
    }

    // 添加单个交易
    pub fn add_transaction(&self, tx: Transaction, utxo_set: &UTXOSet) -> Result<()> {
        // 验证交易
        self.validate_transaction(&tx, utxo_set)?;

        // 获取当前时间戳
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| MempoolError::LockError(e.to_string()))?
            .as_secs();

        // 添加交易到内存池
        {
            let mut txs = self.transactions.write()
                .map_err(|e| MempoolError::LockError(e.to_string()))?;
            let mut timestamps = self.timestamps.write()
                .map_err(|e| MempoolError::LockError(e.to_string()))?;
            
            let tx_id = tx.id.clone();
            txs.insert(tx_id.clone(), tx);
            timestamps.insert(tx_id, timestamp);
        }

        Ok(())
    }

    // 从内存池中移除交易
    pub fn remove_transaction(&self, txid: &str) -> Result<()> {
        let mut txs = self.transactions.write()
            .map_err(|e| MempoolError::LockError(e.to_string()))?;
        let mut timestamps = self.timestamps.write()
            .map_err(|e| MempoolError::LockError(e.to_string()))?;
        
        txs.remove(txid);
        timestamps.remove(txid);
        
        Ok(())
    }

    // 获取内存池大小
    pub fn size(&self) -> usize {
        self.transactions.read().unwrap().len()
    }

    // 清理过期交易
    pub fn clear_expired(&self, max_age_seconds: u64) -> Result<()> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| MempoolError::LockError(e.to_string()))?
            .as_secs();

        let expired_txids: Vec<String> = {
            let timestamps = self.timestamps.read()
                .map_err(|e| MempoolError::LockError(e.to_string()))?;
            timestamps.iter()
                .filter(|(_, &timestamp)| current_time - timestamp > max_age_seconds)
                .map(|(txid, _)| txid.clone())
                .collect()
        };

        for txid in expired_txids {
            self.remove_transaction(&txid)?;
        }

        Ok(())
    }

    // 验证交易
    fn validate_transaction(&self, tx: &Transaction, utxo_set: &UTXOSet) -> Result<()> {
        // 验证输入
        for input in &tx.inputs {
            // 检查输入是否已经在UTXO集中
            if !utxo_set.exists_utxo(&input.txid, input.vout)
                .map_err(|e| MempoolError::UTXOError(e.to_string()))? {
                return Err(MempoolError::ValidationError(
                    "交易输入不存在于UTXO集中".to_string()
                ));
            }
        }

        // 验证输入金额是否大于等于输出金额
        let mut input_sum = 0;
        for input in &tx.inputs {
            if let Some(utxo) = utxo_set.find_utxo(&input.txid, input.vout)
                .map_err(|e| MempoolError::UTXOError(e.to_string()))? {
                input_sum += utxo.value;
            }
        }

        let output_sum: i32 = tx.outputs.iter().map(|output| output.value).sum();
        if input_sum < output_sum {
            return Err(MempoolError::ValidationError(
                "交易输入金额小于输出金额".to_string()
            ));
        }

        Ok(())
    }

    // 获取最优交易列表
    pub fn get_best_transactions(&self, limit: usize) -> Vec<Transaction> {
        let txs = self.transactions.read().unwrap();
        let mut transactions: Vec<Transaction> = txs.values().cloned().collect();
        
        // 按照每字节手续费排序
        transactions.sort_by(|a, b| {
            let a_fee_rate = a.calculate_fee_rate();
            let b_fee_rate = b.calculate_fee_rate();
            b_fee_rate.partial_cmp(&a_fee_rate).unwrap()
        });
        
        transactions.truncate(limit);
        transactions
    }
}

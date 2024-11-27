use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use lru::LruCache;
use parking_lot::RwLock;
use rayon::prelude::*;

use crate::transaction::Transaction;
use crate::utxo::UTXOSet;
use crate::wallet::Wallet;

const MAX_CACHE_SIZE: usize = 10000;
const MAX_MEMPOOL_SIZE: usize = 5000;
const MIN_FEE_RATE: f64 = 0.00001;
const MAX_TRANSACTION_SIZE: usize = 100_000;

#[derive(Debug)]
pub enum MempoolError {
    ValidationError(String),
    DuplicateTransaction(String),
    TransactionNotFound(String),
    CapacityExceeded(String),
    UTXOError(String),
    InvalidAmount(String),
    InvalidFee(String),
    SerializationError(String),
    TransactionError(String),
}

impl std::error::Error for MempoolError {}

impl std::fmt::Display for MempoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            MempoolError::ValidationError(msg) => write!(f, "交易验证错误: {}", msg),
            MempoolError::DuplicateTransaction(msg) => write!(f, "重复交易: {}", msg),
            MempoolError::TransactionNotFound(msg) => write!(f, "交易未找到: {}", msg),
            MempoolError::CapacityExceeded(msg) => write!(f, "内存池容量超限: {}", msg),
            MempoolError::UTXOError(msg) => write!(f, "UTXO错误: {}", msg),
            MempoolError::InvalidAmount(msg) => write!(f, "无效金额: {}", msg),
            MempoolError::InvalidFee(msg) => write!(f, "无效手续费: {}", msg),
            MempoolError::SerializationError(msg) => write!(f, "序列化错误: {}", msg),
            MempoolError::TransactionError(msg) => write!(f, "交易错误: {}", msg),
        }
    }
}

impl From<Box<dyn std::error::Error>> for MempoolError {
    fn from(error: Box<dyn std::error::Error>) -> Self {
        MempoolError::ValidationError(error.to_string())
    }
}

impl From<bincode::Error> for MempoolError {
    fn from(error: bincode::Error) -> Self {
        MempoolError::SerializationError(error.to_string())
    }
}

type Result<T> = std::result::Result<T, MempoolError>;

#[derive(Debug)]
struct TransactionEntry {
    transaction: Transaction,
    #[allow(dead_code)]
    timestamp: u64,
    #[allow(dead_code)]
    fee: f64,
    #[allow(dead_code)]
    validation_result: bool,
}

impl TransactionEntry {
    fn new(transaction: Transaction) -> Result<Self> {
        let fee = transaction.calculate_fee_rate();
        if fee < MIN_FEE_RATE {
            return Err(MempoolError::InvalidFee(format!(
                "手续费率 {} 低于最小要求 {}",
                fee, MIN_FEE_RATE
            )));
        }

        Ok(Self {
            transaction,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            fee,
            validation_result: true,
        })
    }
}

pub struct Mempool {
    transactions: Arc<DashMap<String, TransactionEntry>>,
    validation_cache: Arc<RwLock<LruCache<String, bool>>>,
    utxo_set: Arc<UTXOSet>,
    max_size: usize,
}

impl Mempool {
    pub fn new(utxo_set: Arc<UTXOSet>) -> Self {
        Mempool {
            transactions: Arc::new(DashMap::new()),
            validation_cache: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(MAX_CACHE_SIZE).unwrap()
            ))),
            utxo_set,
            max_size: MAX_MEMPOOL_SIZE,
        }
    }

    pub fn with_capacity(max_size: usize, utxo_set: Arc<UTXOSet>) -> Self {
        Mempool {
            transactions: Arc::new(DashMap::new()),
            validation_cache: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(MAX_CACHE_SIZE).unwrap()
            ))),
            utxo_set,
            max_size,
        }
    }

    pub fn add_transactions(&self, txs: Vec<Transaction>) -> Result<()> {
        if self.transactions.len() + txs.len() > self.max_size {
            return Err(MempoolError::CapacityExceeded(format!(
                "内存池已达到最大容量 {}",
                self.max_size
            )));
        }

        // 并行验证交易
        txs.into_par_iter()
            .try_for_each(|tx| self.add_transaction(tx))?;

        Ok(())
    }

    pub fn add_transaction(&self, tx: Transaction) -> Result<()> {
        // 检查内存池容量
        if self.transactions.len() >= self.max_size {
            return Err(MempoolError::CapacityExceeded(format!(
                "内存池已达到最大容量 {}",
                self.max_size
            )));
        }

        // 验证交易
        self.validate_transaction(&tx)?;

        // 检查交易大小
        let tx_size = bincode::serialize(&tx)
            .map_err(|e| MempoolError::SerializationError(e.to_string()))?
            .len();
        if tx_size > MAX_TRANSACTION_SIZE {
            return Err(MempoolError::ValidationError(format!(
                "交易大小 {} 超过最大限制 {}",
                tx_size, MAX_TRANSACTION_SIZE
            )));
        }

        // 检查是否已存在
        let tx_hash = tx.hash().map_err(|e| MempoolError::SerializationError(e.to_string()))?;
        if self.transactions.contains_key(&tx_hash) {
            return Err(MempoolError::DuplicateTransaction(format!(
                "交易 {} 已存在于内存池中",
                tx_hash
            )));
        }

        // 创建交易条目
        let entry = TransactionEntry::new(tx)?;

        // 添加到内存池
        self.transactions.insert(tx_hash, entry);
        Ok(())
    }

    pub fn remove_transaction(&self, tx_hash: &str) -> Result<()> {
        self.transactions
            .remove(tx_hash)
            .ok_or_else(|| MempoolError::TransactionNotFound(tx_hash.to_string()))?;
        Ok(())
    }

    pub fn get_transaction(&self, tx_hash: &str) -> Result<Transaction> {
        self.transactions
            .get(tx_hash)
            .map(|entry| entry.transaction.clone())
            .ok_or_else(|| MempoolError::TransactionNotFound(tx_hash.to_string()))
    }

    pub fn clear(&self) {
        self.transactions.clear();
        self.validation_cache.write().clear();
    }

    pub fn size(&self) -> usize {
        self.transactions.len()
    }

    pub fn get_transactions(&self) -> Vec<Transaction> {
        self.transactions
            .iter()
            .map(|entry| entry.value().transaction.clone())
            .collect()
    }

    pub fn get_transactions_for_new_block(&self, max_size: usize) -> Vec<Transaction> {
        let mut transactions: Vec<_> = self.transactions.iter().collect();
        transactions.sort_by(|a, b| {
            b.value()
                .fee
                .partial_cmp(&a.value().fee)
                .unwrap()
        });

        transactions
            .into_iter()
            .take(max_size)
            .map(|entry| entry.value().transaction.clone())
            .collect()
    }

    fn validate_transaction(&self, tx: &Transaction) -> Result<()> {
        // 验证交易基本属性
        if tx.inputs.is_empty() {
            return Err(MempoolError::ValidationError("交易输入不能为空".to_string()));
        }
        if tx.outputs.is_empty() {
            return Err(MempoolError::ValidationError("交易输出不能为空".to_string()));
        }

        // 验证输入金额
        let mut total_input = 0i64;
        for input in &tx.inputs {
            if input.value <= 0 {
                return Err(MempoolError::InvalidAmount(format!(
                    "输入金额 {} 必须大于0",
                    input.value
                )));
            }
            total_input += input.value;
        }

        // 验证输出金额
        let mut total_output = 0i64;
        for output in &tx.outputs {
            if output.value <= 0 {
                return Err(MempoolError::InvalidAmount(format!(
                    "输出金额 {} 必须大于0",
                    output.value
                )));
            }
            total_output += output.value;
        }

        // 验证输入大于输出
        if total_input <= total_output {
            return Err(MempoolError::InvalidAmount(format!(
                "输入总额 {} 必须大于输出总额 {}",
                total_input, total_output
            )));
        }

        // 检查UTXO是否存在且未被使用
        for input in &tx.inputs {
            if !self.utxo_set.exists_utxo(&input.txid, input.vout)? {
                return Err(MempoolError::UTXOError(format!(
                    "UTXO {}:{} 不存在或已被使用",
                    input.txid, input.vout
                )));
            }

            // 检查签名
            if input.signature.is_empty() {
                return Err(MempoolError::ValidationError(
                    "交易输入必须包含有效签名".to_string(),
                ));
            }
        }

        // 验证交易本身
        if !tx.verify(&self.utxo_set)? {
            return Err(MempoolError::ValidationError("交易验证失败".to_string()));
        }

        Ok(())
    }

    // 清理过期交易
    pub fn cleanup_old_transactions(&self, max_age: u64) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.transactions.retain(|_, entry| {
            current_time - entry.timestamp <= max_age
        });
    }

    // 获取按手续费排序的交易
    pub fn get_sorted_transactions(&self) -> Vec<Transaction> {
        let mut txs: Vec<_> = self.transactions
            .iter()
            .map(|entry| (entry.value().transaction.clone(), entry.value().fee))
            .collect();
        
        txs.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        txs.into_iter().map(|(tx, _)| tx).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::Wallet;
    use std::sync::Arc;

    #[test]
    fn test_mempool_basic_operations() -> Result<()> {
        let utxo_set = Arc::new(UTXOSet::new());
        let mempool = Mempool::new(utxo_set);
        let wallet = Wallet::new().map_err(|e| MempoolError::TransactionError(e.to_string()))?;

        // Create test transaction
        let tx = Transaction::new_coinbase(&wallet.get_address(), "test")
            .map_err(|e| MempoolError::TransactionError(e.to_string()))?;
        
        // Add transaction to mempool
        mempool.add_transaction(tx.clone());

        // Verify transaction exists
        let tx_hash = tx.hash().map_err(|e| MempoolError::TransactionError(e.to_string()))?;
        let retrieved_tx = mempool.get_transaction(&tx_hash)?;
        assert_eq!(retrieved_tx.hash().map_err(|e| MempoolError::TransactionError(e.to_string()))?, tx_hash);

        // Remove transaction
        mempool.remove_transaction(&tx_hash);
        assert!(mempool.get_transaction(&tx_hash).is_err());

        Ok(())
    }

    #[test]
    fn test_mempool_duplicate_transaction() -> Result<()> {
        let utxo_set = Arc::new(UTXOSet::new());
        let mempool = Mempool::new(utxo_set);
        let wallet = Wallet::new().map_err(|e| MempoolError::TransactionError(e.to_string()))?;

        // Create test transaction
        let tx = Transaction::new_coinbase(&wallet.get_address(), "test")
            .map_err(|e| MempoolError::TransactionError(e.to_string()))?;

        // First addition should succeed
        mempool.add_transaction(tx.clone())?;

        // Second addition should fail
        assert!(mempool.add_transaction(tx.clone()).is_err());

        Ok(())
    }

    #[test]
    fn test_mempool_capacity() -> Result<()> {
        let utxo_set = Arc::new(UTXOSet::new());
        let mempool = Mempool::new(utxo_set);
        let wallet = Wallet::new().map_err(|e| MempoolError::TransactionError(e.to_string()))?;

        // Create test transaction
        let tx = Transaction::new_coinbase(&wallet.get_address(), "test")
            .map_err(|e| MempoolError::TransactionError(e.to_string()))?;

        // Add transactions until capacity limit
        for _ in 0..MAX_MEMPOOL_SIZE {
            let _ = mempool.add_transaction(tx.clone());
        }

        // Adding one more transaction should fail
        assert!(mempool.add_transaction(tx.clone()).is_err());

        Ok(())
    }
}

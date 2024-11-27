use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use lru::LruCache;
use parking_lot::RwLock;
use rayon::prelude::*;

use crate::transaction::Transaction;
use crate::utxo::UTXOSet;

const MAX_CACHE_SIZE: usize = 10000;
const MAX_MEMPOOL_SIZE: usize = 5000;
const MIN_FEE_RATE: f64 = 1.0;

#[derive(Debug)]
pub enum MempoolError {
    LockError(String),
    ValidationError(String),
    UTXOError(String),
    CacheError(String),
}

impl From<Box<dyn std::error::Error>> for MempoolError {
    fn from(error: Box<dyn std::error::Error>) -> Self {
        MempoolError::ValidationError(error.to_string())
    }
}

impl std::fmt::Display for MempoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MempoolError::LockError(msg) => write!(f, "Lock error: {}", msg),
            MempoolError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            MempoolError::UTXOError(msg) => write!(f, "UTXO error: {}", msg),
            MempoolError::CacheError(msg) => write!(f, "Cache error: {}", msg),
        }
    }
}

impl std::error::Error for MempoolError {}

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
            return Err(MempoolError::ValidationError("Mempool is full".to_string()));
        }

        // 并行验证交易
        txs.into_par_iter()
            .try_for_each(|tx| self.add_transaction(tx))?;

        Ok(())
    }

    pub fn add_transaction(&self, tx: Transaction) -> Result<()> {
        let txid = tx.id.clone();

        // 检查交易是否已经在内存池中
        if self.transactions.contains_key(&txid) {
            return Ok(());
        }

        // 检查缓存中是否有验证结果
        let is_valid = {
            let cache = self.validation_cache.read();
            cache.peek(&txid).copied()
        };

        if let Some(valid) = is_valid {
            if !valid {
                return Err(MempoolError::ValidationError("Transaction validation failed (cached)".to_string()));
            }
        } else {
            // 验证交易
            if !tx.verify(&*self.utxo_set)? {
                return Err(MempoolError::ValidationError("Invalid transaction".to_string()));
            }

            // 缓存验证结果
            self.validation_cache.write().put(txid.clone(), true);
        }

        // 计算交易费用
        let fee = tx.calculate_fee_rate();
        if fee < MIN_FEE_RATE {
            return Err(MempoolError::ValidationError("Transaction fee is too low".to_string()));
        }

        // 如果mempool已满，移除费率最低的交易
        if self.transactions.len() >= self.max_size {
            self.remove_lowest_fee_transaction()?;
        }

        // 添加到内存池
        let entry = TransactionEntry {
            transaction: tx,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            fee,
            validation_result: true,
        };

        self.transactions.insert(txid, entry);
        Ok(())
    }

    pub fn get_transaction(&self, txid: &str) -> Option<Transaction> {
        self.transactions.get(txid).map(|entry| entry.transaction.clone())
    }

    pub fn remove_transaction(&self, txid: &str) {
        self.transactions.remove(txid);
        self.validation_cache.write().pop(txid);
    }

    pub fn get_transactions(&self) -> Vec<Transaction> {
        self.transactions
            .iter()
            .map(|entry| entry.value().transaction.clone())
            .collect()
    }

    pub fn clear(&self) {
        self.transactions.clear();
        self.validation_cache.write().clear();
    }

    pub fn size(&self) -> usize {
        self.transactions.len()
    }

    fn remove_lowest_fee_transaction(&self) -> Result<()> {
        let lowest_fee_tx = self
            .transactions
            .iter()
            .min_by(|a, b| {
                a.value()
                    .fee
                    .partial_cmp(&b.value().fee)
                    .unwrap()
            })
            .map(|entry| entry.key().clone());

        if let Some(txid) = lowest_fee_tx {
            self.transactions.remove(&txid);
            self.validation_cache.write().pop(&txid);
        }

        Ok(())
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

    #[allow(dead_code)]
    fn validate_transaction(&self, tx: &Transaction) -> Result<()> {
        // 检查交易输入是否存在于UTXO集中
        for input in &tx.inputs {
            if !self.utxo_set.exists_utxo(&input.txid, input.vout)? {
                return Err(MempoolError::ValidationError(
                    format!("Input UTXO not found: {}:{}", input.txid, input.vout)
                ));
            }
        }

        // 验证交易签名
        if !tx.verify(&*self.utxo_set)? {
            return Err(MempoolError::ValidationError("Transaction signature verification failed".to_string()));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mempool_basic() -> Result<()> {
        let utxo_set = Arc::new(UTXOSet::new());
        let mempool = Mempool::new(utxo_set.clone());

        // Create a test transaction
        let tx = Transaction::new_coinbase("test_address", "test")?;

        // Add transaction to mempool
        mempool.add_transaction(tx.clone())?;

        // Verify transaction is in mempool
        assert!(mempool.get_transaction(&tx.id).is_some());

        Ok(())
    }

    #[test]
    fn test_mempool_expired_transactions() -> Result<()> {
        let utxo_set = Arc::new(UTXOSet::new());
        let mempool = Mempool::new(utxo_set.clone());

        // Add test transaction
        let tx = Transaction::new_coinbase("test_address", "test")?;
        mempool.add_transaction(tx.clone())?;

        // Clear mempool
        mempool.clear();

        // Verify transaction is removed
        assert!(mempool.get_transaction(&tx.id).is_none());

        Ok(())
    }
}

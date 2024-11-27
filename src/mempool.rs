use std::sync::Arc;
use std::num::NonZeroUsize;

use dashmap::DashMap;
use lru::LruCache;
use parking_lot::RwLock;

use crate::transaction::Transaction;
use crate::utxo::UTXOSet;
use super::error::{Result, RustBtcError};

const MAX_CACHE_SIZE: usize = 10000;
const MAX_MEMPOOL_SIZE: usize = 5000;
const MAX_TRANSACTION_SIZE: usize = 100_000;

#[derive(Debug, Clone)]
struct TransactionEntry {
    transaction: Transaction,
}

impl TransactionEntry {
    fn new(transaction: Transaction) -> Result<Self> {
        let _fee = transaction.calculate_fee_rate();
        
        Ok(Self {
            transaction,
        })
    }
}

pub struct Mempool {
    transactions: DashMap<String, TransactionEntry>,
    max_size: usize,
    recent_txs: RwLock<LruCache<String, ()>>,
    utxo_set: Arc<UTXOSet>,
}

impl Mempool {
    pub fn new(utxo_set: Arc<UTXOSet>) -> Self {
        Self {
            transactions: DashMap::new(),
            max_size: MAX_MEMPOOL_SIZE,
            recent_txs: RwLock::new(LruCache::new(NonZeroUsize::new(MAX_CACHE_SIZE).unwrap())),
            utxo_set,
        }
    }

    pub fn add_transaction(&mut self, tx: Transaction) -> Result<()> {
        let tx_size = bincode::serialize(&tx)
            .map_err(|e| RustBtcError::SerializationError(e.to_string()))?
            .len();

        if tx_size > MAX_TRANSACTION_SIZE {
            return Err(RustBtcError::InvalidTransaction(format!(
                "交易大小 {} 超过最大限制 {}",
                tx_size, MAX_TRANSACTION_SIZE
            )));
        }

        if self.transactions.len() >= self.max_size {
            return Err(RustBtcError::CapacityExceeded(format!(
                "内存池已达到最大容量 {}",
                self.max_size
            )));
        }

        let tx_hash = tx.hash()?;
        if self.transactions.contains_key(&tx_hash) {
            return Err(RustBtcError::DuplicateTransaction(format!(
                "交易 {} 已存在于内存池中",
                tx_hash
            )));
        }

        if !self.validate_transaction(&tx)? {
            return Err(RustBtcError::ValidationError("交易验证失败".to_string()));
        }

        let entry = TransactionEntry::new(tx)?;
        self.transactions.insert(tx_hash.clone(), entry);
        self.recent_txs.write().put(tx_hash, ());
        Ok(())
    }

    pub fn add_transactions(&mut self, txs: Vec<Transaction>) -> Result<()> {
        if self.transactions.len() + txs.len() > self.max_size {
            return Err(RustBtcError::CapacityExceeded(format!(
                "内存池已达到最大容量 {}",
                self.max_size
            )));
        }

        for tx in txs {
            self.add_transaction(tx)?;
        }

        Ok(())
    }

    pub fn get_transaction(&self, tx_hash: &str) -> Result<Transaction> {
        self.transactions
            .get(tx_hash)
            .map(|entry| entry.transaction.clone())
            .ok_or_else(|| RustBtcError::TransactionNotFound(tx_hash.to_string()))
    }

    pub fn remove_transaction(&self, tx_hash: &str) -> Result<()> {
        self.transactions
            .remove(tx_hash)
            .ok_or_else(|| RustBtcError::TransactionNotFound(tx_hash.to_string()))?;
        Ok(())
    }

    pub fn get_all_transactions(&self) -> Vec<Transaction> {
        self.transactions
            .iter()
            .map(|entry| entry.transaction.clone())
            .collect()
    }

    pub fn size(&self) -> usize {
        self.transactions.len()
    }

    fn validate_transaction(&self, tx: &Transaction) -> Result<bool> {
        if tx.vin.is_empty() || tx.vout.is_empty() {
            return Err(RustBtcError::ValidationError("交易的输入或输出不能为空".to_string()));
        }

        // 验证所有输入
        let mut total_input = 0;
        for input in &tx.vin {
            if input.value <= 0 {
                return Err(RustBtcError::InvalidAmount("输入金额必须为正数".to_string()));
            }
            total_input += input.value;
        }

        // 验证所有输出
        let mut total_output = 0;
        for output in &tx.vout {
            if output.value <= 0 {
                return Err(RustBtcError::InvalidAmount("输出金额必须为正数".to_string()));
            }
            total_output += output.value;
        }

        // 验证输入总额大于输出总额
        if total_input <= total_output {
            return Err(RustBtcError::InvalidAmount(format!(
                "输入总额 {} 必须大于输出总额 {}",
                total_input, total_output
            )));
        }

        // 验证所有输入的 UTXO
        for input in &tx.vin {
            if !self.utxo_set.verify_input(input)? {
                return Err(RustBtcError::UTXOError(format!(
                    "UTXO {}:{} 验证失败: 不存在、已被使用或签名无效",
                    input.txid, input.vout
                )));
            }
        }

        // 验证交易签名
        if !tx.verify(&self.utxo_set)? {
            return Err(RustBtcError::ValidationError("交易验证失败".to_string()));
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use crate::Wallet;

    use super::*;

    #[test]
    fn test_mempool_basic_operations() -> Result<()> {
        let utxo_set = Arc::new(UTXOSet::new());
        let mut mempool = Mempool::new(utxo_set);
        let wallet = Wallet::new().map_err(|e| RustBtcError::ValidationError(e.to_string()))?;

        // Create test transaction
        let tx = Transaction::new_coinbase(&wallet.get_address(), "test")
            .map_err(|e| RustBtcError::ValidationError(e.to_string()))?;
        
        // Add transaction to mempool
        mempool.add_transaction(tx.clone())?;

        // Verify transaction exists
        let tx_hash = tx.hash()?;
        let retrieved_tx = mempool.get_transaction(&tx_hash)?;
        assert_eq!(retrieved_tx.hash()?, tx_hash);

        // Remove transaction
        mempool.remove_transaction(&tx_hash)?;
        assert!(mempool.get_transaction(&tx_hash).is_err());

        Ok(())
    }

    #[test]
    fn test_mempool_duplicate_transaction() -> Result<()> {
        let utxo_set = Arc::new(UTXOSet::new());
        let mut mempool = Mempool::new(utxo_set);
        let wallet = Wallet::new().map_err(|e| RustBtcError::ValidationError(e.to_string()))?;

        // Create test transaction
        let tx = Transaction::new_coinbase(&wallet.get_address(), "test")
            .map_err(|e| RustBtcError::ValidationError(e.to_string()))?;

        // First addition should succeed
        mempool.add_transaction(tx.clone())?;

        // Second addition should fail
        assert!(matches!(
            mempool.add_transaction(tx.clone()),
            Err(RustBtcError::DuplicateTransaction(_))
        ));

        Ok(())
    }

    #[test]
    fn test_mempool_capacity() -> Result<()> {
        let utxo_set = Arc::new(UTXOSet::new());
        let mut mempool = Mempool::new(utxo_set);
        let wallet = Wallet::new().map_err(|e| RustBtcError::ValidationError(e.to_string()))?;

        // Create test transaction
        let tx = Transaction::new_coinbase(&wallet.get_address(), "test")
            .map_err(|e| RustBtcError::ValidationError(e.to_string()))?;

        // Add transactions until capacity limit
        for _ in 0..MAX_MEMPOOL_SIZE {
            let _ = mempool.add_transaction(tx.clone());
        }

        // Adding one more transaction should fail
        assert!(matches!(
            mempool.add_transaction(tx.clone()),
            Err(RustBtcError::CapacityExceeded(_))
        ));

        Ok(())
    }
}

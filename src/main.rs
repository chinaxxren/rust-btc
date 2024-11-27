use std::error::Error;
use std::collections::HashMap;
use rust_btc::blockchain::Blockchain;
use rust_btc::transaction::Transaction;
use rust_btc::utxo::UTXOSet;
use rust_btc::wallet::Wallet;
use rust_btc::mempool::Mempool;

mod block;
mod blockchain;
mod transaction;
mod utxo;
mod wallet;
mod mempool;

fn main() -> Result<(), Box<dyn Error>> {
    println!("清理数据...");
    cleanup_data()?;
    println!("数据清理完成\n");

    test_transaction_with_mempool()?;

    Ok(())
}

fn cleanup_data() -> Result<(), Box<dyn Error>> {
    let paths = [
        "data/blockchain.db",
        "data/utxo.db",
        "data/wallet.dat",
    ];
    
    for path in paths.iter() {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        if std::path::Path::new(path).exists() {
            std::fs::remove_file(path)?;
        }
    }
    
    Ok(())
}

fn test_transaction_with_mempool() -> Result<(), Box<dyn Error>> {
    println!("测试交易功能...");
    
    // 创建钱包
    let wallet1 = Wallet::new()?;
    let wallet2 = Wallet::new()?;
    
    println!("钱包1地址: {}", wallet1.get_address());
    println!("钱包2地址: {}", wallet2.get_address());
    
    // 创建新的区块链
    println!("创建新的区块链...\n");
    let mut bc = Blockchain::new(&wallet1.get_address())?;
    
    // 创建UTXO集
    let mut utxo_set = UTXOSet::new()?;
    utxo_set.reindex(&bc)?;
    
    // 创建交易池
    let mempool = Mempool::new();
    
    // 创建coinbase交易
    let coinbase_tx = Transaction::new_coinbase(&wallet1.get_address(), "Mining reward")?;
    
    // 将coinbase交易添加到区块链
    bc.add_block(vec![coinbase_tx.clone()])?;
    utxo_set.reindex(&bc)?;
    
    // 计算初始余额
    let balance1 = utxo_set.find_spendable_outputs(&wallet1.get_address(), 0)?;
    let balance2 = utxo_set.find_spendable_outputs(&wallet2.get_address(), 0)?;
    
    println!("初始余额:");
    println!("钱包1余额: {} 代币", balance1.0);
    println!("钱包2余额: {} 代币\n", balance2.0);
    
    println!("执行转账: 从钱包1向钱包2转账10代币\n");
    
    // 创建转账交易
    let tx = Transaction::new_transaction(
        &wallet1.get_address(),
        &wallet2.get_address(),
        10,
        &utxo_set,
    )?;
    
    // 收集前一笔交易用于签名
    let mut prev_txs = HashMap::new();
    for input in &tx.inputs {
        if let Some(prev_tx) = bc.find_transaction(&input.txid)? {
            prev_txs.insert(input.txid.clone(), prev_tx);
        }
    }
    
    // 签名交易
    let mut signed_tx = tx.clone();
    signed_tx.sign(&wallet1, &prev_txs)?;
    
    // 添加交易到交易池
    println!("添加交易到交易池...");
    mempool.add_transaction(signed_tx.clone(), &utxo_set)?;
    
    // 获取最优交易列表
    println!("从交易池获取最优交易...");
    let best_txs = mempool.get_best_transactions(10);
    
    // 创建新区块，包含交易池中的交易
    println!("创建新区块，包含交易池中的交易...");
    bc.add_block(best_txs)?;
    
    // 更新UTXO集
    println!("更新UTXO集...");
    utxo_set.reindex(&bc)?;
    
    // 从交易池中移除已确认的交易
    println!("从交易池中移除已确认的交易...");
    mempool.remove_transaction(&signed_tx.id)?;
    
    // 计算最终余额
    let final_balance1 = utxo_set.find_spendable_outputs(&wallet1.get_address(), 0)?;
    let final_balance2 = utxo_set.find_spendable_outputs(&wallet2.get_address(), 0)?;
    
    println!("\n最终余额:");
    println!("钱包1余额: {} 代币", final_balance1.0);
    println!("钱包2余额: {} 代币", final_balance2.0);
    
    // 测试交易池的其他功能
    println!("\n测试交易池其他功能:");
    println!("交易池大小: {}", mempool.size());
    println!("清理过期交易...");
    mempool.clear_expired(3600)?; // 清理超过1小时的交易
    
    println!("\n测试完成!");
    Ok(())
}

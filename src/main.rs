use std::error::Error;
use std::fs;
use std::sync::Arc;
use std::thread;
use std::time;
use parking_lot::RwLock;

mod block;
mod blockchain;
mod transaction;
mod utxo;
mod wallet;
mod mempool;

use block::Block;
use blockchain::Blockchain;
use mempool::Mempool;
use transaction::Transaction;
use utxo::UTXOSet;
use wallet::Wallet;

fn cleanup_data() -> Result<(), Box<dyn Error>> {
    let _ = fs::remove_dir_all("data");
    fs::create_dir_all("data")?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("清理数据...");
    cleanup_data()?;
    println!("数据清理完成\n");

    println!("测试交易功能...");

    // 创建钱包
    let wallet1 = Wallet::new()?;
    let wallet2 = Wallet::new()?;
    println!("钱包1地址: {}", wallet1.get_address());
    println!("钱包2地址: {}", wallet2.get_address());

    // 创建新的区块链
    println!("\n1. 测试区块链创建...");
    let mut blockchain = Blockchain::new()?;
    println!("区块链创建成功！");

    // 创建创世区块
    println!("\n2. 测试创世区块...");
    let coinbase_tx = Transaction::new_coinbase(&wallet1.get_address(), "Genesis Block")?;
    let mut genesis_block = Block::new(vec![coinbase_tx.clone()], String::new())?;
    genesis_block.mine_block(4)?;
    blockchain.add_block(genesis_block)?;
    println!("创世区块创建成功！");
    
    // 创建并初始化UTXO集
    println!("\n3. 测试UTXO集...");
    let utxo_set = UTXOSet::new();
    let utxo_set_arc = Arc::new(RwLock::new(utxo_set));
    utxo_set_arc.write().reindex(&blockchain)?;
    println!("UTXO集创建成功！");

    // 等待UTXO集更新
    thread::sleep(time::Duration::from_secs(1));

    // 检查初始余额
    let wallet1_balance = utxo_set_arc.read().get_balance(&wallet1.get_address())?;
    let wallet2_balance = utxo_set_arc.read().get_balance(&wallet2.get_address())?;
    println!("初始余额检查:");
    println!("  钱包1余额: {} coins", wallet1_balance);
    println!("  钱包2余额: {} coins", wallet2_balance);
    assert_eq!(wallet1_balance, 50, "钱包1应该有50个挖矿奖励");
    assert_eq!(wallet2_balance, 0, "钱包2初始余额应该为0");
    
    // 测试交易创建和验证
    println!("\n4. 测试交易创建和验证...");
    let mut tx1 = Transaction::new(
        &wallet1,
        &wallet2.get_address(),
        20,
        &*utxo_set_arc.read(),
    )?;
    
    // 签名交易
    tx1.sign(&wallet1)?;
    println!("交易创建和签名成功！");
    
    // 验证交易
    assert!(tx1.verify(&*utxo_set_arc.read())?, "交易验证应该通过");
    println!("交易验证成功！");
    
    // 测试区块创建和挖矿
    println!("\n5. 测试区块创建和挖矿...");
    let mut block2 = Block::new(vec![tx1.clone()], blockchain.get_last_hash()?)?;
    block2.mine_block(4)?;
    blockchain.add_block(block2)?;
    println!("区块创建和挖矿成功！");
    
    // 更新并验证UTXO集
    println!("\n6. 测试UTXO更新...");
    utxo_set_arc.write().reindex(&blockchain)?;
    
    // 等待UTXO集更新
    thread::sleep(time::Duration::from_secs(1));
    
    let wallet1_balance = utxo_set_arc.read().get_balance(&wallet1.get_address())?;
    let wallet2_balance = utxo_set_arc.read().get_balance(&wallet2.get_address())?;
    println!("交易后余额检查:");
    println!("  钱包1余额: {} coins", wallet1_balance);
    println!("  钱包2余额: {} coins", wallet2_balance);
    assert_eq!(wallet2_balance, 20, "钱包2应该收到20个币");
    assert_eq!(wallet1_balance, 30, "钱包1应该剩余30个币");
    
    // 测试内存池功能
    println!("\n7. 测试内存池功能...");
    let mempool = Mempool::new(Arc::new(utxo_set_arc.read().clone()));
    
    // 创建并添加有效交易
    let mut valid_tx = Transaction::new(
        &wallet2,
        &wallet1.get_address(),
        5,
        &*utxo_set_arc.read(),
    )?;
    
    // 签名交易
    valid_tx.sign(&wallet2)?;
    println!("测试交易创建和签名成功！");
    
    // 验证并添加到内存池
    assert!(valid_tx.verify(&*utxo_set_arc.read())?, "测试交易验证应该通过");
    mempool.add_transaction(valid_tx.clone())?;
    println!("有效交易添加成功！");
    assert_eq!(mempool.size(), 1, "内存池应该包含1笔交易");
    
    // 尝试创建无效交易（金额超过余额）
    println!("\n8. 测试无效交易处理...");
    let result = Transaction::new(
        &wallet2,
        &wallet1.get_address(),
        100, // 超过余额
        &*utxo_set_arc.read(),
    );
    assert!(result.is_err(), "创建超额交易应该失败");
    println!("无效交易被正确拒绝！");
    
    // 测试区块链持久化
    println!("\n9. 测试区块链持久化...");
    blockchain.save_to_file()?;
    let loaded_blockchain = Blockchain::load_from_file()?;
    assert_eq!(
        blockchain.get_last_hash()?,
        loaded_blockchain.get_last_hash()?,
        "保存和加载的区块链应该相同"
    );
    println!("区块链持久化测试成功！");
    
    println!("\n所有核心功能测试完成！");
    Ok(())
}

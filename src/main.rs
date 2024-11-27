use std::fs;
use std::sync::Arc;
use std::thread;
use std::time;
use parking_lot::RwLock;

use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use rust_btc::{
    Block,
    Blockchain,
    Mempool,
    Transaction,
    UTXOSet,
    Wallet,
    error::{Result, RustBtcError},
};

fn cleanup_data() -> Result<()> {
    let _ = fs::remove_dir_all("data");
    fs::create_dir_all("data").map_err(|e| RustBtcError::IOError(e.to_string()))?;
    Ok(())
}

fn main() -> Result<()> {
    // 初始化日志记录器
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_target(false)
        .with_ansi(true)
        .with_level(true)
        .with_writer(std::io::stdout)
        .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
        .pretty()
        .try_init();

    if let Err(e) = subscriber {
        eprintln!("Failed to initialize logger: {}", e);
        return Ok(());
    }

    info!("清理数据...");
    cleanup_data()?;
    info!("数据清理完成\n");

    info!("测试交易功能...");

    // 创建钱包
    let wallet1 = Wallet::new()?;
    let wallet2 = Wallet::new()?;
    info!("钱包1地址: {}", wallet1.get_address());
    info!("钱包2地址: {}", wallet2.get_address());

    // 创建新的区块链
    info!("\n1. 测试区块链创建...");
    let mut blockchain = Blockchain::new()?;
    info!("区块链创建成功！");

    // 创建创世区块
    info!("\n2. 测试创世区块...");
    let coinbase_tx = Transaction::new_coinbase(&wallet1.get_address(), "Genesis Block")?;
    let mut genesis_block = Block::new(vec![coinbase_tx], String::new())?;
    genesis_block.mine_block(4)?;
    blockchain.add_block(genesis_block)?;
    info!("创世区块创建成功！");
    
    // 创建并初始化UTXO集
    info!("\n3. 测试UTXO集...");
    let utxo_set = UTXOSet::new();
    let utxo_set_arc = Arc::new(RwLock::new(utxo_set));
    utxo_set_arc.write().reindex(&blockchain)?;
    info!("UTXO集创建成功！");

    // 等待UTXO集更新
    thread::sleep(time::Duration::from_secs(1));

    // 检查初始余额
    let wallet1_balance = utxo_set_arc.read().get_balance(&wallet1.get_address())?;
    let wallet2_balance = utxo_set_arc.read().get_balance(&wallet2.get_address())?;
    info!("初始余额检查:");
    info!("  钱包1余额: {} coins", wallet1_balance);
    info!("  钱包2余额: {} coins", wallet2_balance);
    assert_eq!(wallet1_balance, 50, "钱包1应该有50个挖矿奖励");
    assert_eq!(wallet2_balance, 0, "钱包2初始余额应该为0");
    
    // 测试交易创建和验证
    info!("\n4. 测试交易创建和验证...");
    let mut tx1 = Transaction::new(
        &wallet1,
        &wallet2.get_address(),
        20,
        &*utxo_set_arc.read(),
    )?;
    
    // 签名交易
    tx1.sign(&wallet1)?;
    info!("交易创建和签名成功！");
    
    // 验证交易
    assert!(tx1.verify(&*utxo_set_arc.read())?, "交易验证应该通过");
    info!("交易验证成功！");
    
    // 测试区块创建和挖矿
    info!("\n5. 测试区块创建和挖矿...");
    let coinbase_tx = Transaction::new_coinbase(&wallet1.get_address(), "Mining reward")?;
    let mut block2 = Block::new(vec![coinbase_tx, tx1.clone()], blockchain.get_last_hash()?)?;
    block2.mine_block(4)?;
    blockchain.add_block(block2)?;
    info!("区块创建和挖矿成功！");

    // 等待一会儿，确保区块已经添加成功
    thread::sleep(time::Duration::from_secs(1));

    // 更新并验证UTXO集
    info!("\n6. 测试UTXO更新...");
    utxo_set_arc.write().reindex(&blockchain)?;

    // 等待UTXO集更新
    thread::sleep(time::Duration::from_secs(1));

    // 检查余额
    info!("交易后余额检查:");
    info!("  钱包1余额: {} coins", wallet1_balance);
    info!("  钱包2余额: {} coins", wallet2_balance);

    // 验证余额
    let wallet1_balance = utxo_set_arc.read().get_balance(&wallet1.get_address())?;
    let wallet2_balance = utxo_set_arc.read().get_balance(&wallet2.get_address())?;
    assert_eq!(wallet1_balance, 79, "钱包1应该剩余79个币"); // 50(挖矿) + 29(找零)
    assert_eq!(wallet2_balance, 20, "钱包2应该有20个币");

    // 测试内存池功能
    info!("\n7. 测试内存池功能...");
    let mut mempool = Mempool::new(Arc::new(utxo_set_arc.read().clone()));
    
    // 创建并添加有效交易
    let mut valid_tx = Transaction::new(
        &wallet2,
        &wallet1.get_address(),
        5,
        &*utxo_set_arc.read(),
    )?;
    
    // 签名交易
    valid_tx.sign(&wallet2)?;
    info!("测试交易创建和签名成功！");
    
    // 验证并添加到内存池
    assert!(valid_tx.verify(&*utxo_set_arc.read())?, "测试交易验证应该通过");
    mempool.add_transaction(valid_tx.clone())?;
    info!("有效交易添加成功！");
    assert_eq!(mempool.size(), 1, "内存池应该包含1笔交易");
    
    // 尝试创建无效交易（金额超过余额）
    info!("\n8. 测试无效交易处理...");
    let result = Transaction::new(
        &wallet2,
        &wallet1.get_address(),
        100, // 超过余额
        &*utxo_set_arc.read(),
    );
    assert!(result.is_err(), "创建超额交易应该失败");
    info!("无效交易被正确拒绝！");
    
    // 测试区块链持久化
    info!("\n9. 测试区块链持久化...");
    blockchain.save_to_file()?;
    let loaded_blockchain = Blockchain::load_from_file()?;
    assert_eq!(
        blockchain.get_last_hash()?,
        loaded_blockchain.get_last_hash()?,
        "保存和加载的区块链应该相同"
    );
    info!("区块链持久化测试成功！");
    
    info!("\n所有核心功能测试完成！");
    Ok(())
}

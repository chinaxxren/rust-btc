use std::error::Error;
use std::collections::HashMap;

use crate::blockchain::Blockchain;
use crate::transaction::Transaction;
use crate::wallet::Wallet;

mod block;
mod blockchain;
mod transaction;
mod utxo;
mod wallet;

fn main() -> Result<(), Box<dyn Error>> {
    // 清理旧数据
    Blockchain::cleanup()?;
    
    // 运行测试
    test_transactions()?;
    
    Ok(())
}

fn test_blockchain() -> Result<(), Box<dyn Error>> {
    println!("\n测试区块链功能...");
    
    // 创建一个新的区块链
    let miner_wallet = Wallet::new()?;
    let miner_address = miner_wallet.get_address();
    let mut bc = Blockchain::new(&miner_address)?;
    
    // 获取最新区块
    if let Some(blocks) = bc.get_blocks()?.last() {
        println!("最新区块哈希: {}", blocks.hash);
        println!("前一区块哈希: {}", blocks.prev_block_hash);
        println!("交易数量: {}", blocks.transactions.len());
    } else {
        println!("区块链为空!");
    }
    
    // 创建一个新的交易
    let wallet1 = Wallet::new()?;
    let wallet2 = Wallet::new()?;
    
    let tx = Transaction::new_transaction(
        &wallet1.get_address(),
        &wallet2.get_address(),
        10,
        bc.get_utxo_set()
    )?;
    
    // 添加新区块
    bc.add_block(vec![tx])?;
    
    println!("区块链测试完成!");
    Ok(())
}

fn test_transactions() -> Result<(), Box<dyn Error>> {
    println!("\n测试交易功能...");
    
    // 创建两个钱包
    let wallet1 = Wallet::new()?;
    let wallet2 = Wallet::new()?;
    
    let address1 = wallet1.get_address();
    let address2 = wallet2.get_address();
    
    println!("钱包1地址: {}", address1);
    println!("钱包2地址: {}", address2);
    
    // 创建区块链，钱包1作为矿工
    let mut blockchain = Blockchain::new(&address1)?;
    
    // 等待一个区块确认
    std::thread::sleep(std::time::Duration::from_secs(5));
    
    // 打印初始余额
    let balance1 = blockchain.get_balance(&address1)?;
    let balance2 = blockchain.get_balance(&address2)?;
    
    println!("\n初始余额:");
    println!("钱包1余额: {} 代币", balance1);
    println!("钱包2余额: {} 代币", balance2);
    
    // 尝试从钱包1转账到钱包2
    if balance1 >= 10 {
        println!("\n执行转账: 从钱包1向钱包2转账10代币");
        
        // 创建交易
        let mut tx = Transaction::new_transaction(
            &address1,
            &address2,
            10,
            blockchain.get_utxo_set()
        )?;
        
        // 获取前一笔交易用于签名
        let mut prev_txs = HashMap::new();
        for input in &tx.inputs {
            if let Some(prev_tx) = blockchain.find_transaction(&input.txid)? {
                prev_txs.insert(input.txid.clone(), prev_tx);
            }
        }
        
        // 签名交易
        println!("\n签名交易...");
        tx.sign(&wallet1, &prev_txs)?;
        
        // 验证交易
        println!("\n验证交易签名...");
        if tx.verify(&prev_txs)? {
            println!("交易签名验证成功!");
            
            // 创建新区块
            blockchain.add_block(vec![tx])?;
            println!("转账交易已打包到新区块");
            
            // 获取转账后的余额
            let balance1 = blockchain.get_balance(&address1)?;
            let balance2 = blockchain.get_balance(&address2)?;
            println!("\n转账后余额:");
            println!("钱包1余额: {} 代币", balance1);
            println!("钱包2余额: {} 代币", balance2);
        } else {
            println!("交易签名验证失败!");
        }
    } else {
        println!("\n钱包1余额不足，无法进行交易");
    }
    
    println!("\n测试完成!");
    Ok(())
}

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{self, SystemTime, UNIX_EPOCH};

use tracing::info;
use tokio;

use rust_btc::{
    Block,
    blockchain::Blockchain,
    error::Result,
    network::Message,
    network::P2PNetwork,
    storage::Storage,
    transaction::Transaction,
    utxo::UTXOSet,
    wallet::Wallet,
};

fn cleanup_data() -> Result<()> {
    let _ = std::fs::remove_dir_all("data");
    std::fs::create_dir_all("data").map_err(|e| rust_btc::error::RustBtcError::Io(e))?;
    Ok(())
}

async fn test_p2p_network() -> Result<()> {
    info!("测试P2P网络功能...");

    // 创建三个节点的存储
    let storage1 = Arc::new(Storage::new("data/node1")?);
    let storage2 = Arc::new(Storage::new("data/node2")?);
    let storage3 = Arc::new(Storage::new("data/node3")?);

    // 创建三个网络节点
    let node1_addr: SocketAddr = "127.0.0.1:8001".parse().unwrap();
    let node2_addr: SocketAddr = "127.0.0.1:8002".parse().unwrap();
    let node3_addr: SocketAddr = "127.0.0.1:8003".parse().unwrap();

    let node1: Arc<P2PNetwork> = P2PNetwork::new(node1_addr, Arc::clone(&storage1)).await?;
    let node1_clone: Arc<P2PNetwork> = Arc::clone(&node1);
    tokio::spawn(async move {
        node1_clone.start().await.unwrap();
    });

    let node2 = P2PNetwork::new(node2_addr, Arc::clone(&storage2)).await?;
    node2.connect_to_peer(node1_addr).await?;
    let node2 = Arc::new(node2);
    let node2_clone: Arc<P2PNetwork> = Arc::clone(&node2);
    tokio::spawn(async move {
        node2_clone.start().await.unwrap();
    });

    let node3 = P2PNetwork::new(node3_addr, Arc::clone(&storage3)).await?;
    node3.connect_to_peer(node1_addr).await?;
    let node3 = Arc::new(node3);
    let node3_clone: Arc<P2PNetwork> = Arc::clone(&node3);
    tokio::spawn(async move {
        node3_clone.start().await.unwrap();
    });

    // 等待节点启动
    tokio::time::sleep(time::Duration::from_secs(2)).await;

    // 测试节点连接
    info!("测试节点2连接到节点1");
    if let Some(peer) = node2.get_peer_addresses().await.first() {
        node2.connect_to_peer(*peer).await?;
    }

    info!("测试节点3连接到节点2");
    if let Some(peer) = node3.get_peer_addresses().await.first() {
        node3.connect_to_peer(*peer).await?;
    }

    // 等待连接建立
    tokio::time::sleep(time::Duration::from_secs(2)).await;

    // 创建一个测试区块
    let test_block = Block {
        version: 1,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        transactions: vec![],
        prev_block_hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        merkle_root: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        hash: String::new(),
        nonce: 0,
        height: 0,
        bits: 0x1d00ffff,
    };

    // 模拟节点1挖矿成功，广播新区块
    info!("模拟节点1挖矿成功，广播新区块");
    node1.broadcast_message(Message::MiningSuccess(test_block.clone())).await?;

    // 等待区块同步
    tokio::time::sleep(time::Duration::from_secs(2)).await;

    // 验证所有节点都收到并存储了新区块
    info!("验证区块同步");
    let block1 = storage1.get_block(0)?;
    let block2 = storage2.get_block(0)?;
    let block3 = storage3.get_block(0)?;

    assert!(block1.is_some(), "节点1未存储区块");
    assert!(block2.is_some(), "节点2未存储区块");
    assert!(block3.is_some(), "节点3未存储区块");

    info!("P2P网络测试完成\n");
    Ok(())
}

async fn test_core_features() -> Result<()> {
    info!("开始测试核心功能...");

    // 1. 创建存储实例
    let storage = Arc::new(Storage::new("data")?);
    
    // 2. 创建钱包
    info!("创建测试钱包...");
    let wallet1 = Wallet::new()?;
    let wallet2 = Wallet::new()?;
    
    info!("钱包1地址: {}", wallet1.get_address());
    info!("钱包2地址: {}", wallet2.get_address());

    // 3. 初始化区块链
    info!("初始化区块链...");
    let mut blockchain = Blockchain::new()?;
    
    // 4. 创建UTXO集
    info!("初始化UTXO集...");
    let mut utxo_set = UTXOSet::new();
    
    // 5. 创建创世区块
    info!("创建创世区块...");
    let genesis_block = Block::new_genesis_block(&wallet1.get_address())?;
    blockchain.add_block(genesis_block)?;
    
    // 6. 更新UTXO集
    info!("更新UTXO集...");
    utxo_set.reindex(&blockchain)?;
    
    // 7. 创建一笔交易
    info!("创建测试交易...");
    let amount = 30;
    let tx = Transaction::new(
        &wallet1,
        &wallet2.get_address(),
        amount,
        &utxo_set,
    )?;
    
    // 8. 创建新区块
    info!("创建新区块...");
    let new_block = Block::new(
        vec![tx],
        blockchain.get_last_hash()?.to_string(),
    )?;
    
    // 9. 添加区块到区块链
    info!("添加区块到区块链...");
    blockchain.add_block(new_block)?;
    
    // 10. 再次更新UTXO集
    info!("再次更新UTXO集...");
    utxo_set.reindex(&blockchain)?;
    
    // 11. 验证钱包余额
    info!("验证钱包余额...");
    let wallet1_balance = utxo_set.get_balance(&wallet1.get_address())?;
    let wallet2_balance = utxo_set.get_balance(&wallet2.get_address())?;
    
    info!("钱包1余额: {}", wallet1_balance);
    info!("钱包2余额: {}", wallet2_balance);
    
    // 12. 启动P2P网络节点
    info!("启动P2P网络节点...");
    let addr: SocketAddr = "127.0.0.1:8001".parse().map_err(|e: std::net::AddrParseError| {
        rust_btc::error::RustBtcError::Other(e.to_string())
    })?;
    let node = P2PNetwork::new(addr, storage.clone()).await?;
    let node = Arc::new(node);
    
    let node_clone = Arc::clone(&node);
    tokio::spawn(async move {
        node_clone.start().await.unwrap();
    });
    
    // 等待节点启动
    tokio::time::sleep(time::Duration::from_secs(1)).await;
    
    // 13. 广播最新区块
    info!("广播最新区块...");
    if let Some(block) = blockchain.blocks().last() {
        node.broadcast_message(Message::Block(block.clone())).await?;
    }
    
    info!("核心功能测试完成!");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志记录器
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_target(false)
        .with_ansi(true)
        .pretty()
        .init();

    // 清理旧数据
    cleanup_data()?;

    // 运行核心功能测试
    test_core_features().await?;

    Ok(())
}

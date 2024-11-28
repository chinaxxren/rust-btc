// 导出所有模块
pub mod block;
pub mod blockchain;
pub mod error;
pub mod mempool;
pub mod merkle;
pub mod network;
pub mod pow;
pub mod storage;
pub mod transaction;
pub mod utxo;
pub mod wallet;
pub mod models;
pub mod db;

// 导出常用类型
pub use block::Block;
pub use blockchain::Blockchain;
pub use error::{RustBtcError, Result};
pub use mempool::Mempool;
pub use merkle::MerkleTree;
pub use network::P2PNetwork;
pub use pow::ProofOfWork;
pub use storage::Storage;
pub use transaction::Transaction;
pub use utxo::UTXOSet;
pub use wallet::Wallet;
pub  use models::{WalletData, UTXOEntry};
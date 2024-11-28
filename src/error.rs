use thiserror::Error;
use std::time::SystemTimeError;

#[derive(Error, Debug)]
pub enum RustBtcError {
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("序列化错误: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("反序列化错误: {0}")]
    DeserializationError(String),

    #[error("Base58编码错误: {0}")]
    Base58(String),

    #[error("无效的签名: {0}")]
    InvalidSignature(String),

    #[error("无效的公钥: {0}")]
    InvalidPublicKey(String),

    #[error("无效的消息: {0}")]
    InvalidMessage(String),

    #[error("无效的交易: {0}")]
    InvalidTransaction(String),

    #[error("UTXO错误: {0}")]
    UTXOError(String),

    #[error("区块错误: {0}")]
    BlockError(String),

    #[error("内存池错误: {0}")]
    MempoolError(String),

    #[error("钱包错误: {0}")]
    WalletError(String),

    #[error("验证错误: {0}")]
    ValidationError(String),

    #[error("无效区块: {0}")]
    InvalidBlock(String),

    #[error("无效区块链: {0}")]
    InvalidChain(String),

    #[error("区块未找到: {0}")]
    BlockNotFound(String),

    #[error("哈希错误: {0}")]
    HashError(String),

    #[error("时间戳错误: {0}")]
    TimestampError(#[from] SystemTimeError),

    #[error("交易未找到: {0}")]
    TransactionNotFound(String),

    #[error("重复交易: {0}")]
    DuplicateTransaction(String),

    #[error("无效金额: {0}")]
    InvalidAmount(String),

    #[error("无效手续费: {0}")]
    InvalidFee(String),

    #[error("交易错误: {0}")]
    TransactionError(String),

    #[error("无效输入: {0}")]
    InvalidInput(String),

    #[error("无效输出: {0}")]
    InvalidOutput(String),

    #[error("容量超限: {0}")]
    CapacityExceeded(String),

    #[error("无效地址: {0}")]
    InvalidAddress(String),

    #[error("资金不足: {0}")]
    InsufficientFunds(String),

    #[error("UTXO未找到: {0}")]
    UTXONotFound(String),

    #[error("其他错误: {0}")]
    Other(String),

    #[error("数据库错误: {0}")]
    Database(String),
}

pub type Result<T> = std::result::Result<T, RustBtcError>;

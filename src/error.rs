use thiserror::Error;
use std::time::SystemTimeError;

#[derive(Error, Debug)]
pub enum RustBtcError {
    #[error("验证错误: {0}")]
    ValidationError(String),
    
    #[error("序列化错误: {0}")]
    SerializationError(String),
    
    #[error("反序列化错误: {0}")]
    DeserializationError(String),
    
    #[error("IO错误: {0}")]
    IOError(String),
    
    #[error("无效区块: {0}")]
    InvalidBlock(String),
    
    #[error("无效区块链: {0}")]
    InvalidChain(String),
    
    #[error("区块未找到: {0}")]
    BlockNotFound(String),
    
    #[error("哈希错误: {0}")]
    HashError(String),
    
    #[error("时间戳错误: {0}")]
    TimestampError(String),
    
    #[error("无效交易: {0}")]
    InvalidTransaction(String),
    
    #[error("交易未找到: {0}")]
    TransactionNotFound(String),
    
    #[error("重复交易: {0}")]
    DuplicateTransaction(String),
    
    #[error("无效金额: {0}")]
    InvalidAmount(String),
    
    #[error("无效手续费: {0}")]
    InvalidFee(String),
    
    #[error("无效签名: {0}")]
    InvalidSignature(String),
    
    #[error("交易错误: {0}")]
    TransactionError(String),
    
    #[error("UTXO错误: {0}")]
    UTXOError(String),
    
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
    
    #[error("其他错误: {0}")]
    Other(String),
}

impl From<std::io::Error> for RustBtcError {
    fn from(err: std::io::Error) -> Self {
        RustBtcError::IOError(err.to_string())
    }
}

impl From<bincode::Error> for RustBtcError {
    fn from(err: bincode::Error) -> Self {
        RustBtcError::SerializationError(err.to_string())
    }
}

impl From<SystemTimeError> for RustBtcError {
    fn from(err: SystemTimeError) -> Self {
        RustBtcError::Other(format!("SystemTime error: {}", err))
    }
}

pub type Result<T> = std::result::Result<T, RustBtcError>;

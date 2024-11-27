use bs58;
use sha2::{Sha256, Digest};
use serde::{Deserialize, Serialize};
use tracing::{error, debug};

use crate::error::{Result, RustBtcError};
use super::utxo::UTXOSet;
use super::wallet::Wallet;
use secp256k1::{self, ecdsa};

const SUBSIDY: i64 = 50;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TxInput {
    pub txid: String,
    pub vout: usize,
    pub signature: Vec<u8>,
    pub pubkey: Vec<u8>,
    pub value: i64,
}

impl TxInput {
    pub fn new(txid: String, vout: usize, value: i64) -> Self {
        debug!("创建新的交易输入: txid={}, vout={}, value={}", txid, vout, value);
        TxInput {
            txid,
            vout,
            signature: Vec::new(),
            pubkey: Vec::new(),
            value,
        }
    }

    pub fn verify_signature(&self, data: &[u8]) -> Result<bool> {
        debug!("验证交易输入签名: txid={}", self.txid);
        
        if self.signature.is_empty() || self.pubkey.is_empty() {
            error!("缺少签名或公钥");
            return Err(RustBtcError::InvalidSignature("缺少签名或公钥".to_string()));
        }

        debug!("待验证数据: {:?}", hex::encode(data));

        // 计算数据的哈希
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();

        let public_key = secp256k1::PublicKey::from_slice(&self.pubkey)
            .map_err(|e| {
                error!("解析公钥失败: {}", e);
                RustBtcError::InvalidSignature(e.to_string())
            })?;

        let signature = ecdsa::Signature::from_compact(&self.signature)
            .map_err(|e| {
                error!("解析签名失败: {}", e);
                RustBtcError::InvalidSignature(e.to_string())
            })?;

        let secp = secp256k1::Secp256k1::verification_only();
        let message = secp256k1::Message::from_slice(&hash)
            .map_err(|e| {
                error!("创建消息失败: {}", e);
                RustBtcError::InvalidSignature(e.to_string())
            })?;

        match secp.verify_ecdsa(&message, &signature, &public_key) {
            Ok(_) => {
                debug!("签名验证成功");
                Ok(true)
            }
            Err(e) => {
                error!("签名验证失败: {}", e);
                Ok(false)
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TxOutput {
    pub value: i64,
    pub pubkey_hash: Vec<u8>,
}

impl TxOutput {
    pub fn new(value: i64, address: &str) -> Result<Self> {
        debug!("创建新的交易输出: value={}, address={}", value, address);
        
        if value <= 0 {
            error!("交易输出金额必须大于0");
            return Err(RustBtcError::InvalidAmount(format!(
                "交易输出金额 {} 无效",
                value
            )));
        }

        let pubkey_hash = bs58::decode(address)
            .into_vec()
            .map_err(|e| RustBtcError::InvalidAddress(e.to_string()))?;

        Ok(TxOutput {
            value,
            pubkey_hash,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Transaction {
    pub id: String,
    pub vin: Vec<TxInput>,
    pub vout: Vec<TxOutput>,
}

impl Transaction {
    pub fn new(
        from_wallet: &Wallet,
        to_address: &str,
        amount: i64,
        utxo_set: &UTXOSet,
    ) -> Result<Transaction> {
        debug!("创建新的交易: from={}, to={}, amount={}", 
            from_wallet.get_address(), to_address, amount);
        
        if amount <= 0 {
            error!("交易金额必须大于0");
            return Err(RustBtcError::InvalidAmount(format!(
                "交易金额 {} 无效",
                amount
            )));
        }

        let utxos = utxo_set.find_spendable_outputs(&from_wallet.get_address(), amount)?;
        
        let mut accumulated = 0;
        let mut inputs = Vec::new();
        
        for utxo in utxos {
            accumulated += utxo.value;
            inputs.push(TxInput::new(
                utxo.txid,
                utxo.vout,
                utxo.value,
            ));
        }

        if accumulated < amount {
            error!("余额不足: 需要 {}, 可用 {}", amount, accumulated);
            return Err(RustBtcError::InsufficientFunds(format!(
                "余额不足: 需要 {}, 可用 {}",
                amount, accumulated
            )));
        }

        let mut outputs = Vec::new();
        
        // 创建接收方的输出
        outputs.push(TxOutput::new(amount, to_address)?);
        
        // 如果有找零，创建找零输出
        if accumulated > amount {
            outputs.push(TxOutput::new(
                accumulated - amount - 1, // 扣除1个币作为手续费
                &from_wallet.get_address(),
            )?);
        }

        let mut tx = Transaction {
            id: String::new(),
            vin: inputs,
            vout: outputs,
        };

        // 计算交易ID
        tx.id = tx.hash()?;
        
        // 签名交易
        tx.sign(from_wallet)?;

        debug!("交易创建成功: {}", tx.id);
        Ok(tx)
    }

    pub fn new_coinbase(to: &str, data: &str) -> Result<Transaction> {
        debug!("创建coinbase交易: to={}, data={}", to, data);
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        
        let mut tx = Transaction {
            id: String::new(),
            vin: vec![TxInput::new(
                format!("0_{}", timestamp), 
                0,
                SUBSIDY,
            )],
            vout: vec![TxOutput::new(SUBSIDY, to)?],
        };

        tx.id = tx.hash()?;
        debug!("coinbase交易创建成功: {}", tx.id);
        Ok(tx)
    }

    pub fn hash(&self) -> Result<String> {
        debug!("计算交易哈希");
        
        // 创建一个没有签名和公钥的交易副本
        let mut tx = self.clone();
        for input in &mut tx.vin {
            input.signature = Vec::new();
            input.pubkey = Vec::new();
        }
        
        let data = serde_json::to_string(&tx)
            .map_err(|e| RustBtcError::SerializationError(e.to_string()))?;
            
        let mut hasher = Sha256::new();
        hasher.update(&data);
        Ok(hex::encode(hasher.finalize()))
    }

    pub fn sign(&mut self, wallet: &Wallet) -> Result<()> {
        debug!("签名交易: {}", self.id);
        
        if self.is_coinbase() {
            debug!("coinbase交易无需签名");
            return Ok(());
        }

        // 计算交易哈希
        let tx_hash = self.hash()?;
        debug!("交易哈希: {}", tx_hash);

        // 将十六进制字符串转换为字节数组
        let tx_hash_bytes = hex::decode(&tx_hash)
            .map_err(|e| RustBtcError::InvalidSignature(e.to_string()))?;

        // 对每个输入进行签名
        for input in &mut self.vin {
            let signature = wallet.sign(&tx_hash_bytes)?;
            input.signature = signature;
            input.pubkey = wallet.get_public_key().to_vec();
            debug!("输入已签名: txid={}, vout={}", input.txid, input.vout);
        }

        Ok(())
    }

    pub fn verify(&self, utxo_set: &UTXOSet) -> Result<bool> {
        debug!("验证交易: {}", self.id);
        
        if self.is_coinbase() {
            debug!("coinbase交易无需验证");
            return Ok(true);
        }

        // 验证交易数据
        if !self.verify_transaction_data()? {
            error!("交易数据验证失败");
            return Ok(false);
        }

        // 验证每个输入
        for input in &self.vin {
            if !utxo_set.exists_utxo(&input.txid, input.vout)? {
                error!("UTXO不存在: txid={}, vout={}", input.txid, input.vout);
                return Ok(false);
            }
        }

        debug!("交易验证通过");
        Ok(true)
    }

    pub fn verify_transaction_data(&self) -> Result<bool> {
        debug!("验证交易数据: {}", self.id);
        
        // 验证输入和输出不为空
        if self.vin.is_empty() || self.vout.is_empty() {
            error!("交易输入或输出为空");
            return Ok(false);
        }

        // 验证输出金额
        for output in &self.vout {
            if output.value <= 0 {
                error!("交易输出金额无效: {}", output.value);
                return Ok(false);
            }
        }

        // 验证输入总额大于输出总额
        let input_total: i64 = self.vin.iter().map(|input| input.value).sum();
        let output_total: i64 = self.vout.iter().map(|output| output.value).sum();
        if input_total <= output_total {
            error!("输入总额 {} 必须大于输出总额 {}", input_total, output_total);
            return Ok(false);
        }

        debug!("交易数据验证通过");
        Ok(true)
    }

    pub fn calculate_fee_rate(&self) -> f64 {
        debug!("计算交易费率: {}", self.id);
        
        if self.is_coinbase() {
            return 0.0;
        }

        let mut input_value = 0;
        for input in &self.vin {
            input_value += input.value;
        }

        let mut output_value = 0;
        for output in &self.vout {
            output_value += output.value;
        }

        let fee = input_value - output_value;
        let size = serde_json::to_string(self).unwrap_or(String::new()).len() as f64;
        
        if size > 0.0 {
            fee as f64 / size
        } else {
            0.0
        }
    }

    pub fn is_coinbase(&self) -> bool {
        self.vin.len() == 1 && self.vin[0].txid.starts_with("0_")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_coinbase_transaction() -> Result<()> {
        let address = "test_address";
        let data = "test_data";
        let tx = Transaction::new_coinbase(address, data)?;

        assert!(tx.is_coinbase());
        assert_eq!(tx.vin.len(), 1);
        assert_eq!(tx.vout.len(), 1);
        assert_eq!(tx.vout[0].value, SUBSIDY);
        Ok(())
    }

    #[test]
    fn test_transaction_hash() -> Result<()> {
        let address = "test_address";
        let data = "test_data";
        let tx = Transaction::new_coinbase(address, data)?;
        
        let hash = tx.hash()?;
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA256 hash length in hex
        Ok(())
    }

    #[test]
    fn test_transaction_fee_rate() -> Result<()> {
        let address = "test_address";
        let data = "test_data";
        let tx = Transaction::new_coinbase(address, data)?;
        
        let fee_rate = tx.calculate_fee_rate();
        assert_eq!(fee_rate, 0.0); // Coinbase transactions have no fee
        Ok(())
    }
}

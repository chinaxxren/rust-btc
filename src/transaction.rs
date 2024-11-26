use std::error::Error;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use crate::utxo::UTXOSet;
use crate::wallet::Wallet;
use std::collections::HashMap;
use bs58;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInput {
    pub txid: String,
    pub vout: i32,
    pub signature: Vec<u8>,
    pub pub_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutput {
    pub value: i32,
    pub pub_key_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
}

impl Transaction {
    pub fn new(inputs: Vec<TxInput>, outputs: Vec<TxOutput>) -> Result<Transaction, Box<dyn Error>> {
        let mut tx = Transaction {
            id: String::new(),
            inputs,
            outputs,
        };
        
        tx.id = tx.hash()?;
        Ok(tx)
    }
    
    pub fn hash(&self) -> Result<String, Box<dyn Error>> {
        let mut tx_copy = self.clone();
        for input in &mut tx_copy.inputs {
            input.signature = Vec::new();
            input.pub_key = None;
        }
        
        let encoded: Vec<u8> = bincode::serialize(&tx_copy)?;
        let mut hasher = Sha256::new();
        hasher.update(&encoded);
        let hash = hasher.finalize();
        
        Ok(hex::encode(hash))
    }
    
    pub fn sign(&mut self, wallet: &Wallet, prev_txs: &HashMap<String, Transaction>) -> Result<(), Box<dyn Error>> {
        if self.is_coinbase() {
            return Ok(());
        }
        
        // 验证所有输入都有对应的前一笔交易
        for input in &self.inputs {
            if !prev_txs.contains_key(&input.txid) {
                return Err("前一笔交易不存在".into());
            }
        }
        
        // 创建一个交易副本用于签名
        let mut tx_copy = self.clone();
        
        // 清除所有输入的签名和公钥
        for input in &mut tx_copy.inputs {
            input.signature = Vec::new();
            input.pub_key = None;
        }
        
        // 对每个输入进行签名
        for i in 0..self.inputs.len() {
            // 设置当前输入的公钥
            tx_copy.inputs[i].pub_key = Some(bs58::encode(wallet.get_public_key()).into_string());
            
            // 计算交易哈希并签名
            let tx_hash = tx_copy.hash()?;
            let signature = wallet.sign(tx_hash.as_bytes())?;
            
            // 保存签名和公钥
            self.inputs[i].signature = signature;
            self.inputs[i].pub_key = Some(bs58::encode(wallet.get_public_key()).into_string());
            
            // 清除当前输入的公钥，为下一个输入做准备
            tx_copy.inputs[i].pub_key = None;
        }
        
        Ok(())
    }
    
    pub fn verify(&self, prev_txs: &HashMap<String, Transaction>) -> Result<bool, Box<dyn Error>> {
        if self.is_coinbase() {
            return Ok(true);
        }
        
        // 验证所有输入都有对应的前一笔交易
        for input in &self.inputs {
            if !prev_txs.contains_key(&input.txid) {
                return Err("前一笔交易不存在".into());
            }
        }
        
        // 创建一个交易副本用于验证
        let mut tx_copy = self.clone();
        
        // 清除所有输入的签名和公钥
        for input in &mut tx_copy.inputs {
            input.signature = Vec::new();
            input.pub_key = None;
        }
        
        // 验证每个输入的签名
        for i in 0..self.inputs.len() {
            let input = &self.inputs[i];
            
            // 设置当前输入的公钥
            tx_copy.inputs[i].pub_key = input.pub_key.clone();
            
            // 计算交易哈希
            let tx_hash = tx_copy.hash()?;
            
            // 使用公钥验证签名
            let pub_key_str = input.pub_key.as_ref().ok_or("缺少公钥")?;
            let pub_key = bs58::decode(pub_key_str).into_vec().map_err(|_| "无效的公钥")?;
            let verifying_wallet = Wallet::from_public_key(&pub_key)?;
            
            if !verifying_wallet.verify(tx_hash.as_bytes(), &input.signature)? {
                return Ok(false);
            }
            
            // 清除当前输入的公钥，为下一个输入做准备
            tx_copy.inputs[i].pub_key = None;
        }
        
        Ok(true)
    }
    
    pub fn new_coinbase(to: &str, data: &str) -> Result<Transaction, Box<dyn Error>> {
        println!("\n创建coinbase交易...");
        println!("接收方: {}", to);
        println!("奖励金额: 50代币");
        
        let input = TxInput {
            txid: String::from("0"),
            vout: -1,
            signature: Vec::new(),
            pub_key: Some(data.to_string()),
        };
        
        let output = TxOutput {
            value: 50,
            pub_key_hash: to.to_string(),
        };
        
        let tx = Transaction::new(vec![input], vec![output])?;
        println!("交易ID: {}", tx.id);
        
        Ok(tx)
    }
    
    pub fn new_transaction(from: &str, to: &str, amount: i32, utxo_set: &UTXOSet) -> Result<Transaction, Box<dyn Error>> {
        println!("\n创建新交易...");
        println!("发送方: {}", from);
        println!("接收方: {}", to);
        println!("金额: {} 代币", amount);
        
        let (total_amount, spendable_outputs) = utxo_set.find_spendable_outputs(from, amount)?;
        
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        
        for (txid, vout) in spendable_outputs {
            let input = TxInput {
                txid,
                vout,
                signature: Vec::new(),
                pub_key: Some(from.to_string()),
            };
            inputs.push(input);
        }
        
        outputs.push(TxOutput {
            value: amount,
            pub_key_hash: to.to_string(),
        });
        
        if total_amount > amount {
            outputs.push(TxOutput {
                value: total_amount - amount,
                pub_key_hash: from.to_string(),
            });
        }
        
        let tx = Transaction::new(inputs, outputs)?;
        println!("交易ID: {}", tx.id);
        
        Ok(tx)
    }
    
    pub fn is_coinbase(&self) -> bool {
        self.inputs.len() == 1 && self.inputs[0].txid == "0" && self.inputs[0].vout == -1
    }
}

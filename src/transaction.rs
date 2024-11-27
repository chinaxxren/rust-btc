use std::error::Error;
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use crate::utxo::UTXOSet;
use crate::wallet::Wallet;
use std::collections::HashMap;
use bs58;
use rayon::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInput {
    pub txid: String,     // 引用的交易ID
    pub vout: i32,        // 引用的输出索引
    pub signature: Vec<u8>, // 签名
    pub pubkey: Vec<u8>,   // 公钥
    pub value: i32,       // 输入金额
}

impl TxInput {
    pub fn new(txid: String, vout: i32, value: i32) -> Self {
        TxInput {
            txid,
            vout,
            signature: Vec::new(),
            pubkey: Vec::new(),
            value,
        }
    }
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
            input.pubkey = Vec::new();
        }
        
        let encoded: Vec<u8> = bincode::serialize(&tx_copy)?;
        let hash = Sha256::digest(&encoded);
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
        
        // 创建交易副本
        let mut tx_copy = self.clone();
        
        // 清除所有输入的签名和公钥
        for input in &mut tx_copy.inputs {
            input.signature = Vec::new();
            input.pubkey = Vec::new();
        }
        
        // 使用迭代器对每个输入进行签名
        for i in 0..self.inputs.len() {
            // 设置当前输入的公钥
            let pub_key = bs58::encode(wallet.get_public_key()).into_string();
            let pub_key_bytes = bs58::decode(&pub_key).into_vec()?;
            
            // 设置交易副本的公钥
            tx_copy.inputs[i].pubkey = pub_key_bytes.clone();
            
            // 计算交易哈希并签名
            let tx_hash = tx_copy.hash()?;
            let signature = wallet.sign(tx_hash.as_bytes())?;
            
            // 设置原始交易的签名和公钥
            self.inputs[i].signature = signature;
            self.inputs[i].pubkey = pub_key_bytes;
            
            // 清除当前输入的公钥，为下一个输入做准备
            tx_copy.inputs[i].pubkey = Vec::new();
        }
        
        Ok(())
    }
    
    pub fn verify(&self, prev_txs: &HashMap<String, Transaction>) -> Result<bool, Box<dyn Error>> {
        if self.is_coinbase() {
            return Ok(true);
        }
        
        let prev_txs = Arc::new(prev_txs.clone());
        
        // 验证所有输入
        let results: Vec<bool> = self.inputs
            .par_iter()
            .map(|input| {
                let prev_txs = Arc::clone(&prev_txs);
                match verify_input(input, self, &prev_txs) {
                    Ok(valid) => valid,
                    Err(_) => false,
                }
            })
            .collect();
        
        Ok(!results.contains(&false))
    }
    
    pub fn new_coinbase(to: &str, _data: &str) -> Result<Transaction, Box<dyn Error>> {
        println!("\n创建coinbase交易...");
        println!("接收方: {}", to);
        println!("奖励金额: 50代币");
        
        let input = TxInput {
            txid: String::from("0"),
            vout: -1,
            signature: Vec::new(),
            pubkey: Vec::new(),
            value: 0,
        };
        
        let output = TxOutput {
            value: 50,
            pub_key_hash: to.to_string(),
        };
        
        let mut tx = Transaction {
            id: String::new(),
            inputs: vec![input],
            outputs: vec![output],
        };
        
        tx.id = tx.hash()?;
        println!("交易ID: {}", tx.id);
        
        Ok(tx)
    }
    
    pub fn new_transaction(
        from: &str,
        to: &str,
        amount: i32,
        utxo_set: &UTXOSet,
    ) -> Result<Transaction, Box<dyn Error>> {
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();

        // 找到足够的UTXO
        let (acc, valid_outputs) = utxo_set.find_spendable_outputs(from, amount)?;

        if acc < amount {
            return Err("余额不足".into());
        }

        // 构建输入
        for (txid, vout) in valid_outputs {
            if let Some(utxo) = utxo_set.find_utxo(&txid, vout)? {
                let input = TxInput::new(txid, vout, utxo.value);
                inputs.push(input);
            }
        }

        // 构建输出
        outputs.push(TxOutput {
            value: amount,
            pub_key_hash: to.to_string(),
        });

        // 如果有找零，添加找零输出
        if acc > amount {
            outputs.push(TxOutput {
                value: acc - amount,
                pub_key_hash: from.to_string(),
            });
        }

        let mut tx = Transaction {
            id: String::new(),
            inputs,
            outputs,
        };

        // 设置交易ID
        tx.id = tx.hash()?;

        Ok(tx)
    }
    
    pub fn is_coinbase(&self) -> bool {
        self.inputs.len() == 1 && self.inputs[0].txid == "0" && self.inputs[0].vout == -1
    }
    
    // 计算交易的每字节费用率
    pub fn calculate_fee_rate(&self) -> f64 {
        // 简单实现：假设每个交易的大小是固定的100字节
        // 实际应用中应该计算真实的序列化大小
        const ASSUMED_TX_SIZE: f64 = 100.0;
        
        // 计算输入总额
        let input_sum: i32 = self.inputs.iter()
            .map(|input| input.value)
            .sum();
        
        // 计算输出总额
        let output_sum: i32 = self.outputs.iter()
            .map(|output| output.value)
            .sum();
        
        // 计算费用
        let fee = input_sum - output_sum;
        
        // 计算费率（每字节的费用）
        fee as f64 / ASSUMED_TX_SIZE
    }
}

// 辅助函数：验证单个输入
fn verify_input(input: &TxInput, tx: &Transaction, prev_txs: &HashMap<String, Transaction>) -> Result<bool, Box<dyn Error>> {
    let _prev_tx = prev_txs.get(&input.txid)
        .ok_or_else(|| "前一笔交易不存在".to_string())?;
    
    let pub_key = input.pubkey.clone();
    let wallet = Wallet::from_public_key(&pub_key)?;
    
    // 创建用于验证的交易副本
    let mut tx_copy = tx.clone();
    for input in &mut tx_copy.inputs {
        input.signature = Vec::new();
        input.pubkey = Vec::new();
    }
    
    let tx_hash = tx_copy.hash()?;
    wallet.verify(tx_hash.as_bytes(), &input.signature)
}

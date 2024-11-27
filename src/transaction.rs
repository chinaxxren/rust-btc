use std::error::Error;

use bs58;
use ring::digest;
use serde::{Deserialize, Serialize};

use crate::utxo::UTXOSet;
use crate::wallet::Wallet;

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
        TxInput {
            txid,
            vout,
            signature: Vec::new(),
            pubkey: Vec::new(),
            value,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TxOutput {
    pub value: i64,
    pub pub_key_hash: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Transaction {
    pub id: String,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
}

impl Transaction {
    pub fn new(
        from_wallet: &Wallet,
        to_address: &str,
        amount: i64,
        utxo_set: &UTXOSet,
    ) -> Result<Transaction, Box<dyn Error>> {
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();

        // 找到足够的UTXO
        let (accumulated, valid_outputs) = utxo_set.find_spendable_outputs(&from_wallet.get_address(), amount)?;

        if accumulated < amount {
            return Err("余额不足".into());
        }

        // 构建输入
        for (txid, outs) in valid_outputs {
            for (vout, output) in outs {
                let input = TxInput {
                    txid: txid.clone(),
                    vout,
                    signature: Vec::new(),
                    pubkey: from_wallet.get_public_key().to_vec(),
                    value: output.value,
                };
                inputs.push(input);
            }
        }

        // 构建输出
        outputs.push(TxOutput {
            value: amount,
            pub_key_hash: bs58::decode(to_address).into_vec()?,
        });

        if accumulated > amount {
            outputs.push(TxOutput {
                value: accumulated - amount,
                pub_key_hash: bs58::decode(&from_wallet.get_address()).into_vec()?,
            });
        }

        let mut tx = Transaction {
            id: String::new(),
            inputs,
            outputs,
        };

        tx.id = tx.hash()?;
        tx.sign(from_wallet)?;

        Ok(tx)
    }

    pub fn new_coinbase(to: &str, data: &str) -> Result<Transaction, Box<dyn Error>> {
        let mut tx = Transaction {
            id: String::new(),
            inputs: vec![TxInput {
                txid: String::from("0"),
                vout: 0,
                signature: Vec::new(),
                pubkey: data.as_bytes().to_vec(),
                value: SUBSIDY,
            }],
            outputs: vec![TxOutput {
                value: SUBSIDY,
                pub_key_hash: bs58::decode(to).into_vec()?,
            }],
        };

        tx.id = tx.hash()?;
        Ok(tx)
    }

    pub fn hash(&self) -> Result<String, Box<dyn Error>> {
        let data = bincode::serialize(self)?;
        let mut hasher = digest::Context::new(&digest::SHA256);
        hasher.update(&data);
        let hash = hasher.finish();
        Ok(hex::encode(hash.as_ref()))
    }

    pub fn sign(&mut self, wallet: &Wallet) -> Result<(), Box<dyn Error>> {
        // 为每个输入签名
        for input in &mut self.inputs {
            // 设置公钥
            input.pubkey = wallet.get_public_key().to_vec();
            
            // 创建签名
            let signature = wallet.sign(&input.pubkey)?;
            input.signature = signature;
        }
        Ok(())
    }

    pub fn verify(&self, utxo_set: &UTXOSet) -> Result<bool, Box<dyn Error>> {
        // 验证输入
        for input in &self.inputs {
            // 检查UTXO是否存在
            if !utxo_set.exists_utxo(&input.txid, input.vout)? {
                return Ok(false);
            }
            
            // 验证输入的有效性（包括签名验证）
            if !utxo_set.verify_input(input)? {
                return Ok(false);
            }
        }
        
        // 验证输入总额是否大于等于输出总额
        let input_value: i64 = self.inputs.iter().map(|input| input.value).sum();
        let output_value: i64 = self.outputs.iter().map(|output| output.value).sum();
        if input_value < output_value {
            return Ok(false);
        }
        
        Ok(true)
    }

    pub fn calculate_fee_rate(&self) -> f64 {
        let input_value: i64 = self.inputs.iter().map(|input| input.value).sum();
        let output_value: i64 = self.outputs.iter().map(|output| output.value).sum();
        let fee = input_value - output_value;
        fee as f64 / 1000.0 // 每KB的费用
    }

    pub fn is_coinbase(&self) -> bool {
        self.inputs.len() == 1 && self.inputs[0].txid == "0" && self.inputs[0].vout == 0
    }
}

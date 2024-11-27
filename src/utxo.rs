use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;

use bs58;
use ring::digest;
use serde::{Deserialize, Serialize};

use crate::blockchain::Blockchain;
use crate::transaction::{Transaction, TxOutput,TxInput};

const UTXO_TREE_FILE: &str = "data/utxo.dat";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UTXOSet {
    utxos: HashMap<String, Vec<(usize, TxOutput)>>,
}

impl UTXOSet {
    pub fn new() -> Self {
        UTXOSet {
            utxos: HashMap::new(),
        }
    }

    pub fn update(&mut self, block_txs: &[Transaction]) -> Result<(), Box<dyn Error>> {
        for tx in block_txs {
            // 删除已花费的输出
            if !tx.is_coinbase() {
                for input in &tx.inputs {
                    if let Some(outputs) = self.utxos.get_mut(&input.txid) {
                        outputs.retain(|(vout, _)| *vout != input.vout);
                        if outputs.is_empty() {
                            self.utxos.remove(&input.txid);
                        }
                    }
                }
            }

            // 添加新的未花费输出
            let mut outputs = Vec::new();
            for (vout, output) in tx.outputs.iter().enumerate() {
                outputs.push((vout, output.clone()));
            }
            self.utxos.insert(tx.id.clone(), outputs);
        }

        Ok(())
    }

    pub fn find_spendable_outputs(
        &self,
        address: &str,
        amount: i64,
    ) -> Result<(i64, HashMap<String, Vec<(usize, TxOutput)>>), Box<dyn Error>> {
        let mut unspent_outputs = HashMap::new();
        let mut accumulated = 0;

        for (txid, outputs) in &self.utxos {
            let mut tx_outputs = Vec::new();
            for (vout, output) in outputs {
                if accumulated >= amount {
                    break;
                }
                
                let address_bytes = bs58::decode(address).into_vec()?;
                if output.pub_key_hash == address_bytes {
                    accumulated += output.value;
                    tx_outputs.push((*vout, output.clone()));
                }
            }
            
            if !tx_outputs.is_empty() {
                unspent_outputs.insert(txid.clone(), tx_outputs);
            }
            
            if accumulated >= amount {
                break;
            }
        }

        Ok((accumulated, unspent_outputs))
    }

    pub fn verify_input(&self, input: &TxInput) -> Result<bool, Box<dyn Error>> {
        if let Some(outputs) = self.utxos.get(&input.txid) {
            if let Some((_, output)) = outputs.iter().find(|(vout, _)| *vout == input.vout) {
                // 验证公钥哈希是否匹配
                let mut hasher = digest::Context::new(&digest::SHA256);
                hasher.update(&input.pubkey);
                let pub_key_hash = hasher.finish();
                
                // 验证签名
                if !input.signature.is_empty() && output.pub_key_hash == pub_key_hash.as_ref() {
                    // TODO: 实现更严格的签名验证
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub fn exists_utxo(&self, txid: &str, vout: usize) -> Result<bool, Box<dyn Error>> {
        if let Some(outputs) = self.utxos.get(txid) {
            return Ok(outputs.iter().any(|(v, _)| *v == vout));
        }
        Ok(false)
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(self)?;
        fs::write(UTXO_TREE_FILE, data)?;
        Ok(())
    }

    pub fn load() -> Result<Self, Box<dyn Error>> {
        if Path::new(UTXO_TREE_FILE).exists() {
            let data = fs::read(UTXO_TREE_FILE)?;
            let utxo_set: UTXOSet = bincode::deserialize(&data)?;
            Ok(utxo_set)
        } else {
            Ok(UTXOSet::new())
        }
    }

    pub fn reindex(&mut self, blockchain: &Blockchain) -> Result<(), Box<dyn Error>> {
        self.utxos.clear();
        
        for block in blockchain.blocks().iter() {
            for tx in &block.transactions {
                // Remove spent outputs
                if !tx.is_coinbase() {
                    for input in &tx.inputs {
                        if let Some(outputs) = self.utxos.get_mut(&input.txid) {
                            outputs.retain(|(vout, _)| *vout != input.vout);
                            if outputs.is_empty() {
                                self.utxos.remove(&input.txid);
                            }
                        }
                    }
                }
                
                // Add new unspent outputs
                let mut outputs = Vec::new();
                for (vout, output) in tx.outputs.iter().enumerate() {
                    outputs.push((vout, output.clone()));
                }
                self.utxos.insert(tx.id.clone(), outputs);
            }
        }
        
        Ok(())
    }

    pub fn get_balance(&self, address: &str) -> Result<i64, Box<dyn Error>> {
        let mut balance = 0;
        
        for (_, outputs) in self.utxos.iter() {
            for (_, output) in outputs {
                if bs58::encode(&output.pub_key_hash).into_string() == address {
                    balance += output.value;
                }
            }
        }
        
        Ok(balance)
    }

    pub fn find_utxo(&self, txid: &str, vout: usize) -> Result<Option<TxOutput>, Box<dyn Error>> {
        if let Some(outputs) = self.utxos.get(txid) {
            for (idx, output) in outputs {
                if *idx == vout {
                    return Ok(Some(output.clone()));
                }
            }
        }
        Ok(None)
    }
}

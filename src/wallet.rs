use secp256k1::{Secp256k1, Message, SecretKey, PublicKey};
use rand::rngs::OsRng;
use sha2::{Sha256, Digest};
use ripemd::Ripemd160;
use bs58;
use std::collections::HashMap;
use std::fs;
use serde::{Serialize, Deserialize};
use once_cell::sync::Lazy;
use std::path::Path;
use hex;

use super::error::{Result, RustBtcError};

const VERSION: u8 = 0x00;
const CHECKSUM_LENGTH: usize = 4;
const WALLET_FILE: &str = "wallet.dat";

static SECP: Lazy<Secp256k1<secp256k1::All>> = Lazy::new(Secp256k1::new);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    secret_key: Vec<u8>,
    public_key: Vec<u8>,
}

impl Wallet {
    pub fn new() -> Result<Wallet> {
        let mut rng = OsRng::default();
        
        // 生成密钥对
        let (secret_key, public_key) = SECP.generate_keypair(&mut rng);
        
        Ok(Wallet {
            secret_key: secret_key.secret_bytes().to_vec(),
            public_key: public_key.serialize().to_vec(),
        })
    }
    
    pub fn get_address(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&self.public_key);
        let pub_hash = hasher.finalize();
        
        let mut hasher = Ripemd160::new();
        hasher.update(pub_hash);
        let pub_ripemd = hasher.finalize();
        
        let mut version_payload = vec![VERSION];
        version_payload.extend(pub_ripemd);
        
        let mut hasher = Sha256::new();
        hasher.update(&version_payload);
        let first_hash = hasher.finalize();
        
        let mut hasher = Sha256::new();
        hasher.update(first_hash);
        let second_hash = hasher.finalize();
        
        let checksum = &second_hash[..CHECKSUM_LENGTH];
        version_payload.extend(checksum);
        
        bs58::encode(version_payload).into_string()
    }
    
    pub fn get_public_key(&self) -> &[u8] {
        &self.public_key
    }

    pub fn get_private_key(&self) -> &[u8] {
        &self.secret_key
    }
    
    pub fn from_public_key(pub_key: &[u8]) -> Result<Wallet> {
        Ok(Wallet {
            secret_key: Vec::new(),
            public_key: pub_key.to_vec(),
        })
    }
    
    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        if self.secret_key.is_empty() {
            println!("Cannot sign with read-only wallet");
            return Err(RustBtcError::ValidationError("无法使用只读钱包签名".to_string()));
        }

        println!("Data to sign: {:?}", hex::encode(data));

        let secret_key = SecretKey::from_slice(&self.secret_key)
            .map_err(|e| {
                println!("Failed to parse secret key: {}", e);
                RustBtcError::InvalidSignature(e.to_string())
            })?;

        let message = Message::from_slice(data)
            .map_err(|e| {
                println!("Failed to create message: {}", e);
                RustBtcError::InvalidSignature(e.to_string())
            })?;

        let signature = SECP.sign_ecdsa(&message, &secret_key);
        let signature_bytes = signature.serialize_compact().to_vec();
        println!("Generated signature: {:?}", hex::encode(&signature_bytes));
        Ok(signature_bytes)
    }
    
    pub fn verify(&self, data: &[u8], signature: &[u8]) -> Result<bool> {
        let public_key = PublicKey::from_slice(&self.public_key)
            .map_err(|e| RustBtcError::InvalidSignature(e.to_string()))?;

        let message = Message::from_slice(data)
            .map_err(|e| RustBtcError::InvalidSignature(e.to_string()))?;

        let sig = secp256k1::ecdsa::Signature::from_compact(signature)
            .map_err(|e| RustBtcError::InvalidSignature(e.to_string()))?;

        Ok(SECP.verify_ecdsa(&message, &sig, &public_key).is_ok())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Wallets {
    wallets: HashMap<String, Wallet>,
}

impl Wallets {
    // 创建或加载钱包集合
    pub fn new() -> Result<Wallets> {
        if Path::new(WALLET_FILE).exists() {
            let data = fs::read(WALLET_FILE)
                .map_err(|e| RustBtcError::Io(e))?;
                
            let wallets: Wallets = bincode::deserialize(&data)
                .map_err(|e: Box<bincode::ErrorKind>| RustBtcError::Serialization(e))?;
                
            Ok(wallets)
        } else {
            Ok(Wallets {
                wallets: HashMap::new(),
            })
        }
    }
    
    // 创建新钱包
    pub fn create_wallet(&mut self) -> Result<String> {
        let wallet = Wallet::new()?;
        let address = wallet.get_address();
        
        self.wallets.insert(address.clone(), wallet);
        self.save()?;
        
        Ok(address)
    }
    
    // 获取所有钱包地址
    pub fn get_addresses(&self) -> Vec<String> {
        self.wallets.keys().cloned().collect()
    }
    
    // 获取指定地址的钱包
    pub fn get_wallet(&self, address: &str) -> Option<&Wallet> {
        self.wallets.get(address)
    }
    
    // 保存钱包到文件
    pub fn save(&self) -> Result<()> {
        let data = bincode::serialize(&self)
            .map_err(|e: Box<bincode::ErrorKind>| RustBtcError::Serialization(e))?;

        fs::write(WALLET_FILE, data)
            .map_err(|e| RustBtcError::Io(e))?;
            
        Ok(())
    }
}

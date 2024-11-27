use std::error::Error;
use secp256k1::{Secp256k1, Message, SecretKey, PublicKey};
use rand::rngs::OsRng;
use sha2::{Sha256, Digest};
use ripemd::Ripemd160;
use bs58;
use std::collections::HashMap;
use std::fs;
use serde::{Serialize, Deserialize};
use once_cell::sync::Lazy;

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
    pub fn new() -> Result<Wallet, Box<dyn Error>> {
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
        
        let mut address = version_payload;
        address.extend_from_slice(&second_hash[..CHECKSUM_LENGTH]);
        
        bs58::encode(address).into_string()
    }
    
    pub fn get_public_key(&self) -> &[u8] {
        &self.public_key
    }
    
    pub fn from_public_key(pub_key: &[u8]) -> Result<Wallet, Box<dyn Error>> {
        Ok(Wallet {
            secret_key: Vec::new(),
            public_key: pub_key.to_vec(),
        })
    }
    
    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        // 创建消息哈希
        let hash = Sha256::digest(data);
        
        // 创建消息
        let message = Message::from_slice(&hash)?;
        
        // 从字节创建私钥
        let secret_key = SecretKey::from_slice(&self.secret_key)?;
        
        // 签名消息
        let signature = SECP.sign_ecdsa(&message, &secret_key);
        
        Ok(signature.serialize_compact().to_vec())
    }
    
    pub fn verify(&self, data: &[u8], signature: &[u8]) -> Result<bool, Box<dyn Error>> {
        // 创建消息哈希
        let hash = Sha256::digest(data);
        
        // 创建消息
        let message = Message::from_slice(&hash)?;
        
        // 从字节创建公钥和签名
        let public_key = PublicKey::from_slice(&self.public_key)?;
        let signature = secp256k1::ecdsa::Signature::from_compact(signature)?;
        
        // 验证签名
        Ok(SECP.verify_ecdsa(&message, &signature, &public_key).is_ok())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Wallets {
    wallets: HashMap<String, Wallet>,
}

impl Wallets {
    // 创建或加载钱包集合
    pub fn new() -> Result<Wallets, Box<dyn Error>> {
        if let Ok(data) = fs::read(WALLET_FILE) {
            let wallets: Wallets = bincode::deserialize(&data)?;
            Ok(wallets)
        } else {
            Ok(Wallets {
                wallets: HashMap::new(),
            })
        }
    }
    
    // 创建新钱包
    pub fn create_wallet(&mut self) -> Result<String, Box<dyn Error>> {
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
    fn save(&self) -> Result<(), Box<dyn Error>> {
        let data = bincode::serialize(self)?;
        fs::write(WALLET_FILE, data)?;
        Ok(())
    }
}

use crate::db::{Database, DbTable};
use crate::error::Result;
use crate::models::{Block, WalletData, UTXOEntry};

pub struct Storage {
    db: Database,
}

impl Storage {
    pub fn new(path: &str) -> Result<Self> {
        let db = Database::new(path)?;
        Ok(Storage { db })
    }

    // Block storage operations
    pub fn save_block(&self, height: u64, block: &Block) -> Result<()> {
        let key = height.to_be_bytes();
        let value = block.serialize()?;
        self.db.put(DbTable::Block, &key, &value)
    }

    pub fn get_block(&self, height: u64) -> Result<Option<Block>> {
        let key = height.to_be_bytes();
        match self.db.view(DbTable::Block, &key)? {
            Some(data) => Ok(Some(Block::deserialize(&data)?)),
            None => Ok(None),
        }
    }

    pub fn delete_block(&self, height: u64) -> Result<()> {
        let key = height.to_be_bytes();
        self.db.delete(DbTable::Block, &key)
    }

    // Wallet storage operations
    pub fn save_wallet(&self, address: &str, wallet: &WalletData) -> Result<()> {
        let value = wallet.serialize()?;
        self.db.put(DbTable::Address, address.as_bytes(), &value)
    }

    pub fn get_wallet(&self, address: &str) -> Result<Option<WalletData>> {
        match self.db.view(DbTable::Address, address.as_bytes())? {
            Some(data) => Ok(Some(WalletData::deserialize(&data)?)),
            None => Ok(None),
        }
    }

    pub fn delete_wallet(&self, address: &str) -> Result<()> {
        self.db.delete(DbTable::Address, address.as_bytes())
    }

    // UTXO storage operations
    pub fn save_utxo(&self, txid: &str, vout: u32, utxo: &UTXOEntry) -> Result<()> {
        let key = format!("{}:{}", txid, vout);
        let value = utxo.serialize()?;
        self.db.put(DbTable::UTXO, key.as_bytes(), &value)
    }

    pub fn get_utxo(&self, txid: &str, vout: u32) -> Result<Option<UTXOEntry>> {
        let key = format!("{}:{}", txid, vout);
        match self.db.view(DbTable::UTXO, key.as_bytes())? {
            Some(data) => Ok(Some(UTXOEntry::deserialize(&data)?)),
            None => Ok(None),
        }
    }

    pub fn delete_utxo(&self, txid: &str, vout: u32) -> Result<()> {
        let key = format!("{}:{}", txid, vout);
        self.db.delete(DbTable::UTXO, key.as_bytes())
    }

    // Iteration methods for each bucket
    pub fn iter_blocks(&self) -> Result<impl Iterator<Item = (u64, Block)>> {
        let iter = self.db.iterate(DbTable::Block)?;
        Ok(iter.filter_map(|(key, value)| {
            if key.len() == 8 {
                let height = u64::from_be_bytes(key.as_ref().try_into().ok()?);
                let block = Block::deserialize(&value).ok()?;
                Some((height, block))
            } else {
                None
            }
        }))
    }

    pub fn iter_wallets(&self) -> Result<impl Iterator<Item = (String, WalletData)>> {
        let iter = self.db.iterate(DbTable::Address)?;
        Ok(iter.filter_map(|(key, value)| {
            let address = String::from_utf8(key.to_vec()).ok()?;
            let wallet = WalletData::deserialize(&value).ok()?;
            Some((address, wallet))
        }))
    }

    pub fn iter_utxos(&self) -> Result<impl Iterator<Item = (String, UTXOEntry)>> {
        let iter = self.db.iterate(DbTable::UTXO)?;
        Ok(iter.filter_map(|(key, value)| {
            let key_str = String::from_utf8(key.to_vec()).ok()?;
            let utxo = UTXOEntry::deserialize(&value).ok()?;
            Some((key_str, utxo))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_block_storage() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let storage = Storage::new(temp_dir.path().to_str().unwrap())?;

        let block = Block {
            version: 1,
            prev_block_hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            merkle_root: "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b".to_string(),
            timestamp: 1231006505,
            bits: 0x1d00ffff,
            nonce: 2083236893,
            transactions: vec![],
        };

        // Test save and retrieve
        storage.save_block(0, &block)?;
        let retrieved = storage.get_block(0)?.unwrap();
        assert_eq!(retrieved.version, block.version);
        assert_eq!(retrieved.prev_block_hash, block.prev_block_hash);

        // Test delete
        storage.delete_block(0)?;
        assert!(storage.get_block(0)?.is_none());

        Ok(())
    }

    // Add more tests for wallet and UTXO storage...
}

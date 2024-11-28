use std::path::Path;
use sled::IVec;
use crate::error::{Result, RustBtcError};

const BLOCK_BUCKET: &str = "blocks";
const ADDR_BUCKET: &str = "addresses";
const UTXO_BUCKET: &str = "utxos";

#[derive(Debug, Clone, Copy)]
pub enum DbTable {
    Block,
    Address,
    UTXO,
}

impl DbTable {
    fn as_str(&self) -> &'static str {
        match self {
            DbTable::Block => BLOCK_BUCKET,
            DbTable::Address => ADDR_BUCKET,
            DbTable::UTXO => UTXO_BUCKET,
        }
    }
}

pub struct Database {
    path: String,
}

impl Database {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_str = path.as_ref()
            .to_str()
            .ok_or_else(|| RustBtcError::Database("Invalid path".to_string()))?
            .to_string();
            
        // Test database connection
        sled::open(&path_str)
            .map_err(|e| RustBtcError::Database(e.to_string()))?;
            
        Ok(Database {
            path: path_str,
        })
    }

    fn get_table(&self, table: DbTable) -> Result<sled::Tree> {
        let db = sled::open(&self.path)
            .map_err(|e| RustBtcError::Database(e.to_string()))?;
            
        db.open_tree(table.as_str())
            .map_err(|e| RustBtcError::Database(e.to_string()))
    }

    pub fn put(&self, table: DbTable, key: &[u8], value: &[u8]) -> Result<()> {
        let tree = self.get_table(table)?;
        tree.insert(key, value)
            .map_err(|e| RustBtcError::Database(e.to_string()))?;
        tree.flush()
            .map_err(|e| RustBtcError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn view(&self, table: DbTable, key: &[u8]) -> Result<Option<IVec>> {
        let tree = self.get_table(table)?;
        tree.get(key)
            .map_err(|e| RustBtcError::Database(e.to_string()))
    }

    pub fn delete(&self, table: DbTable, key: &[u8]) -> Result<()> {
        let tree = self.get_table(table)?;
        tree.remove(key)
            .map_err(|e| RustBtcError::Database(e.to_string()))?;
        tree.flush()
            .map_err(|e| RustBtcError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn iterate(&self, table: DbTable) -> Result<impl Iterator<Item = (IVec, IVec)>> {
        let tree = self.get_table(table)?;
        Ok(tree.iter().filter_map(|r| match r {
            Ok(item) => Some(item),
            Err(_) => None
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_database_operations() -> Result<()> {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db");
        
        let db = Database::new(&db_path)?;
        
        // Test put and view
        let key = b"test_key";
        let value = b"test_value";
        
        db.put(DbTable::Block, key, value)?;
        let retrieved = db.view(DbTable::Block, key)?;
        assert_eq!(retrieved.as_ref().map(|v| v.as_ref()), Some(value.as_ref()));
        
        // Test delete
        db.delete(DbTable::Block, key)?;
        let retrieved = db.view(DbTable::Block, key)?;
        assert_eq!(retrieved, None);
        
        Ok(())
    }
}

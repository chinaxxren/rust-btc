use sha2::{Digest, Sha256};

pub struct ProofOfWork {
    target: String,
}

impl ProofOfWork {
    pub fn new(difficulty: u32) -> Self {
        let target = "0".repeat(difficulty as usize);
        ProofOfWork { target }
    }

    pub fn run(&self, data: &[u8]) -> (u64, String) {
        let mut nonce = 0u64;
        loop {
            let hash = self.calculate_hash(data, nonce);
            if hash.starts_with(&self.target) {
                return (nonce, hash);
            }
            nonce += 1;
        }
    }

    pub fn validate(&self, data: &[u8], nonce: u64) -> bool {
        let hash = self.calculate_hash(data, nonce);
        hash.starts_with(&self.target)
    }

    fn calculate_hash(&self, data: &[u8], nonce: u64) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.update(nonce.to_string());
        let result = hasher.finalize();
        hex::encode(result)
    }
}

use sha2::{Sha256, Digest};
use std::fmt;

#[derive(Debug, Clone)]
pub struct MerkleNode {
    pub hash: Vec<u8>,
    pub left: Option<Box<MerkleNode>>,
    pub right: Option<Box<MerkleNode>>,
}

impl MerkleNode {
    // 创建新的叶子节点
    pub fn new_leaf(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize().to_vec();

        MerkleNode {
            hash,
            left: None,
            right: None,
        }
    }

    // 创建新的中间节点
    pub fn new_parent(left: MerkleNode, right: MerkleNode) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(&left.hash);
        hasher.update(&right.hash);
        let hash = hasher.finalize().to_vec();

        MerkleNode {
            hash,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        }
    }

    // 验证节点
    pub fn verify(&self, data: &[u8], proof: &[Vec<u8>], index: usize) -> bool {
        let mut current_hash = {
            let mut hasher = Sha256::new();
            hasher.update(data);
            hasher.finalize().to_vec()
        };

        let mut current_index = index;

        for sibling in proof {
            let mut hasher = Sha256::new();
            
            if current_index % 2 == 0 {
                hasher.update(&current_hash);
                hasher.update(sibling);
            } else {
                hasher.update(sibling);
                hasher.update(&current_hash);
            }
            
            current_hash = hasher.finalize().to_vec();
            current_index /= 2;
        }

        current_hash == self.hash
    }
}

impl fmt::Display for MerkleNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Hash: {:?}", self.hash)
    }
}

#[derive(Debug)]
pub struct MerkleTree {
    pub root: Option<MerkleNode>,
    leaves: Vec<MerkleNode>,
}

impl MerkleTree {
    // 从数据切片构建Merkle树
    pub fn new(data: &[Vec<u8>]) -> Self {
        if data.is_empty() {
            return MerkleTree {
                root: None,
                leaves: Vec::new(),
            };
        }

        // 创建叶子节点
        let mut leaves: Vec<MerkleNode> = data.iter()
            .map(|d| MerkleNode::new_leaf(d))
            .collect();

        // 如果叶子节点数量为奇数，复制最后一个节点
        if leaves.len() % 2 == 1 {
            leaves.push(leaves.last().unwrap().clone());
        }

        let mut nodes = leaves.clone();
        let mut layer = Vec::new();

        // 构建树的各层
        while nodes.len() > 1 {
            layer.clear();
            
            for chunk in nodes.chunks(2) {
                if chunk.len() == 2 {
                    layer.push(MerkleNode::new_parent(
                        chunk[0].clone(),
                        chunk[1].clone(),
                    ));
                } else {
                    layer.push(chunk[0].clone());
                }
            }
            
            nodes = layer.clone();
        }

        MerkleTree {
            root: Some(nodes[0].clone()),
            leaves,
        }
    }

    // 获取Merkle根哈希
    pub fn root_hash(&self) -> Option<Vec<u8>> {
        self.root.as_ref().map(|node| node.hash.clone())
    }

    // 生成Merkle证明
    pub fn get_proof(&self, index: usize) -> Option<Vec<Vec<u8>>> {
        if index >= self.leaves.len() {
            return None;
        }

        let mut proof = Vec::new();
        let mut current_index = index;
        let mut nodes = self.leaves.clone();

        while nodes.len() > 1 {
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            if sibling_index < nodes.len() {
                proof.push(nodes[sibling_index].hash.clone());
            }

            let mut next_level = Vec::new();
            for chunk in nodes.chunks(2) {
                if chunk.len() == 2 {
                    next_level.push(MerkleNode::new_parent(
                        chunk[0].clone(),
                        chunk[1].clone(),
                    ));
                } else {
                    next_level.push(chunk[0].clone());
                }
            }

            nodes = next_level;
            current_index /= 2;
        }

        Some(proof)
    }

    // 验证Merkle证明
    pub fn verify_proof(&self, data: &[u8], proof: &[Vec<u8>], index: usize) -> bool {
        if let Some(root) = &self.root {
            root.verify(data, proof, index)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_tree() {
        let data = vec![
            b"Transaction 1".to_vec(),
            b"Transaction 2".to_vec(),
            b"Transaction 3".to_vec(),
            b"Transaction 4".to_vec(),
        ];

        let tree = MerkleTree::new(&data);
        assert!(tree.root.is_some());

        // 验证Merkle证明
        if let Some(proof) = tree.get_proof(0) {
            assert!(tree.verify_proof(&data[0], &proof, 0));
        }
    }

    #[test]
    fn test_empty_tree() {
        let data: Vec<Vec<u8>> = vec![];
        let tree = MerkleTree::new(&data);
        assert!(tree.root.is_none());
    }

    #[test]
    fn test_single_node() {
        let data = vec![b"Single Transaction".to_vec()];
        let tree = MerkleTree::new(&data);
        assert!(tree.root.is_some());
    }
}

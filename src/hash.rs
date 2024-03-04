use sha2::{Digest, Sha256};

pub struct Hash {}

impl Hash {
    pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    pub fn sha256(data: &[u8]) -> [u8; 32] {
        Hash::sha256_bytes(data).into()
    }

    pub fn sha256d(data: &[u8]) -> [u8; 32] {
        Hash::sha256_bytes(&Hash::sha256_bytes(data))
    }
}

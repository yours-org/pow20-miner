use sha2::{Digest, Sha256};

pub struct Hash {}

impl Hash {
    pub fn sha256_bytes(data: Vec<u8>) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        result.to_vec()
    }

    pub fn sha256(data: String) -> String {
        hex::encode(Hash::sha256_bytes(data.as_bytes().to_vec()))
    }

    pub fn sha256d(data: Vec<u8>) -> String {
        hex::encode(Hash::sha256_bytes(Hash::sha256_bytes(data)))
    }
}

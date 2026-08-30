#![deny(unused_must_use)]

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CanonicalEncodingError {
    source: Arc<serde_json::Error>,
}

impl CanonicalEncodingError {
    fn from_serde(source: serde_json::Error) -> Self {
        Self {
            source: Arc::new(source),
        }
    }
}

impl From<serde_json::Error> for CanonicalEncodingError {
    fn from(source: serde_json::Error) -> Self {
        Self::from_serde(source)
    }
}

impl PartialEq for CanonicalEncodingError {
    fn eq(&self, other: &Self) -> bool {
        self.source.classify() == other.source.classify()
            && self.source.to_string() == other.source.to_string()
    }
}

impl Eq for CanonicalEncodingError {}

impl std::fmt::Display for CanonicalEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.source.fmt(formatter)
    }
}

impl std::error::Error for CanonicalEncodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

pub fn hash_canonical<T: Serialize + ?Sized>(
    domain: &str,
    value: &T,
) -> Result<[u8; 32], CanonicalEncodingError> {
    let encoded = serde_json::to_vec(value).map_err(CanonicalEncodingError::from_serde)?;
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len() + size_of::<u64>());
    bytes.extend_from_slice(&(domain.len() as u64).to_be_bytes());
    bytes.extend_from_slice(domain.as_bytes());
    bytes.extend_from_slice(&encoded);
    Ok(Sha256::digest(&bytes).into())
}

#[cfg(test)]
mod tests {
    use super::hash_canonical;

    #[test]
    fn canonical_hash_preserves_the_existing_encoding_contract() {
        let digest = hash_canonical("yssbi.test.v1", "abc").expect("string must serialize");
        let hex = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        assert_eq!(
            hex,
            "7d026fd67fd8690d6c22649b2e07922d5a6ca3402e45ac052d0d70aba936c345"
        );
    }

    #[test]
    fn domains_separate_equal_payloads() {
        let left = hash_canonical("yssbi.left.v1", &42).expect("integer must serialize");
        let right = hash_canonical("yssbi.right.v1", &42).expect("integer must serialize");

        assert_ne!(left, right);
    }
}

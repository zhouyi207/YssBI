use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use yss_graph_protocol::NodeProtocol;

macro_rules! fingerprint {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name([u8; 32]);
        impl $name {
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }
            pub fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
            pub fn to_hex(&self) -> String {
                self.0.iter().map(|b| format!("{b:02x}")).collect()
            }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&self.to_hex())
                    .finish()
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.to_hex())
            }
        }
    };
}

fingerprint!(ProtocolFingerprint);
fingerprint!(RegistryFingerprint);

pub(crate) fn protocol_fingerprint(
    protocol: &NodeProtocol,
) -> Result<ProtocolFingerprint, CanonicalEncodingError> {
    hash_canonical(
        "yssbi.node-protocol.v1",
        &canonical_semantic_protocol(protocol),
    )
    .map(ProtocolFingerprint)
}

pub(crate) fn canonical_semantic_protocol(protocol: &NodeProtocol) -> serde_json::Value {
    serde_json::json!({
        "execution": &protocol.execution,
        "interface": &protocol.interface,
        "managedRole": &protocol.managed_role,
        "parameters": &protocol.parameters,
        "scope": &protocol.scope,
        "typeId": &protocol.type_id,
    })
}

pub(crate) fn registry_fingerprint<T: Serialize>(
    value: &T,
) -> Result<RegistryFingerprint, CanonicalEncodingError> {
    hash_canonical("yssbi.node-registry.v1", value).map(RegistryFingerprint)
}

#[derive(Debug, Clone)]
pub struct CanonicalEncodingError {
    source: Arc<serde_json::Error>,
}

impl CanonicalEncodingError {
    pub(crate) fn from_serde(source: serde_json::Error) -> Self {
        Self {
            source: Arc::new(source),
        }
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

pub(crate) fn hash_canonical<T: Serialize>(
    domain: &str,
    value: &T,
) -> Result<[u8; 32], CanonicalEncodingError> {
    let encoded = serde_json::to_vec(value).map_err(CanonicalEncodingError::from_serde)?;
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len() + 9);
    bytes.extend_from_slice(&(domain.len() as u64).to_be_bytes());
    bytes.extend_from_slice(domain.as_bytes());
    bytes.extend_from_slice(&encoded);
    Ok(sha256(&bytes))
}

// Small, dependency-free SHA-256 keeps fingerprints portable without changing Cargo.
fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in w[..16].iter_mut().enumerate() {
            let offset = i * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    let mut out = [0; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::sha256;
    #[test]
    fn sha256_matches_standard_vector() {
        let hex: String = sha256(b"abc").iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

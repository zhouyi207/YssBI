#![deny(unused_must_use)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
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
    let serialized = serde_json::to_string(value).map_err(CanonicalEncodingError::from_serde)?;
    let mut encoded = Vec::with_capacity(serialized.len());
    encode_canonical(&serialized, &mut encoded).map_err(CanonicalEncodingError::from_serde)?;
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_be_bytes());
    digest.update(domain.as_bytes());
    digest.update(&encoded);
    Ok(digest.finalize().into())
}

#[derive(Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct ObjectKey<'a>(#[serde(borrow)] Cow<'a, str>);

fn encode_canonical(raw: &str, encoded: &mut Vec<u8>) -> Result<(), serde_json::Error> {
    // Borrow encoded values instead of converting numbers through Value, which
    // cannot represent every integer accepted by Serde JSON's serializer.
    let raw = raw.trim();
    match raw.as_bytes()[0] {
        b'{' => {
            let members: std::collections::BTreeMap<ObjectKey<'_>, &serde_json::value::RawValue> =
                serde_json::from_str(raw)?;
            encoded.push(b'{');
            for (index, (key, value)) in members.into_iter().enumerate() {
                if index != 0 {
                    encoded.push(b',');
                }
                serde_json::to_writer(&mut *encoded, &key.0)?;
                encoded.push(b':');
                encode_canonical(value.get(), encoded)?;
            }
            encoded.push(b'}');
        }
        b'[' => {
            let values: Vec<&serde_json::value::RawValue> = serde_json::from_str(raw)?;
            encoded.push(b'[');
            for (index, value) in values.into_iter().enumerate() {
                if index != 0 {
                    encoded.push(b',');
                }
                encode_canonical(value.get(), encoded)?;
            }
            encoded.push(b']');
        }
        _ => encoded.extend_from_slice(raw.as_bytes()),
    }
    Ok(())
}

/// SHA-256 of an artifact's exact bytes, independent of JSON/domain encoding.
pub fn content_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Hash file-sized artifacts without retaining their contents in memory.
pub fn content_sha256_reader(mut reader: impl std::io::Read) -> std::io::Result<String> {
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            return Ok(format!("{:x}", digest.finalize()));
        }
        digest.update(&buffer[..count]);
    }
}

#[cfg(test)]
mod tests {
    use super::hash_canonical;
    use serde::Serialize;

    #[test]
    fn object_member_order_does_not_change_struct_or_nested_json_identity() {
        #[derive(Serialize)]
        struct Fields {
            z: Vec<serde_json::Value>,
            a: u32,
        }
        let nested = |reverse| {
            let mut object = serde_json::Map::new();
            for (key, value) in if reverse {
                [("z", 2), ("a", 1)]
            } else {
                [("a", 1), ("z", 2)]
            } {
                object.insert(key.into(), serde_json::json!(value));
            }
            serde_json::Value::Object(object)
        };
        let fields = Fields {
            z: vec![nested(true)],
            a: 7,
        };
        let equivalent = serde_json::json!({"a": 7, "z": [nested(false)]});
        assert_eq!(
            hash_canonical("test.object", &fields).unwrap(),
            hash_canonical("test.object", &equivalent).unwrap()
        );
    }

    #[test]
    fn array_order_remains_part_of_canonical_identity() {
        let first = serde_json::json!({"items": [1, 2]});
        let reversed = serde_json::json!({"items": [2, 1]});
        assert_ne!(
            hash_canonical("test.array", &first).unwrap(),
            hash_canonical("test.array", &reversed).unwrap()
        );
    }

    #[test]
    fn wide_integer_encoding_keeps_every_digit() {
        let digest = hash_canonical("test.wide", &u128::MAX).unwrap();
        let actual = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            actual,
            "f03bc9b1e8f353b88604b5cb77e515180361520a3f972868767322e34eb0a2b9"
        );
    }

    #[test]
    fn escaped_and_unicode_object_keys_keep_sorted_exact_values() {
        #[derive(Serialize)]
        struct EscapedKeys {
            #[serde(rename = "z\"")]
            quote: u128,
            #[serde(rename = "a\\")]
            backslash: u128,
            #[serde(rename = "m\n")]
            newline: u128,
            #[serde(rename = "é")]
            unicode: u128,
        }
        let value = EscapedKeys {
            quote: u128::MAX,
            backslash: 1,
            newline: 2,
            unicode: 3,
        };
        let sorted = std::collections::BTreeMap::from([
            ("z\"", u128::MAX),
            ("a\\", 1),
            ("m\n", 2),
            ("é", 3),
        ]);
        use sha2::Digest;
        let domain = "test.escaped-keys";
        let mut expected = sha2::Sha256::new();
        expected.update((domain.len() as u64).to_be_bytes());
        expected.update(domain.as_bytes());
        expected.update(serde_json::to_vec(&sorted).unwrap());
        assert_eq!(
            hash_canonical(domain, &value).unwrap(),
            <[u8; 32]>::from(expected.finalize())
        );
    }

    #[test]
    fn streaming_hash_covers_every_buffer_and_propagates_io_failures() {
        let bytes = vec![0x5a; 150_001];
        assert_eq!(
            super::content_sha256_reader(bytes.as_slice()).unwrap(),
            super::content_sha256(&bytes)
        );
        struct FailedRead;
        impl std::io::Read for FailedRead {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("failed read"))
            }
        }
        assert!(super::content_sha256_reader(FailedRead).is_err());
    }

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

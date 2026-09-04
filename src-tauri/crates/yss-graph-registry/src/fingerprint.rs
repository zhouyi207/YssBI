use serde::{Deserialize, Serialize};
use std::fmt;
use yss_canonical_hash::{CanonicalEncodingError, hash_canonical};
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
        "typing": &protocol.typing,
        "typeId": &protocol.type_id,
    })
}

pub(crate) fn registry_fingerprint<T: Serialize>(
    value: &T,
) -> Result<RegistryFingerprint, CanonicalEncodingError> {
    hash_canonical("yssbi.node-registry.v1", value).map(RegistryFingerprint)
}

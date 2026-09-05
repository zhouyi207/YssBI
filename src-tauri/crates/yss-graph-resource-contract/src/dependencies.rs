use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub enum GraphDependencyKey {
    Function(Box<str>),
    FunctionBody(Box<str>),
    Variable(Box<str>),
    Database(Box<str>),
}

impl GraphDependencyKey {
    pub fn identity(&self) -> &str {
        match self {
            Self::Function(identity)
            | Self::FunctionBody(identity)
            | Self::Variable(identity)
            | Self::Database(identity) => identity,
        }
    }

    pub fn storage_key(&self) -> String {
        let digest = yss_canonical_hash::hash_canonical("yssbi.graph-dependency-key.v1", self)
            .expect("resource identity is serializable");
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GraphDependencyManifest(pub(crate) BTreeMap<GraphDependencyKey, Option<[u8; 32]>>);

impl GraphDependencyManifest {
    pub fn entries(&self) -> &BTreeMap<GraphDependencyKey, Option<[u8; 32]>> {
        &self.0
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        yss_canonical_hash::hash_canonical(
            "yssbi.graph-dependencies.v1",
            &self.0.iter().collect::<Vec<_>>(),
        )
        .expect("dependency observations are serializable")
    }
}

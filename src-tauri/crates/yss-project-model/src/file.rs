//! Shared mechanics for independently typed resource file bodies.
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fmt::Debug,
    hash::{Hash, Hasher},
    marker::PhantomData,
};
use yss_project_identity::ResourceRevision;
use yss_resource_naming::ResourceName;

pub const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
pub trait FileContent: Clone + Debug + PartialEq + Serialize + DeserializeOwned {
    type Edit: Clone + Debug + DeserializeOwned;
    const KIND: &'static str;
    const DIRECTORY: &'static str;
    const EXTENSION: &'static str;
    fn new(title: &str, next_id: &mut dyn FnMut() -> String) -> Self;
    fn decode(bytes: &[u8]) -> Result<Self, String>;
    fn encode(&self) -> Result<Vec<u8>, String>;
    fn apply(&mut self, edit: Self::Edit) -> Result<(), String>;
    fn duplicate(&self, next_id: &mut dyn FnMut() -> String) -> Self;
    fn fingerprint(&self) -> Result<String, String> {
        self.encode()
            .map(|bytes| yss_canonical_hash::content_sha256(&bytes))
    }
}
pub fn bounded(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if bytes.len() > MAX_FILE_BYTES {
        Err("file size limit exceeded".into())
    } else {
        Ok(bytes)
    }
}
#[derive(Debug, Clone)]
pub struct FilePath<T: FileContent>(String, PhantomData<fn() -> T>);
impl<T: FileContent> FilePath<T> {
    pub fn parse(value: &str) -> Result<Self, String> {
        let name = value
            .strip_prefix(&format!("{}/", T::DIRECTORY))
            .and_then(|name| name.strip_suffix(&format!(".{}", T::EXTENSION)))
            .ok_or("invalid resource file path")?;
        ResourceName::parse(name).map_err(|e| e.to_string())?;
        Ok(Self(value.into(), PhantomData))
    }
    pub fn from_name(name: &ResourceName) -> Self {
        Self(
            format!("{}/{}.{}", T::DIRECTORY, name.as_str(), T::EXTENSION),
            PhantomData,
        )
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn name(&self) -> &str {
        &self.0[T::DIRECTORY.len() + 1..self.0.len() - T::EXTENSION.len() - 1]
    }
}
impl<T: FileContent> PartialEq for FilePath<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T: FileContent> Eq for FilePath<T> {}
impl<T: FileContent> Hash for FilePath<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
impl<T: FileContent> Serialize for FilePath<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}
impl<'de, T: FileContent> Deserialize<'de> for FilePath<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileVersion {
    pub session_id: String,
    pub revision: ResourceRevision,
}
#[derive(Debug, Clone)]
pub struct FileState<T: FileContent> {
    pub document: T,
    pub version: FileVersion,
    pub saved_hash: String,
}
impl<T: FileContent> FileState<T> {
    pub fn new(document: T, session_id: String) -> Result<Self, String> {
        Ok(Self {
            saved_hash: document.fingerprint()?,
            document,
            version: FileVersion {
                session_id,
                revision: ResourceRevision::INITIAL,
            },
        })
    }
    pub fn dirty(&self) -> Result<bool, String> {
        Ok(self.document.fingerprint()? != self.saved_hash)
    }
}

#[derive(Debug, Clone)]
pub enum FilePatch<T: FileContent> {
    Put {
        path: FilePath<T>,
        document: FileState<T>,
    },
    Remove {
        path: FilePath<T>,
    },
    Move {
        from: FilePath<T>,
        to: FilePath<T>,
        document: FileState<T>,
    },
}

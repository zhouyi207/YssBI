use super::declaration::DatabaseDecl;
use super::identity::{DatabaseDeclarationFingerprint, DatabaseId};
use super::observation::DatabaseDeclarationObservationSet;
use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::Arc;

/// Opaque identity for one database runtime session.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DatabaseSessionIdentity(Box<str>);

impl DatabaseSessionIdentity {
    pub fn from_existing(value: Box<str>) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Contract-owned input for opening a database runtime session.
#[derive(Clone, Debug)]
pub struct DatabaseSessionOpenRequest {
    identity: DatabaseSessionIdentity,
    generation: NonZeroU64,
    root: Option<PathBuf>,
    declarations: Arc<[DatabaseDecl]>,
    observations: DatabaseDeclarationObservationSet,
}

/// Validated, owned values consumed by a database runtime adapter.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct DatabaseSessionOpenRequestParts {
    pub identity: DatabaseSessionIdentity,
    pub generation: NonZeroU64,
    pub root: Option<PathBuf>,
    pub declarations: Arc<[DatabaseDecl]>,
    pub observations: DatabaseDeclarationObservationSet,
}

impl DatabaseSessionOpenRequest {
    pub fn new(
        identity: DatabaseSessionIdentity,
        generation: NonZeroU64,
        root: Option<PathBuf>,
        declarations: Arc<[DatabaseDecl]>,
        observations: DatabaseDeclarationObservationSet,
    ) -> Self {
        Self {
            identity,
            generation,
            root,
            declarations,
            observations,
        }
    }

    pub fn validate(&self) -> Result<(), DatabaseSessionOpenRequestError> {
        if self.identity.as_str().is_empty() {
            return Err(DatabaseSessionOpenRequestError::EmptyIdentity);
        }

        let mut declaration_ids = BTreeSet::new();
        for declaration in self.declarations.iter() {
            if !declaration_ids.insert(declaration.id.clone()) {
                return Err(DatabaseSessionOpenRequestError::DuplicateDeclarationId(
                    declaration.id.clone(),
                ));
            }

            let Some(observation) = self.observations.get(&declaration.id) else {
                return Err(DatabaseSessionOpenRequestError::MissingObservation(
                    declaration.id.clone(),
                ));
            };
            let fingerprint = DatabaseDeclarationFingerprint::from_decl(declaration);
            if observation.fingerprint() != &fingerprint {
                return Err(DatabaseSessionOpenRequestError::FingerprintMismatch(
                    declaration.id.clone(),
                ));
            }
        }

        if let Some((id, _)) = self
            .observations
            .iter()
            .find(|(id, _)| !declaration_ids.contains(*id))
        {
            return Err(DatabaseSessionOpenRequestError::UnexpectedObservation(
                id.clone(),
            ));
        }
        Ok(())
    }

    pub fn into_validated_parts(
        self,
    ) -> Result<DatabaseSessionOpenRequestParts, DatabaseSessionOpenRequestError> {
        self.validate()?;
        Ok(DatabaseSessionOpenRequestParts {
            identity: self.identity,
            generation: self.generation,
            root: self.root,
            declarations: self.declarations,
            observations: self.observations,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DatabaseSessionOpenRequestError {
    #[error("database session identity is empty")]
    EmptyIdentity,
    #[error("database declaration id is duplicated")]
    DuplicateDeclarationId(DatabaseId),
    #[error("database declaration observation is missing")]
    MissingObservation(DatabaseId),
    #[error("database declaration observation has no declaration")]
    UnexpectedObservation(DatabaseId),
    #[error("database declaration observation fingerprint does not match")]
    FingerprintMismatch(DatabaseId),
}

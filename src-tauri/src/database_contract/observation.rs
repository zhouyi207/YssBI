use super::identity::{DatabaseDeclarationFingerprint, DatabaseDeclarationRevision, DatabaseId};
use std::collections::BTreeMap;

/// Revision and declaration fingerprint observed when a database session opens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseDeclarationObservation {
    revision: DatabaseDeclarationRevision,
    fingerprint: DatabaseDeclarationFingerprint,
}

impl DatabaseDeclarationObservation {
    pub fn new(
        revision: DatabaseDeclarationRevision,
        fingerprint: DatabaseDeclarationFingerprint,
    ) -> Self {
        Self {
            revision,
            fingerprint,
        }
    }

    pub fn revision(&self) -> DatabaseDeclarationRevision {
        self.revision
    }

    pub fn fingerprint(&self) -> &DatabaseDeclarationFingerprint {
        &self.fingerprint
    }
}

/// A duplicate-free, complete-by-construction map of declaration observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseDeclarationObservationSet(BTreeMap<DatabaseId, DatabaseDeclarationObservation>);

impl DatabaseDeclarationObservationSet {
    pub fn try_from_iter<I>(entries: I) -> Result<Self, DatabaseDeclarationObservationSetError>
    where
        I: IntoIterator<Item = (DatabaseId, DatabaseDeclarationObservation)>,
    {
        let mut observations = BTreeMap::new();
        for (id, observation) in entries {
            if observations.insert(id.clone(), observation).is_some() {
                return Err(DatabaseDeclarationObservationSetError::DuplicateId(id));
            }
        }
        Ok(Self(observations))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&DatabaseId, &DatabaseDeclarationObservation)> {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a DatabaseDeclarationObservationSet {
    type Item = (&'a DatabaseId, &'a DatabaseDeclarationObservation);
    type IntoIter =
        std::collections::btree_map::Iter<'a, DatabaseId, DatabaseDeclarationObservation>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DatabaseDeclarationObservationSetError {
    #[error("database declaration observation id is duplicated")]
    DuplicateId(DatabaseId),
}

//! Selected profile reads over one admitted immutable dataset snapshot.
use crate::error::{DatabaseError, DatabaseOperation};
use crate::runtime::DatabaseRuntimeSession;
use yss_database_contract::DatabaseId;
use yss_relational_contract::RelationControl;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DatabaseProfileMetric {
    Completeness,
    Statistics,
    Distribution,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DatabaseProfileQuery {
    pub columns: Vec<String>,
    pub metrics: Vec<DatabaseProfileMetric>,
}

#[derive(Debug)]
pub struct DatabaseProfileSnapshot {
    pub completeness: Option<yss_dataset_profile::DatasetOverview>,
    pub statistics: Option<Vec<yss_dataset_profile::ColumnStats>>,
    pub distributions: Option<Vec<yss_dataset_profile::ColumnDistribution>>,
}

pub fn profile_snapshot(
    session: &DatabaseRuntimeSession,
    database: DatabaseId,
    query: &DatabaseProfileQuery,
    control: &RelationControl,
) -> Result<DatabaseProfileSnapshot, DatabaseError> {
    let (_lease, snapshot) = session.capture_operation(DatabaseOperation::Query)?;
    if !snapshot.revisions.contains_key(&database) {
        return Err(DatabaseError::not_found(
            DatabaseOperation::Query,
            Some(database),
        ));
    }
    if query.columns.is_empty()
        || query.metrics.is_empty()
        || query
            .metrics
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != query.metrics.len()
    {
        return Err(DatabaseError::invalid_request(
            DatabaseOperation::Query,
            Some(database),
        ));
    }
    session.read_physical_profile(&database, query, control)
}

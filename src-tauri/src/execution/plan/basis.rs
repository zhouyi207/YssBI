use std::collections::BTreeMap;

use super::identity::{
    PlanGraphRevision, PlanProjectSessionId, PlanRegistryFingerprint, PlanResourceId,
    PlanResourceVersion,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceKind {
    DatabaseConnection,
    DataFrame,
    File,
    Variable,
    Plot,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceAccess {
    Shared,
    Exclusive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanResourceRequirement {
    resource: PlanResourceId,
    kind: ResourceKind,
    access: ResourceAccess,
    optional: bool,
}

impl PlanResourceRequirement {
    pub fn new(
        resource: PlanResourceId,
        kind: ResourceKind,
        access: ResourceAccess,
        optional: bool,
    ) -> Self {
        Self {
            resource,
            kind,
            access,
            optional,
        }
    }

    pub fn resource(&self) -> &PlanResourceId {
        &self.resource
    }

    pub const fn kind(&self) -> ResourceKind {
        self.kind
    }

    pub const fn access(&self) -> ResourceAccess {
        self.access
    }

    pub const fn optional(&self) -> bool {
        self.optional
    }
}

pub type PlanResourceVersionSet = BTreeMap<PlanResourceId, PlanResourceVersion>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanResourceObservedState {
    Present(PlanResourceVersion),
    Absent(Option<PlanResourceVersion>),
}

pub type PlanResourceObservationSet = BTreeMap<PlanResourceId, PlanResourceObservedState>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanCompilationBasis {
    project_session: PlanProjectSessionId,
    graph_revision: PlanGraphRevision,
    registry_fingerprint: PlanRegistryFingerprint,
    resource_versions: PlanResourceVersionSet,
    resource_observations: PlanResourceObservationSet,
}

impl PlanCompilationBasis {
    pub fn new(
        project_session: PlanProjectSessionId,
        graph_revision: PlanGraphRevision,
        registry_fingerprint: PlanRegistryFingerprint,
        resource_versions: PlanResourceVersionSet,
        resource_observations: PlanResourceObservationSet,
    ) -> Self {
        Self {
            project_session,
            graph_revision,
            registry_fingerprint,
            resource_versions,
            resource_observations,
        }
    }

    pub fn project_session(&self) -> &PlanProjectSessionId {
        &self.project_session
    }

    pub const fn graph_revision(&self) -> PlanGraphRevision {
        self.graph_revision
    }

    pub const fn registry_fingerprint(&self) -> PlanRegistryFingerprint {
        self.registry_fingerprint
    }

    pub fn resource_versions(&self) -> &PlanResourceVersionSet {
        &self.resource_versions
    }

    pub fn resource_observations(&self) -> &PlanResourceObservationSet {
        &self.resource_observations
    }
}

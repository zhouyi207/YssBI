use super::CompilerDiagnostic;
use crate::node_system::analysis::{
    AnalysisResourceResolver, ResourceKey, ResourceObservationSet, ResourceObservedState,
};
use crate::node_system::analysis::{
    CompilationBasis, DiagnosticLocation, NodeDiagnostic, ResolvedInterface, ResolvedPort,
    ResolvedPortStatus, ResourceVersionSet,
};
use crate::node_system::document::materialization::ProjectedMemberRef;
use crate::node_system::document::{
    CompilationBasisToken, CompilationRegistryFingerprint, CompilationResourceKey,
    CompilationResourceVersion, MaterializationAuthorization,
};
use crate::node_system::document::{
    ConnectionId, DynamicMemberLocator, DynamicPortBinding, GraphDocument, GraphRevision,
    LastKnownPortMetadata, NodeId, OrderKey, PortAddress, PortInstanceId, PortRef,
};
use crate::node_system::protocol::{
    InterfaceResolverId, NodeProtocol, PortInstances, PortKey, PortSpec, ResolvedSchemaFact,
    TypeExpr,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub type DynamicInterfaceDiagnostic = NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>;

/// Describes whether a schema member can safely retain state between resolutions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaFieldIdentityGuarantee {
    Stable,
    SnapshotScoped,
    None,
}

impl SchemaFieldIdentityGuarantee {
    pub const fn permits_persistent_state(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// A member reported by an interface resolver for one compilation snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceResolverMember {
    pub basis: CompilationBasis<GraphRevision>,
    pub locator: DynamicMemberLocator,
    pub label: String,
    pub value_type: TypeExpr,
    pub identity: SchemaFieldIdentityGuarantee,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceResolverDiagnostic {
    pub locator: DynamicMemberLocator,
    pub diagnostic: CompilerDiagnostic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceResolverOutput {
    pub members: Box<[InterfaceResolverMember]>,
    pub diagnostics: Box<[InterfaceResolverDiagnostic]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceResolverError {
    pub detail: Box<str>,
    pub resource: Option<(ResourceKey, Box<str>)>,
}

impl InterfaceResolverError {
    pub fn new(detail: impl Into<Box<str>>) -> Self {
        Self {
            detail: detail.into(),
            resource: None,
        }
    }

    pub fn from_resource(error: &crate::node_system::analysis::ResourceResolutionError) -> Self {
        Self {
            detail: error.to_string().into(),
            resource: Some((error.key().clone(), error.reason().into())),
        }
    }
}

impl std::fmt::Display for InterfaceResolverError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for InterfaceResolverError {}

pub struct InterfaceResolverRequest<'a> {
    pub basis: &'a CompilationBasis<GraphRevision>,
    pub node_id: NodeId,
    pub template: &'a PortSpec,
    pub protocol: &'a NodeProtocol,
    pub document: &'a GraphDocument,
    pub resolved_schemas: &'a BTreeMap<PortAddress, ResolvedSchemaFact>,
    pub resources: &'a mut dyn AnalysisResourceResolver,
}

pub trait InterfaceResolver: Send + Sync {
    fn schema_dependencies(&self) -> &[PortKey] {
        &[]
    }

    fn resolve(
        &self,
        request: InterfaceResolverRequest<'_>,
    ) -> Result<InterfaceResolverOutput, InterfaceResolverError>;
}

#[derive(Default)]
pub struct InterfaceResolverSet {
    resolvers: BTreeMap<InterfaceResolverId, Arc<dyn InterfaceResolver>>,
}

impl InterfaceResolverSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        id: InterfaceResolverId,
        resolver: Arc<dyn InterfaceResolver>,
    ) -> Result<(), DuplicateResolver> {
        if self.resolvers.contains_key(&id) {
            return Err(DuplicateResolver(id));
        }
        self.resolvers.insert(id, resolver);
        Ok(())
    }

    pub fn get(&self, id: &InterfaceResolverId) -> Option<&dyn InterfaceResolver> {
        self.resolvers.get(id).map(Arc::as_ref)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateResolver(pub InterfaceResolverId);

impl std::fmt::Display for DuplicateResolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "interface resolver '{}' is already registered",
            self.0
        )
    }
}

impl std::error::Error for DuplicateResolver {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectedDynamicPortBinding {
    Resolved {
        origin: DynamicMemberLocator,
        order: OrderKey,
        last_known: LastKnownPortMetadata,
        identity: SchemaFieldIdentityGuarantee,
    },
    Orphan {
        origin: DynamicMemberLocator,
        order: OrderKey,
        last_known: LastKnownPortMetadata,
    },
}

/// Resolver output validated against the authoritative compilation basis.
///
/// Construction stays inside the compiler so callers cannot relabel arbitrary
/// resolver output as validated materialization input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProjectedMember {
    template: PortKey,
    direction: crate::node_system::protocol::PortDirection,
    kind: crate::node_system::protocol::PortKind,
    connections: crate::node_system::protocol::ConnectionsPerPort,
    member: InterfaceResolverMember,
    projection_address: PortAddress,
    bound_address: Option<PortAddress>,
}

impl ValidatedProjectedMember {
    pub const fn template(&self) -> &PortKey {
        &self.template
    }

    pub(crate) const fn direction(&self) -> crate::node_system::protocol::PortDirection {
        self.direction
    }

    pub(crate) const fn kind(&self) -> crate::node_system::protocol::PortKind {
        self.kind
    }

    pub(crate) const fn connections(&self) -> crate::node_system::protocol::ConnectionsPerPort {
        self.connections
    }

    pub const fn member(&self) -> &InterfaceResolverMember {
        &self.member
    }

    pub const fn projection_address(&self) -> &PortAddress {
        &self.projection_address
    }

    pub const fn bound_address(&self) -> Option<&PortAddress> {
        self.bound_address.as_ref()
    }

    pub(crate) fn authorize_materialization(
        &self,
        graph_path: crate::node_system::document::GraphResourcePath,
        basis: &CompilationBasis<GraphRevision>,
        order: OrderKey,
    ) -> (ProjectedMemberRef, MaterializationAuthorization) {
        let token = CompilationBasisToken::new(
            graph_path,
            basis.graph_revision,
            CompilationRegistryFingerprint::from_bytes(*basis.registry_fingerprint.as_bytes()),
            basis
                .resource_versions
                .iter()
                .map(|(key, version)| {
                    (
                        CompilationResourceKey::new(key.as_str()),
                        CompilationResourceVersion::new(version.as_str()),
                    )
                })
                .collect(),
        );
        let member = ProjectedMemberRef::new(
            token,
            self.projection_address.node_id,
            self.template.clone(),
            self.direction,
            self.member.locator.clone(),
            LastKnownPortMetadata {
                label: self.member.label.clone(),
                value_type: Some(self.member.value_type.clone()),
            },
        );
        let authorization = MaterializationAuthorization::new(member.clone(), order);
        (member, authorization)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNodeInterfaceProjection {
    pub projected_bindings: BTreeMap<PortAddress, ProjectedDynamicPortBinding>,
    pub available_members: Box<[ValidatedProjectedMember]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedInterfaceProjection {
    pub basis: CompilationBasis<GraphRevision>,
    pub nodes: BTreeMap<NodeId, ValidatedNodeInterfaceProjection>,
}

impl ValidatedInterfaceProjection {
    fn find_materialization_candidate(
        &self,
        projection_address: &PortAddress,
    ) -> Option<(usize, &ValidatedProjectedMember)> {
        self.nodes
            .get(&projection_address.node_id)?
            .available_members
            .iter()
            .enumerate()
            .find(|(_, candidate)| {
                candidate.projection_address == *projection_address
                    && candidate.bound_address.is_none()
                    && candidate.member.identity.permits_persistent_state()
            })
    }

    #[cfg(test)]
    pub(crate) fn materialization_candidate(
        &self,
        projection_address: &PortAddress,
    ) -> Option<&ValidatedProjectedMember> {
        self.find_materialization_candidate(projection_address)
            .map(|(_, candidate)| candidate)
    }

    pub(crate) fn authorize_materialization_candidate(
        &self,
        graph_path: &crate::node_system::document::GraphResourcePath,
        projection_address: &PortAddress,
    ) -> Option<(
        &ValidatedProjectedMember,
        ProjectedMemberRef,
        MaterializationAuthorization,
    )> {
        let (index, candidate) = self.find_materialization_candidate(projection_address)?;
        let (member, authorization) = candidate.authorize_materialization(
            graph_path.clone(),
            &self.basis,
            OrderKey(format!("{index:05}").into()),
        );
        Some((candidate, member, authorization))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicInterfaceResolution {
    pub interface: ResolvedInterface<NodeId, PortAddress>,
    pub projected_bindings: BTreeMap<PortAddress, ProjectedDynamicPortBinding>,
    pub available_members: Box<[ValidatedProjectedMember]>,
    pub diagnostics: Box<[DynamicInterfaceDiagnostic]>,
    pub deferred_for_schema: bool,
}

/// Validates resolver output against the exact graph, registry, and resource snapshot.
fn validate_projected_member_basis(
    expected: &CompilationBasis<GraphRevision>,
    member: &InterfaceResolverMember,
) -> bool {
    member.basis == *expected
}

/// Resolves a node interface without mutating its document or inventing persistent bindings.
pub fn materialize_dynamic_interface(
    basis: &CompilationBasis<GraphRevision>,
    node_id: NodeId,
    protocol: &NodeProtocol,
    document: &GraphDocument,
    resolvers: &InterfaceResolverSet,
) -> DynamicInterfaceResolution {
    let mut resources = EmptyResourceSnapshot {
        reads: ResourceVersionSet::new(),
        observations: ResourceObservationSet::new(),
    };
    let resolved_schemas = BTreeMap::new();
    materialize_dynamic_interface_with_resources(
        basis,
        node_id,
        protocol,
        document,
        &resolved_schemas,
        &mut resources,
        resolvers,
    )
}

pub(crate) fn materialize_dynamic_interface_with_resources(
    basis: &CompilationBasis<GraphRevision>,
    node_id: NodeId,
    protocol: &NodeProtocol,
    document: &GraphDocument,
    resolved_schemas: &BTreeMap<PortAddress, ResolvedSchemaFact>,
    resources: &mut dyn AnalysisResourceResolver,
    resolvers: &InterfaceResolverSet,
) -> DynamicInterfaceResolution {
    let mut state = MaterializationState::new(node_id, document);

    for spec in protocol.interface.ports.iter() {
        match &spec.instances {
            PortInstances::Declared => state.add_declared(spec),
            PortInstances::UserCreated { .. } => state.add_existing_instances(spec, None),
            PortInstances::Derived { resolver } => {
                let Some(implementation) = resolvers.get(resolver) else {
                    state.push_node_diagnostic(CompilerDiagnostic::InterfaceResolverMissing {
                        resolver_id: resolver.to_string().into(),
                    });
                    state.add_existing_instances(spec, None);
                    continue;
                };
                if implementation.schema_dependencies().iter().any(|key| {
                    !resolved_schemas.contains_key(&PortAddress::declared(node_id, key.clone()))
                }) {
                    state.add_existing_instances(spec, None);
                    state.deferred_for_schema = true;
                    continue;
                }
                match implementation.resolve(InterfaceResolverRequest {
                    basis,
                    node_id,
                    template: spec,
                    protocol,
                    document,
                    resolved_schemas,
                    resources,
                }) {
                    Ok(output) => {
                        if state.add_resolved_instances(basis, spec, output).is_err() {
                            state.push_node_diagnostic(
                                CompilerDiagnostic::InterfaceResolverFailed {
                                    resolver_id: resolver.to_string().into(),
                                },
                            );
                            state.add_existing_instances(spec, None);
                        }
                    }
                    Err(error) => {
                        if let Some((resource_key, reason)) = error.resource {
                            state.push_node_diagnostic(
                                CompilerDiagnostic::resource_resolution_failed(
                                    resource_key.as_str(),
                                    reason,
                                ),
                            );
                        } else {
                            state.push_node_diagnostic(
                                CompilerDiagnostic::InterfaceResolverFailed {
                                    resolver_id: resolver.to_string().into(),
                                },
                            );
                        }
                        state.add_existing_instances(spec, None);
                    }
                }
            }
        }
    }

    state.finish()
}

struct EmptyResourceSnapshot {
    reads: ResourceVersionSet,
    observations: ResourceObservationSet,
}

impl AnalysisResourceResolver for EmptyResourceSnapshot {
    fn resolve_function(
        &mut self,
        path: &crate::node_system::document::GraphResourcePath,
    ) -> Result<
        crate::node_system::analysis::ResolvedFunction<'_>,
        crate::node_system::analysis::ResourceResolutionError,
    > {
        let key = ResourceKey::new(path.0.clone());
        let state = ResourceObservedState::Absent(None);
        self.observations.insert(key.clone(), state.clone());
        Err(crate::node_system::analysis::ResourceResolutionError::new(
            key,
            state,
            format!("function resource '{}' is unavailable", path.0),
        ))
    }

    fn resolve_variable(
        &mut self,
        id: &crate::variable::VariableId,
    ) -> Result<
        crate::node_system::analysis::ResolvedVariable<'_>,
        crate::node_system::analysis::ResourceResolutionError,
    > {
        let key = ResourceKey::new(format!("variables/{id}"));
        let state = ResourceObservedState::Absent(None);
        self.observations.insert(key.clone(), state.clone());
        Err(crate::node_system::analysis::ResourceResolutionError::new(
            key,
            state,
            format!("variable resource '{id}' is unavailable"),
        ))
    }

    fn resolve_database(
        &mut self,
        id: &str,
    ) -> Result<
        crate::node_system::analysis::ResolvedDatabase<'_>,
        crate::node_system::analysis::ResourceResolutionError,
    > {
        let key = ResourceKey::new(format!("databases/{id}"));
        let state = ResourceObservedState::Absent(None);
        self.observations.insert(key.clone(), state.clone());
        Err(crate::node_system::analysis::ResourceResolutionError::new(
            key,
            state,
            format!("database resource '{id}' is unavailable"),
        ))
    }

    fn reads(&self) -> &ResourceVersionSet {
        &self.reads
    }

    fn observations(&self) -> &ResourceObservationSet {
        &self.observations
    }
}

struct MaterializationState<'a> {
    node_id: NodeId,
    document: &'a GraphDocument,
    ports: BTreeMap<PortAddress, ResolvedPort<PortAddress>>,
    port_sequence: Vec<PortAddress>,
    projected_bindings: BTreeMap<PortAddress, ProjectedDynamicPortBinding>,
    available_members: Vec<ValidatedProjectedMember>,
    diagnostics: Vec<DynamicInterfaceDiagnostic>,
    deferred_for_schema: bool,
}

impl<'a> MaterializationState<'a> {
    fn new(node_id: NodeId, document: &'a GraphDocument) -> Self {
        Self {
            node_id,
            document,
            ports: BTreeMap::new(),
            port_sequence: Vec::new(),
            projected_bindings: BTreeMap::new(),
            available_members: Vec::new(),
            diagnostics: Vec::new(),
            deferred_for_schema: false,
        }
    }

    fn insert_port(&mut self, address: PortAddress, port: ResolvedPort<PortAddress>) {
        if !self.ports.contains_key(&address) {
            self.port_sequence.push(address.clone());
        }
        self.ports.insert(address, port);
    }

    fn add_declared(&mut self, spec: &PortSpec) {
        let address = PortAddress::declared(self.node_id, spec.key.clone());
        self.insert_port(
            address.clone(),
            resolved_port(
                address,
                spec,
                None,
                spec.value_type.clone(),
                ResolvedPortStatus::Resolved,
            ),
        );
    }

    fn add_resolved_instances(
        &mut self,
        basis: &CompilationBasis<GraphRevision>,
        spec: &PortSpec,
        output: InterfaceResolverOutput,
    ) -> Result<(), InterfaceResolverError> {
        let InterfaceResolverOutput {
            members,
            diagnostics,
        } = output;
        let mut ordered_members = Vec::new();
        let mut locator_indices = BTreeMap::new();
        let mut duplicated = BTreeSet::new();
        for member in members.into_vec() {
            if !validate_projected_member_basis(basis, &member) {
                self.push_node_diagnostic(CompilerDiagnostic::InterfaceBasisMismatch {
                    expected_basis: serde_json::to_string(basis)
                        .expect("compilation basis is serializable")
                        .into(),
                    actual_basis: serde_json::to_string(&member.basis)
                        .expect("compilation basis is serializable")
                        .into(),
                });
                continue;
            }
            let locator = member.locator.clone();
            if let Some(index) = locator_indices.get(&locator).copied() {
                ordered_members[index] = None;
                duplicated.insert(locator);
            } else {
                locator_indices.insert(locator, ordered_members.len());
                ordered_members.push(Some(member));
            }
        }
        // A duplicated locator is ambiguous even if labels differ; reject it rather than name-match.
        for locator in duplicated {
            locator_indices.remove(&locator);
            self.push_node_diagnostic(CompilerDiagnostic::InterfaceDuplicateLocator {
                port_key: spec.key.to_string().into(),
                locator: serde_json::to_string(&locator)
                    .expect("dynamic member locators are serializable")
                    .into(),
            });
        }
        let by_locator = ordered_members
            .iter()
            .filter_map(Option::as_ref)
            .map(|member| (member.locator.clone(), member.clone()))
            .collect::<BTreeMap<_, _>>();
        if let Some(unlocatable) = diagnostics
            .iter()
            .find(|diagnostic| !by_locator.contains_key(&diagnostic.locator))
        {
            return Err(InterfaceResolverError::new(format!(
                "interface resolver diagnostic locator '{}' is absent from validated members",
                locator_detail(&unlocatable.locator),
            )));
        }

        let mut addresses_by_locator = BTreeMap::new();
        for member in ordered_members.into_iter().flatten() {
            let binding = self
                .find_binding(spec, &member.locator)
                .map(|(address, binding)| (address.clone(), binding.clone()));
            let bound_address = binding.as_ref().map(|(address, _)| address.clone());
            let projection_address = match bound_address.clone() {
                Some(address) => address,
                None => self
                    .unbound_projection_address(basis, spec, &member.locator)
                    .ok_or_else(|| {
                        InterfaceResolverError::new(
                            "dynamic port projection identity space is exhausted",
                        )
                    })?,
            };
            if let Some((address, binding)) = binding {
                self.add_current_instance(spec, address, &binding, &member);
            } else {
                self.insert_port(
                    projection_address.clone(),
                    resolved_port(
                        projection_address.clone(),
                        spec,
                        Some(member.label.clone().into()),
                        member.value_type.clone(),
                        ResolvedPortStatus::Resolved,
                    ),
                );
            }
            addresses_by_locator.insert(member.locator.clone(), projection_address.clone());
            self.available_members.push(ValidatedProjectedMember {
                template: spec.key.clone(),
                direction: spec.direction,
                kind: spec.kind,
                connections: spec.connections,
                member,
                projection_address,
                bound_address,
            });
        }
        self.add_existing_instances(spec, Some(&by_locator));
        for diagnostic in diagnostics {
            let address = addresses_by_locator
                .get(&diagnostic.locator)
                .expect("resolver diagnostic locators were validated");
            self.push_port_diagnostic(diagnostic.diagnostic, address);
        }
        Ok(())
    }

    fn add_current_instance(
        &mut self,
        spec: &PortSpec,
        address: PortAddress,
        binding: &DynamicPortBinding,
        member: &InterfaceResolverMember,
    ) {
        let Some((origin, order, _)) = binding_parts(binding) else {
            return;
        };
        if !member.identity.permits_persistent_state() {
            self.diagnose_forbidden_state(&address);
        }
        let last_known = LastKnownPortMetadata {
            label: member.label.clone(),
            value_type: Some(member.value_type.clone()),
        };
        self.projected_bindings.insert(
            address.clone(),
            ProjectedDynamicPortBinding::Resolved {
                origin: origin.clone(),
                order: order.clone(),
                last_known,
                identity: member.identity,
            },
        );
        self.insert_port(
            address.clone(),
            resolved_port(
                address,
                spec,
                Some(member.label.clone().into()),
                member.value_type.clone(),
                ResolvedPortStatus::Resolved,
            ),
        );
    }

    fn add_existing_instances(
        &mut self,
        spec: &PortSpec,
        members: Option<&BTreeMap<DynamicMemberLocator, InterfaceResolverMember>>,
    ) {
        let mut bindings = self
            .document
            .port_bindings
            .iter()
            .filter(|(address, _)| {
                address.node_id == self.node_id
                    && matches!(&address.port, PortRef::Instance { template, .. } if template == &spec.key)
            })
            .map(|(address, binding)| (address.clone(), binding.clone()))
            .collect::<Vec<_>>();
        bindings.sort_by(
            |(left_address, left_binding), (right_address, right_binding)| {
                dynamic_binding_order(left_binding)
                    .cmp(dynamic_binding_order(right_binding))
                    .then_with(|| left_address.cmp(right_address))
            },
        );

        for (address, binding) in bindings {
            if members.is_some_and(|values| {
                binding_origin(&binding).is_some_and(|origin| values.contains_key(origin))
            }) {
                continue;
            }
            if let DynamicPortBinding::UserCreated { .. } = &binding {
                let status = if matches!(spec.instances, PortInstances::UserCreated { .. }) {
                    ResolvedPortStatus::Resolved
                } else {
                    self.push_port_diagnostic(
                        CompilerDiagnostic::PortBindingKindMismatch {
                            expected_kind: "derived".into(),
                            actual_kind: "user_created".into(),
                        },
                        &address,
                    );
                    ResolvedPortStatus::Orphan
                };
                self.insert_port(
                    address.clone(),
                    resolved_port(address, spec, None, spec.value_type.clone(), status),
                );
                continue;
            }
            if matches!(spec.instances, PortInstances::UserCreated { .. }) {
                self.push_port_diagnostic(
                    CompilerDiagnostic::PortBindingKindMismatch {
                        expected_kind: "user_created".into(),
                        actual_kind: "derived".into(),
                    },
                    &address,
                );
                self.insert_port(
                    address.clone(),
                    resolved_port(
                        address,
                        spec,
                        None,
                        spec.value_type.clone(),
                        ResolvedPortStatus::Orphan,
                    ),
                );
                continue;
            }
            let Some((origin, order, old_last_known)) = binding_parts(&binding) else {
                continue;
            };
            let member = members.and_then(|values| values.get(origin));
            let status = if members.is_some() && member.is_none() {
                ResolvedPortStatus::Orphan
            } else if matches!(binding, DynamicPortBinding::Orphan { .. }) && members.is_none() {
                ResolvedPortStatus::Orphan
            } else {
                ResolvedPortStatus::Resolved
            };
            let mut last_known = member
                .map(|value| LastKnownPortMetadata {
                    label: value.label.clone(),
                    value_type: Some(value.value_type.clone()),
                })
                .or_else(|| old_last_known.cloned())
                .unwrap_or_default();
            if last_known.label.is_empty() {
                last_known.label = locator_detail(origin);
            }

            let instance_label: Box<str> = last_known.label.clone().into();
            let value_type = last_known
                .value_type
                .clone()
                .unwrap_or_else(|| spec.value_type.clone());
            let projection = match (status, member) {
                (ResolvedPortStatus::Resolved, Some(member)) => {
                    if !member.identity.permits_persistent_state() {
                        self.diagnose_forbidden_state(&address);
                    }
                    ProjectedDynamicPortBinding::Resolved {
                        origin: origin.clone(),
                        order: order.clone(),
                        last_known,
                        identity: member.identity,
                    }
                }
                (ResolvedPortStatus::Resolved, None) => ProjectedDynamicPortBinding::Resolved {
                    origin: origin.clone(),
                    order: order.clone(),
                    last_known,
                    identity: SchemaFieldIdentityGuarantee::Stable,
                },
                (ResolvedPortStatus::Orphan, _) => {
                    self.push_port_diagnostic(
                        CompilerDiagnostic::PortOrphan {
                            port: address.to_string().into(),
                        },
                        &address,
                    );
                    ProjectedDynamicPortBinding::Orphan {
                        origin: origin.clone(),
                        order: order.clone(),
                        last_known,
                    }
                }
            };
            self.projected_bindings.insert(address.clone(), projection);
            self.insert_port(
                address.clone(),
                resolved_port(address, spec, Some(instance_label), value_type, status),
            );
        }
    }

    fn find_binding(
        &self,
        spec: &PortSpec,
        locator: &DynamicMemberLocator,
    ) -> Option<(&PortAddress, &DynamicPortBinding)> {
        self.document.port_bindings.iter().find(|(address, binding)| {
            address.node_id == self.node_id
                && matches!(&address.port, PortRef::Instance { template, .. } if template == &spec.key)
                && binding_origin(binding).is_some_and(|origin| origin == locator)
        })
    }

    fn unbound_projection_address(
        &self,
        basis: &CompilationBasis<GraphRevision>,
        spec: &PortSpec,
        locator: &DynamicMemberLocator,
    ) -> Option<PortAddress> {
        (0..=u32::MAX).find_map(|salt| {
            let address = projected_address(basis, self.node_id, &spec.key, locator, salt);
            (!self.ports.contains_key(&address)
                && !self.document.port_bindings.contains_key(&address))
            .then_some(address)
        })
    }

    fn diagnose_forbidden_state(&mut self, address: &PortAddress) {
        for (&connection_id, connection) in &self.document.connections {
            if connection.input == *address || connection.output == *address {
                self.diagnostics.push(
                    CompilerDiagnostic::InterfaceIdentityNoneConnection {
                        port: address.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Connection(connection_id)),
                );
            }
        }
        if self.document.input_states.contains_key(address) {
            self.push_port_diagnostic(
                CompilerDiagnostic::InterfaceIdentityNoneOverride {
                    port: address.to_string().into(),
                },
                address,
            );
        }
    }

    fn push_node_diagnostic(&mut self, diagnostic: CompilerDiagnostic) {
        self.diagnostics
            .push(diagnostic.into_node(DiagnosticLocation::Node(self.node_id)));
    }

    fn push_port_diagnostic(&mut self, diagnostic: CompilerDiagnostic, address: &PortAddress) {
        self.diagnostics
            .push(diagnostic.into_node(DiagnosticLocation::Port(address.clone())));
    }

    fn finish(self) -> DynamicInterfaceResolution {
        let mut ports = self.ports;
        let ordered_ports = self
            .port_sequence
            .into_iter()
            .filter_map(|address| ports.remove(&address))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        DynamicInterfaceResolution {
            interface: ResolvedInterface {
                node_id: self.node_id,
                ports: ordered_ports,
            },
            projected_bindings: self.projected_bindings,
            available_members: self.available_members.into_boxed_slice(),
            diagnostics: self.diagnostics.into_boxed_slice(),
            deferred_for_schema: self.deferred_for_schema,
        }
    }
}

fn dynamic_binding_order(binding: &DynamicPortBinding) -> &OrderKey {
    match binding {
        DynamicPortBinding::UserCreated { order }
        | DynamicPortBinding::Resolved { order, .. }
        | DynamicPortBinding::Orphan { order, .. } => order,
    }
}

fn binding_origin(binding: &DynamicPortBinding) -> Option<&DynamicMemberLocator> {
    match binding {
        DynamicPortBinding::UserCreated { .. } => None,
        DynamicPortBinding::Resolved { origin, .. } | DynamicPortBinding::Orphan { origin, .. } => {
            Some(origin)
        }
    }
}

fn binding_parts(
    binding: &DynamicPortBinding,
) -> Option<(
    &DynamicMemberLocator,
    &OrderKey,
    Option<&LastKnownPortMetadata>,
)> {
    match binding {
        DynamicPortBinding::UserCreated { .. } => None,
        DynamicPortBinding::Resolved {
            origin,
            order,
            last_known,
        } => Some((origin, order, Some(last_known))),
        DynamicPortBinding::Orphan {
            origin,
            order,
            last_known,
        } => Some((origin, order, Some(last_known))),
    }
}

fn resolved_port(
    address: PortAddress,
    spec: &PortSpec,
    instance_label: Option<Box<str>>,
    value_type: TypeExpr,
    status: ResolvedPortStatus,
) -> ResolvedPort<PortAddress> {
    ResolvedPort {
        address,
        template: spec.key.clone(),
        direction: spec.direction,
        kind: spec.kind,
        instance_label,
        value_type,
        status,
    }
}

fn locator_detail(locator: &DynamicMemberLocator) -> String {
    match locator {
        DynamicMemberLocator::FunctionParameter {
            function,
            parameter,
        } => {
            format!("function:{}/{}", function.0, parameter.0)
        }
        DynamicMemberLocator::SchemaField { source, field } => {
            format!("schema:{}/{}", source.0, field.0)
        }
    }
}

fn projected_address(
    basis: &CompilationBasis<GraphRevision>,
    node_id: NodeId,
    template: &PortKey,
    locator: &DynamicMemberLocator,
    salt: u32,
) -> PortAddress {
    let encoded = serde_json::to_vec(&(basis, node_id, template, locator, salt))
        .expect("projection identities are serializable");
    let mut high = 0x6c62_272e_07bb_0142u64;
    let mut low = 0x62b8_2175_6295_c58du64;
    for byte in encoded {
        high = high.wrapping_mul(0x0000_0100_0000_01b3) ^ u64::from(byte);
        low ^= u64::from(byte);
        low = low.rotate_left(13).wrapping_mul(0x9e37_79b1_85eb_ca87);
    }
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&high.to_be_bytes());
    bytes[8..].copy_from_slice(&low.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    PortAddress::instance(
        node_id,
        template.clone(),
        PortInstanceId::from_uuid(uuid::Uuid::from_bytes(bytes)),
    )
}

use super::pipeline::ResourceSnapshot;
use crate::node_system::analysis::{
    CompilationBasis, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity, NodeDiagnostic,
    ResolvedInterface, ResolvedPort, ResolvedPortStatus, ResourceVersionSet,
};
use crate::node_system::document::{
    ConnectionId, DynamicMemberLocator, DynamicPortBinding, GraphDocument, GraphRevision,
    LastKnownPortMetadata, NodeId, OrderKey, PortAddress, PortInstanceId, PortRef,
};
use crate::node_system::protocol::{
    I18nKey, InterfaceResolverId, NodeProtocol, PortInstances, PortKey, PortSpec,
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
    pub identity: SchemaFieldIdentityGuarantee,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceResolverError {
    pub detail: Box<str>,
}

impl InterfaceResolverError {
    pub fn new(detail: impl Into<Box<str>>) -> Self {
        Self {
            detail: detail.into(),
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
    pub resources: &'a dyn ResourceSnapshot,
}

pub trait InterfaceResolver: Send + Sync {
    fn resolve(
        &self,
        request: InterfaceResolverRequest<'_>,
    ) -> Result<Box<[InterfaceResolverMember]>, InterfaceResolverError>;
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
    member: InterfaceResolverMember,
    projection_address: PortAddress,
    bound_address: Option<PortAddress>,
}

impl ValidatedProjectedMember {
    pub const fn template(&self) -> &PortKey {
        &self.template
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
    pub fn materialization_candidate(
        &self,
        projection_address: &PortAddress,
    ) -> Option<&ValidatedProjectedMember> {
        self.nodes
            .get(&projection_address.node_id)?
            .available_members
            .iter()
            .find(|candidate| {
                candidate.projection_address == *projection_address
                    && candidate.bound_address.is_none()
                    && candidate.member.identity.permits_persistent_state()
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicInterfaceResolution {
    pub interface: ResolvedInterface<NodeId, PortAddress>,
    pub projected_bindings: BTreeMap<PortAddress, ProjectedDynamicPortBinding>,
    pub available_members: Box<[ValidatedProjectedMember]>,
    pub diagnostics: Box<[DynamicInterfaceDiagnostic]>,
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
    materialize_dynamic_interface_with_resources(
        basis,
        node_id,
        protocol,
        document,
        &EmptyResourceSnapshot,
        resolvers,
    )
}

pub(crate) fn materialize_dynamic_interface_with_resources(
    basis: &CompilationBasis<GraphRevision>,
    node_id: NodeId,
    protocol: &NodeProtocol,
    document: &GraphDocument,
    resources: &dyn ResourceSnapshot,
    resolvers: &InterfaceResolverSet,
) -> DynamicInterfaceResolution {
    let mut state = MaterializationState::new(node_id, document);

    for spec in protocol.interface.ports.iter() {
        match &spec.instances {
            PortInstances::Declared => state.add_declared(spec),
            PortInstances::UserCreated { .. } => state.add_existing_instances(spec, None),
            PortInstances::Derived { resolver } => {
                let Some(implementation) = resolvers.get(resolver) else {
                    state.push_node_diagnostic(
                        "compiler.interface.resolver_missing",
                        format!("{}:{}", spec.key, resolver),
                    );
                    state.add_existing_instances(spec, None);
                    continue;
                };
                match implementation.resolve(InterfaceResolverRequest {
                    basis,
                    node_id,
                    template: spec,
                    protocol,
                    document,
                    resources,
                }) {
                    Ok(members) => state.add_resolved_instances(basis, spec, members),
                    Err(error) => {
                        state.push_node_diagnostic(
                            "compiler.interface.resolver_failed",
                            format!("{}:{}", spec.key, error),
                        );
                        state.add_existing_instances(spec, None);
                    }
                }
            }
        }
    }

    state.finish()
}

struct EmptyResourceSnapshot;

impl ResourceSnapshot for EmptyResourceSnapshot {
    fn versions(&self) -> ResourceVersionSet {
        ResourceVersionSet::new()
    }
}

struct MaterializationState<'a> {
    node_id: NodeId,
    document: &'a GraphDocument,
    ports: BTreeMap<PortAddress, ResolvedPort<PortAddress>>,
    projected_bindings: BTreeMap<PortAddress, ProjectedDynamicPortBinding>,
    available_members: Vec<ValidatedProjectedMember>,
    diagnostics: Vec<DynamicInterfaceDiagnostic>,
}

impl<'a> MaterializationState<'a> {
    fn new(node_id: NodeId, document: &'a GraphDocument) -> Self {
        Self {
            node_id,
            document,
            ports: BTreeMap::new(),
            projected_bindings: BTreeMap::new(),
            available_members: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn add_declared(&mut self, spec: &PortSpec) {
        let address = PortAddress::declared(self.node_id, spec.key.clone());
        self.ports.insert(
            address.clone(),
            resolved_port(address, spec, ResolvedPortStatus::Resolved),
        );
    }

    fn add_resolved_instances(
        &mut self,
        basis: &CompilationBasis<GraphRevision>,
        spec: &PortSpec,
        members: Box<[InterfaceResolverMember]>,
    ) {
        let mut by_locator = BTreeMap::new();
        let mut duplicated = BTreeSet::new();
        for member in members.into_vec() {
            if !validate_projected_member_basis(basis, &member) {
                self.push_node_diagnostic(
                    "compiler.interface.basis_mismatch",
                    format!("{}:{}", spec.key, locator_detail(&member.locator)),
                );
                continue;
            }
            let locator = member.locator.clone();
            if by_locator.insert(locator.clone(), member).is_some() {
                duplicated.insert(locator);
            }
        }
        // A duplicated locator is ambiguous even if labels differ; reject it rather than name-match.
        for locator in duplicated {
            by_locator.remove(&locator);
            self.push_node_diagnostic(
                "compiler.interface.duplicate_locator",
                format!("{}:{}", spec.key, locator_detail(&locator)),
            );
        }

        self.add_existing_instances(spec, Some(&by_locator));
        for member in by_locator.into_values() {
            let bound_address = self
                .find_binding(spec, &member.locator)
                .map(|(address, _)| address.clone());
            let projection_address = bound_address
                .clone()
                .unwrap_or_else(|| self.unbound_projection_address(basis, spec, &member.locator));
            if bound_address.is_none() {
                self.ports.insert(
                    projection_address.clone(),
                    resolved_port(
                        projection_address.clone(),
                        spec,
                        ResolvedPortStatus::Resolved,
                    ),
                );
            }
            self.available_members.push(ValidatedProjectedMember {
                template: spec.key.clone(),
                member,
                projection_address,
                bound_address,
            });
        }
    }

    fn add_existing_instances(
        &mut self,
        spec: &PortSpec,
        members: Option<&BTreeMap<DynamicMemberLocator, InterfaceResolverMember>>,
    ) {
        let bindings = self
            .document
            .port_bindings
            .iter()
            .filter(|(address, _)| {
                address.node_id == self.node_id
                    && matches!(&address.port, PortRef::Instance { template, .. } if template == &spec.key)
            })
            .map(|(address, binding)| (address.clone(), binding.clone()))
            .collect::<Vec<_>>();

        for (address, binding) in bindings {
            if let DynamicPortBinding::UserCreated { .. } = &binding {
                let status = if matches!(spec.instances, PortInstances::UserCreated { .. }) {
                    ResolvedPortStatus::Resolved
                } else {
                    self.push_port_diagnostic(
                        "compiler.port.binding_kind_mismatch",
                        &address,
                        address.to_string(),
                    );
                    ResolvedPortStatus::Orphan
                };
                self.ports
                    .insert(address.clone(), resolved_port(address, spec, status));
                continue;
            }
            if matches!(spec.instances, PortInstances::UserCreated { .. }) {
                self.push_port_diagnostic(
                    "compiler.port.binding_kind_mismatch",
                    &address,
                    address.to_string(),
                );
                self.ports.insert(
                    address.clone(),
                    resolved_port(address, spec, ResolvedPortStatus::Orphan),
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
            let last_known = member
                .map(|value| LastKnownPortMetadata {
                    label: value.label.clone(),
                })
                .or_else(|| old_last_known.cloned())
                .unwrap_or_else(|| LastKnownPortMetadata {
                    label: locator_detail(origin),
                });

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
                        "compiler.port.orphan",
                        &address,
                        locator_detail(origin),
                    );
                    ProjectedDynamicPortBinding::Orphan {
                        origin: origin.clone(),
                        order: order.clone(),
                        last_known,
                    }
                }
            };
            self.projected_bindings.insert(address.clone(), projection);
            self.ports
                .insert(address.clone(), resolved_port(address, spec, status));
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
    ) -> PortAddress {
        let mut salt = 0u32;
        loop {
            let address = projected_address(basis, self.node_id, &spec.key, locator, salt);
            if !self.ports.contains_key(&address)
                && !self.document.port_bindings.contains_key(&address)
            {
                return address;
            }
            salt = salt.wrapping_add(1);
        }
    }

    fn diagnose_forbidden_state(&mut self, address: &PortAddress) {
        for (&connection_id, connection) in &self.document.connections {
            if connection.input == *address || connection.output == *address {
                self.diagnostics.push(diagnostic(
                    "compiler.interface.identity_none_connection",
                    DiagnosticLocation::Connection(connection_id),
                    address.to_string(),
                ));
            }
        }
        if self.document.input_states.contains_key(address) {
            self.push_port_diagnostic(
                "compiler.interface.identity_none_override",
                address,
                address.to_string(),
            );
        }
    }

    fn push_node_diagnostic(&mut self, code: &'static str, detail: String) {
        self.diagnostics.push(diagnostic(
            code,
            DiagnosticLocation::Node(self.node_id),
            detail,
        ));
    }

    fn push_port_diagnostic(&mut self, code: &'static str, address: &PortAddress, detail: String) {
        self.diagnostics.push(diagnostic(
            code,
            DiagnosticLocation::Port(address.clone()),
            detail,
        ));
    }

    fn finish(self) -> DynamicInterfaceResolution {
        DynamicInterfaceResolution {
            interface: ResolvedInterface {
                node_id: self.node_id,
                ports: self
                    .ports
                    .into_values()
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
            projected_bindings: self.projected_bindings,
            available_members: self.available_members.into_boxed_slice(),
            diagnostics: self.diagnostics.into_boxed_slice(),
        }
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
        DynamicPortBinding::Resolved { origin, order } => Some((origin, order, None)),
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
    status: ResolvedPortStatus,
) -> ResolvedPort<PortAddress> {
    ResolvedPort {
        address,
        template: spec.key.clone(),
        direction: spec.direction,
        kind: spec.kind,
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

fn diagnostic(
    code: &'static str,
    primary: DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>,
    detail: String,
) -> DynamicInterfaceDiagnostic {
    NodeDiagnostic {
        code: DiagnosticCode::new(code),
        message_key: I18nKey::new(format!("diagnostics.{code}")).expect("static diagnostic key"),
        arguments: BTreeMap::from([(Box::<str>::from("detail"), detail.into_boxed_str())]),
        severity: DiagnosticSeverity::Error,
        primary,
        related: Box::new([]),
    }
}

use std::sync::Arc;

use crate::data_contract::DataType;
use crate::execution::plan::PlanCompilationBasis;
use crate::graph::analysis::{GraphAnalysis, GraphAnalysisInput};
use crate::graph::resource_catalog::{GraphResourceId, ResourceCatalogSnapshot};
use crate::graph_document::{
    DynamicPortBinding, GraphDocument, GraphResourcePath, PortAddress, PortRef,
};
use crate::node_system::catalog::{
    BuiltinCatalog, CatalogResourceEntry, LocalizedCatalog, ResourceBoundCreateArgsDto,
};
use crate::node_system::compiler::ProjectCompileCoordinator;
use crate::node_system::protocol::{
    NodeTypeId, PortDirection, PortInstances, PortKind, TypeConstructorId, TypeExpr, TypeId,
};
use crate::node_system::registry::NodeRegistry;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GraphRuntimeEpoch(u64);

impl GraphRuntimeEpoch {
    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

pub struct GraphRuntimeComponents {
    pub registry: Arc<NodeRegistry>,
    pub catalog: Arc<BuiltinCatalog>,
    pub compiler: Arc<ProjectCompileCoordinator>,
    pub resource_catalog: Arc<ResourceCatalogSnapshot>,
}

pub struct GraphRuntimeState {
    epoch: GraphRuntimeEpoch,
    components: GraphRuntimeComponents,
}

impl GraphRuntimeState {
    pub fn from_components(epoch: GraphRuntimeEpoch, components: GraphRuntimeComponents) -> Self {
        Self { epoch, components }
    }

    pub const fn epoch(&self) -> GraphRuntimeEpoch {
        self.epoch
    }

    pub fn accepts_basis(&self, basis: &PlanCompilationBasis) -> bool {
        basis.registry_fingerprint().as_bytes()
            == *self.components.registry.fingerprint().as_bytes()
    }

    pub fn resource_catalog(&self) -> &ResourceCatalogSnapshot {
        &self.components.resource_catalog
    }

    pub(crate) fn registry_fingerprint(&self) -> [u8; 32] {
        *self.components.registry.fingerprint().as_bytes()
    }

    pub(crate) fn analyze(
        &self,
        document: &GraphDocument,
        catalog: &ResourceCatalogSnapshot,
        settings: &crate::graph::settings::GraphCompileSettings,
        basis: &PlanCompilationBasis,
    ) -> GraphAnalysis {
        crate::graph::analysis::analyze(GraphAnalysisInput {
            document,
            catalog,
            settings,
            basis,
        })
    }

    pub(crate) fn localized_catalog_with_resources(
        &self,
        resources: &[CatalogResourceEntry],
        locale: &str,
    ) -> LocalizedCatalog {
        self.components.catalog.localize_with_resources(
            self.components.registry.as_ref(),
            locale,
            resources,
        )
    }

    pub(crate) fn compatible_catalog_with_resources(
        &self,
        graph_path: &GraphResourcePath,
        document: &GraphDocument,
        source: &PortAddress,
        catalog: &ResourceCatalogSnapshot,
        resources: &[CatalogResourceEntry],
        locale: &str,
    ) -> Result<LocalizedCatalog, GraphRuntimeCatalogError> {
        let source = self
            .source_port(document, source, catalog)
            .ok_or(GraphRuntimeCatalogError::SourceInvalid)?;
        let mut localized = self.components.catalog.localize_with_resources(
            self.components.registry.as_ref(),
            locale,
            resources,
        );
        localized.items.retain(|item| {
            let Ok(node_type) = NodeTypeId::new(item.node_type_id.as_ref()) else {
                return false;
            };
            let resource = item.resource_path.as_ref().and_then(|path| {
                resources.iter().find(|entry| {
                    entry.resource_path.as_str() == path.as_str() && entry.node_type_id == node_type
                })
            });
            self.candidate_ports(graph_path, &node_type, resource, catalog)
                .iter()
                .any(|candidate| ports_are_compatible(&source, candidate))
        });
        let categories = localized
            .items
            .iter()
            .map(|item| item.category_id.as_ref())
            .collect::<std::collections::BTreeSet<_>>();
        localized
            .categories
            .retain(|category| categories.contains(category.category_id.as_ref()));
        Ok(localized)
    }

    fn source_port(
        &self,
        document: &GraphDocument,
        source: &PortAddress,
        catalog: &ResourceCatalogSnapshot,
    ) -> Option<PortCandidate> {
        let node = document.nodes.get(&source.node_id)?;
        let protocol = self.components.registry.protocol(&node.node_type)?;
        let template = match &source.port {
            PortRef::Declared { key } => key,
            PortRef::Instance { template, .. } => template,
        };
        let spec = protocol
            .interface
            .ports
            .iter()
            .find(|port| &port.key == template)?;
        let mut value_type = spec.value_type.clone();
        if let Some(binding) = document.port_bindings.get(source) {
            match binding {
                DynamicPortBinding::Resolved { last_known, .. } => {
                    if let Some(last_known) = last_known.value_type.as_ref() {
                        value_type = last_known.clone();
                    }
                }
                DynamicPortBinding::Orphan { .. } => return None,
                DynamicPortBinding::UserCreated { .. } => {}
            }
        }
        if is_unresolved(&value_type, &protocol.interface.type_parameters) {
            if let Some(resource) = node.parameters.values().find_map(serde_json::Value::as_str) {
                let resource = GraphResourceId::new(resource.to_owned().into_boxed_str());
                if let Some(variable) = catalog.variable_contract(&resource) {
                    value_type = data_type_to_type_expr(variable.data_type())?;
                } else if source_is_database(node.node_type.as_str(), &resource, catalog) {
                    value_type = concrete_type("tabular.dataframe")?;
                }
            }
        }
        (!is_unresolved(&value_type, &protocol.interface.type_parameters)).then_some(
            PortCandidate {
                direction: spec.direction,
                kind: spec.kind,
                value_type,
                type_parameters: protocol.interface.type_parameters.clone(),
            },
        )
    }

    fn candidate_ports(
        &self,
        _graph_path: &GraphResourcePath,
        node_type: &NodeTypeId,
        resource: Option<&CatalogResourceEntry>,
        catalog: &ResourceCatalogSnapshot,
    ) -> Vec<PortCandidate> {
        let Some(protocol) = self.components.registry.protocol(node_type) else {
            return Vec::new();
        };
        let mut candidates = protocol
            .interface
            .ports
            .iter()
            .filter(|port| match port.instances {
                PortInstances::Declared => true,
                PortInstances::UserCreated { min, .. } => min > 0,
                PortInstances::Derived { .. } => false,
            })
            .map(|port| PortCandidate {
                direction: port.direction,
                kind: port.kind,
                value_type: port.value_type.clone(),
                type_parameters: protocol.interface.type_parameters.clone(),
            })
            .collect::<Vec<_>>();

        let Some(resource) = resource else {
            return candidates;
        };
        match resource.create_args {
            ResourceBoundCreateArgsDto::Variable => {
                if let Some(variable) = catalog
                    .variable_contract(&GraphResourceId::new(resource.resource_path.as_str()))
                {
                    let Some(value_type) = data_type_to_type_expr(variable.data_type()) else {
                        return candidates;
                    };
                    for candidate in &mut candidates {
                        if is_unresolved(&candidate.value_type, &candidate.type_parameters) {
                            candidate.value_type = value_type.clone();
                        }
                    }
                }
            }
            ResourceBoundCreateArgsDto::Database => {
                if node_type.as_str() == "yssbi.dataframe.source.get" {
                    if let Some(value_type) = concrete_type("tabular.dataframe") {
                        for candidate in &mut candidates {
                            if is_unresolved(&candidate.value_type, &candidate.type_parameters) {
                                candidate.value_type = value_type.clone();
                            }
                        }
                    }
                }
            }
            ResourceBoundCreateArgsDto::Function => {
                let Ok(function_path) = GraphResourcePath::new(resource.resource_path.as_str())
                else {
                    return candidates;
                };
                let Some(signature) = catalog.function_signature(&function_path) else {
                    return candidates;
                };
                let arguments = protocol
                    .interface
                    .ports
                    .iter()
                    .find(|port| port.key.as_str() == "arguments");
                if let Some(arguments) = arguments {
                    candidates.extend(signature.parameters().iter().filter_map(|data_type| {
                        Some(PortCandidate {
                            direction: arguments.direction,
                            kind: arguments.kind,
                            value_type: data_type_to_type_expr(data_type)?,
                            type_parameters: Box::new([]),
                        })
                    }));
                }
                if let (Some(results), Some(data_type)) = (
                    protocol
                        .interface
                        .ports
                        .iter()
                        .find(|port| port.key.as_str() == "results"),
                    signature.result(),
                ) {
                    if let Some(value_type) = data_type_to_type_expr(data_type) {
                        candidates.push(PortCandidate {
                            direction: results.direction,
                            kind: results.kind,
                            value_type,
                            type_parameters: Box::new([]),
                        });
                    }
                }
            }
        }
        candidates
    }
}

#[derive(Debug, Error)]
pub(crate) enum GraphRuntimeCatalogError {
    #[error("compatible source port is invalid")]
    SourceInvalid,
}

#[derive(Clone, Debug)]
struct PortCandidate {
    direction: PortDirection,
    kind: PortKind,
    value_type: TypeExpr,
    type_parameters: Box<[crate::node_system::protocol::TypeParameterId]>,
}

fn ports_are_compatible(source: &PortCandidate, candidate: &PortCandidate) -> bool {
    if source.direction == candidate.direction || source.kind != candidate.kind {
        return false;
    }
    if source.kind != PortKind::Data
        || is_unresolved(&source.value_type, &source.type_parameters)
        || is_unresolved(&candidate.value_type, &candidate.type_parameters)
    {
        return source.kind == candidate.kind && source.kind != PortKind::Data;
    }
    let compatibility = match source.direction {
        PortDirection::Output => crate::node_system::compiler::type_exprs_compatibility(
            &source.value_type,
            &candidate.value_type,
            &source.type_parameters,
            &candidate.type_parameters,
        ),
        PortDirection::Input => crate::node_system::compiler::type_exprs_compatibility(
            &candidate.value_type,
            &source.value_type,
            &candidate.type_parameters,
            &source.type_parameters,
        ),
    };
    compatibility != crate::node_system::compiler::TypeCompatibility::Incompatible
}

fn is_unresolved(
    expression: &TypeExpr,
    parameters: &[crate::node_system::protocol::TypeParameterId],
) -> bool {
    let declared = parameters.iter().collect::<std::collections::BTreeSet<_>>();
    match expression {
        TypeExpr::Concrete(_) => false,
        TypeExpr::Generic(id) => !declared.contains(id),
        TypeExpr::Applied { arguments, .. } | TypeExpr::Union(arguments) => arguments
            .iter()
            .any(|argument| is_unresolved(argument, parameters)),
        TypeExpr::Unknown => true,
    }
}

fn data_type_to_type_expr(data_type: &DataType) -> Option<TypeExpr> {
    match data_type {
        DataType::Boolean => concrete_type("core.bool"),
        DataType::Int64 => concrete_type("core.int64"),
        DataType::Float64 => concrete_type("core.float64"),
        DataType::String => concrete_type("core.string"),
        DataType::Date => concrete_type("core.date"),
        DataType::Datetime => concrete_type("core.datetime"),
        DataType::Time => concrete_type("core.time"),
        DataType::Categorical => concrete_type("core.categorical"),
        DataType::Object => concrete_type("core.object"),
        DataType::DataFrame => concrete_type("tabular.dataframe"),
        DataType::Struct(id) => concrete_type(id),
        DataType::Array(element) => applied_type("core.array", element),
        DataType::DataSeries(element) => applied_type("core.data_series", element),
        DataType::OneOf(values) => values
            .iter()
            .map(data_type_to_type_expr)
            .collect::<Option<Vec<_>>>()
            .map(TypeExpr::Union),
        DataType::Any => Some(TypeExpr::Unknown),
    }
}

fn concrete_type(value: &str) -> Option<TypeExpr> {
    TypeId::new(value).ok().map(TypeExpr::Concrete)
}

fn applied_type(constructor: &str, element: &DataType) -> Option<TypeExpr> {
    Some(TypeExpr::Applied {
        constructor: TypeConstructorId::new(constructor).ok()?,
        arguments: vec![data_type_to_type_expr(element)?],
    })
}

fn source_is_database(
    node_type: &str,
    resource: &GraphResourceId,
    catalog: &ResourceCatalogSnapshot,
) -> bool {
    node_type == "yssbi.dataframe.source.get" && catalog.database_schema(resource).is_some()
}

use super::{CompilerDiagnostic, CompilerDiagnosticLocation};
use crate::graph_document::{NodeId, PortAddress, PortRef};
use crate::node_system::analysis::DiagnosticLocation;
use crate::node_system::protocol::{
    ColumnRename, ColumnSelectionExpr, NodeProtocol, ParameterKey, PortKey, RelationalScalarType,
    RenameExpr, ResolvedSchemaFact, SchemaColumnRef, SchemaDependency, SchemaExpr,
    SchemaResolverId,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaResolutionError {
    pub message: Box<str>,
    pub resource: Option<(crate::node_system::analysis::ResourceKey, Box<str>)>,
}

impl SchemaResolutionError {
    pub fn new(message: impl Into<Box<str>>) -> Self {
        Self {
            message: message.into(),
            resource: None,
        }
    }

    pub fn from_resource(error: &crate::node_system::analysis::ResourceResolutionError) -> Self {
        Self {
            message: error.to_string().into(),
            resource: Some((error.key().clone(), error.reason().into())),
        }
    }
}

pub type SchemaFact = ResolvedSchemaFact;

pub struct SchemaResolutionContext<'a, 'resources> {
    pub node_id: NodeId,
    pub parameters: &'a BTreeMap<ParameterKey, serde_json::Value>,
    pub port_dependencies: &'a BTreeMap<PortKey, Option<SchemaFact>>,
    pub interface_dependencies: &'a [crate::node_system::protocol::InterfaceResolverId],
    pub resources:
        Option<&'resources mut dyn crate::node_system::analysis::AnalysisResourceResolver>,
}

pub trait SchemaResolver: Send + Sync {
    fn resolve(
        &self,
        context: &mut SchemaResolutionContext<'_, '_>,
    ) -> Result<SchemaFact, SchemaResolutionError>;
}

/// Compiler-injected resolver capabilities. This is intentionally separate from
/// protocol lookup, so `NodeRegistry` remains the sole protocol/implementation table.
#[derive(Clone, Default)]
pub struct SchemaResolverSet {
    schema: BTreeMap<SchemaResolverId, Arc<dyn SchemaResolver>>,
}

impl SchemaResolverSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        id: SchemaResolverId,
        resolver: impl SchemaResolver + 'static,
    ) -> Option<Arc<dyn SchemaResolver>> {
        self.schema.insert(id, Arc::new(resolver))
    }

    fn get(&self, id: &SchemaResolverId) -> Option<&dyn SchemaResolver> {
        self.schema.get(id).map(Arc::as_ref)
    }
}

pub(crate) struct SchemaAnalysisIssue {
    pub location: CompilerDiagnosticLocation,
    pub diagnostic: CompilerDiagnostic,
}

struct SchemaNode<'a> {
    protocol: &'a NodeProtocol,
    parameters: &'a BTreeMap<ParameterKey, serde_json::Value>,
    ports: Vec<PortAddress>,
}

pub(crate) struct SchemaAnalyzer<'a> {
    resolvers: &'a SchemaResolverSet,
    nodes: BTreeMap<NodeId, SchemaNode<'a>>,
    sources: BTreeMap<PortAddress, PortAddress>,
    facts: BTreeMap<PortAddress, SchemaFact>,
    evaluated: BTreeSet<PortAddress>,
    active: BTreeSet<PortAddress>,
    issues: Vec<SchemaAnalysisIssue>,
}

impl<'a> SchemaAnalyzer<'a> {
    pub fn new(resolvers: &'a SchemaResolverSet) -> Self {
        Self {
            resolvers,
            nodes: BTreeMap::new(),
            sources: BTreeMap::new(),
            facts: BTreeMap::new(),
            evaluated: BTreeSet::new(),
            active: BTreeSet::new(),
            issues: Vec::new(),
        }
    }

    pub fn add_node(
        &mut self,
        node_id: NodeId,
        protocol: &'a NodeProtocol,
        parameters: &'a BTreeMap<ParameterKey, serde_json::Value>,
        ports: impl Iterator<Item = PortAddress>,
    ) {
        self.nodes.insert(
            node_id,
            SchemaNode {
                protocol,
                parameters,
                ports: ports.collect(),
            },
        );
    }

    pub fn add_connection(&mut self, output: PortAddress, input: PortAddress) {
        self.sources.insert(input, output);
    }

    #[cfg(test)]
    pub fn analyze(
        self,
    ) -> (
        BTreeMap<PortAddress, SchemaExpr>,
        BTreeMap<PortAddress, SchemaFact>,
        Vec<SchemaAnalysisIssue>,
    ) {
        self.analyze_internal(None)
    }

    pub fn analyze_with_resources(
        self,
        resources: &mut dyn crate::node_system::analysis::AnalysisResourceResolver,
    ) -> (
        BTreeMap<PortAddress, SchemaExpr>,
        BTreeMap<PortAddress, SchemaFact>,
        Vec<SchemaAnalysisIssue>,
    ) {
        self.analyze_internal(Some(resources))
    }

    fn analyze_internal(
        mut self,
        mut resources: Option<&mut dyn crate::node_system::analysis::AnalysisResourceResolver>,
    ) -> (
        BTreeMap<PortAddress, SchemaExpr>,
        BTreeMap<PortAddress, SchemaFact>,
        Vec<SchemaAnalysisIssue>,
    ) {
        let addresses = self
            .nodes
            .values()
            .flat_map(|node| node.ports.iter().cloned())
            .collect::<Vec<_>>();
        for address in addresses {
            self.evaluate_port(&address, &mut resources);
        }
        let expressions = self
            .facts
            .iter()
            .map(|(address, fact)| (address.clone(), fact.expression.clone()))
            .collect();
        (expressions, self.facts, self.issues)
    }

    fn evaluate_port(
        &mut self,
        address: &PortAddress,
        resources: &mut Option<&mut dyn crate::node_system::analysis::AnalysisResourceResolver>,
    ) -> Option<SchemaFact> {
        if let Some(fact) = self.facts.get(address) {
            return Some(fact.clone());
        }
        if self.evaluated.contains(address) || !self.active.insert(address.clone()) {
            return None;
        }

        let expression = self.port_schema(address).cloned();
        let result = if let Some(expression) = expression {
            self.evaluate_expr(address, address.node_id, &expression, resources)
        } else if let Some(source) = self.sources.get(address).cloned() {
            self.evaluate_port(&source, resources)
        } else {
            None
        };
        self.active.remove(address);
        self.evaluated.insert(address.clone());
        if let Some(fact) = result.clone() {
            self.facts.insert(address.clone(), fact);
        }
        result
    }

    fn evaluate_expr(
        &mut self,
        address: &PortAddress,
        node_id: NodeId,
        expression: &SchemaExpr,
        resources: &mut Option<&mut dyn crate::node_system::analysis::AnalysisResourceResolver>,
    ) -> Option<SchemaFact> {
        match expression {
            SchemaExpr::Input(key) => self
                .port_address(node_id, key)
                .and_then(|address| self.sources.get(&address).cloned().or(Some(address)))
                .and_then(|address| self.evaluate_port(&address, resources)),
            SchemaExpr::Filter { input, predicate } => {
                let input = self.evaluate_expr(address, node_id, input, resources)?;
                match predicate {
                    Some(predicate) => self.filter(node_id, input, predicate),
                    None => Some(SchemaFact::new(
                        SchemaExpr::Filter {
                            input: Box::new(input.expression),
                            predicate: None,
                        },
                        input.fields,
                    )),
                }
            }
            SchemaExpr::Project { input, columns } => {
                let input = self.evaluate_expr(address, node_id, input, resources)?;
                let parameter = match columns {
                    ColumnSelectionExpr::FromParameter(key) => Some(key),
                    _ => None,
                };
                let columns = self.resolve_columns(node_id, columns)?;
                self.project(node_id, input, columns, parameter)
            }
            SchemaExpr::Append { inputs } => {
                let inputs = inputs
                    .iter()
                    .map(|input| self.evaluate_expr(address, node_id, input, resources))
                    .collect::<Option<Vec<_>>>()?;
                let fields = inputs
                    .first()
                    .map(|fact| fact.fields.clone())
                    .unwrap_or_default();
                Some(SchemaFact::new(
                    SchemaExpr::Append {
                        inputs: inputs.into_iter().map(|fact| fact.expression).collect(),
                    },
                    fields,
                ))
            }
            SchemaExpr::Rename { input, mapping } => {
                let input = self.evaluate_expr(address, node_id, input, resources)?;
                let mapping = self.resolve_rename(node_id, mapping)?;
                self.rename(node_id, input, mapping)
            }
            SchemaExpr::Derived {
                resolver,
                dependencies,
            } => self.resolve_derived(address, node_id, resolver, dependencies, resources),
        }
    }

    fn resolve_derived(
        &mut self,
        address: &PortAddress,
        node_id: NodeId,
        resolver_id: &SchemaResolverId,
        dependencies: &[SchemaDependency],
        resources: &mut Option<&mut dyn crate::node_system::analysis::AnalysisResourceResolver>,
    ) -> Option<SchemaFact> {
        let Some(resolver) = self.resolvers.get(resolver_id) else {
            self.issues.push(SchemaAnalysisIssue {
                location: DiagnosticLocation::Port(address.clone()),
                diagnostic: CompilerDiagnostic::SchemaResolverMissing {
                    resolver_id: resolver_id.to_string().into(),
                },
            });
            return None;
        };
        let mut port_dependencies = BTreeMap::new();
        let mut interface_dependencies = Vec::new();
        for dependency in dependencies {
            match dependency {
                SchemaDependency::Port(key) => {
                    let schema = self
                        .port_address(node_id, key)
                        .and_then(|address| self.evaluate_port(&address, resources));
                    port_dependencies.insert(key.clone(), schema);
                }
                SchemaDependency::Parameter(_) => {}
                SchemaDependency::Interface(id) => interface_dependencies.push(id.clone()),
            }
        }
        interface_dependencies.sort();
        let parameters = &self.nodes.get(&node_id)?.parameters;
        let mut context = SchemaResolutionContext {
            node_id,
            parameters,
            port_dependencies: &port_dependencies,
            interface_dependencies: &interface_dependencies,
            resources: resources.take(),
        };
        let result = resolver.resolve(&mut context);
        *resources = context.resources.take();
        match result {
            Ok(schema) => Some(schema),
            Err(error) => {
                let diagnostic = if let Some((resource_key, reason)) = error.resource {
                    CompilerDiagnostic::resource_resolution_failed(resource_key.as_str(), reason)
                } else {
                    CompilerDiagnostic::SchemaResolverFailed {
                        resolver_id: resolver_id.to_string().into(),
                    }
                };
                self.issues.push(SchemaAnalysisIssue {
                    location: DiagnosticLocation::Port(address.clone()),
                    diagnostic,
                });
                None
            }
        }
    }

    fn resolve_columns(
        &mut self,
        node_id: NodeId,
        expression: &ColumnSelectionExpr,
    ) -> Option<ColumnSelectionExpr> {
        match expression {
            ColumnSelectionExpr::All | ColumnSelectionExpr::Explicit(_) => Some(expression.clone()),
            ColumnSelectionExpr::FromParameter(key) => {
                let value = self.nodes.get(&node_id)?.parameters.get(key)?;
                let Some(items) = value.as_array() else {
                    self.invalid_parameter(node_id, key, "expected an array of column names");
                    return None;
                };
                let mut columns = Vec::with_capacity(items.len());
                for item in items {
                    let Some(name) = item.as_str() else {
                        self.invalid_parameter(node_id, key, "expected an array of column names");
                        return None;
                    };
                    columns.push(SchemaColumnRef(name.into()));
                }
                Some(ColumnSelectionExpr::Explicit(columns))
            }
        }
    }

    fn resolve_rename(&mut self, node_id: NodeId, expression: &RenameExpr) -> Option<RenameExpr> {
        match expression {
            RenameExpr::Explicit(_) => Some(expression.clone()),
            RenameExpr::FromParameter(key) => {
                let value = self.nodes.get(&node_id)?.parameters.get(key)?;
                let Some(mapping) = value.as_object() else {
                    self.invalid_parameter(node_id, key, "expected an object of column renames");
                    return None;
                };
                let mut renames = Vec::with_capacity(mapping.len());
                for (from, to) in mapping {
                    let Some(to) = to.as_str() else {
                        self.invalid_parameter(node_id, key, "expected string rename targets");
                        return None;
                    };
                    renames.push(ColumnRename {
                        from: SchemaColumnRef(from.as_str().into()),
                        to: SchemaColumnRef(to.into()),
                    });
                }
                Some(RenameExpr::Explicit(renames))
            }
            RenameExpr::FromParameters { from, to } => {
                let from = self.resolve_rename_name(node_id, from)?;
                let to = self.resolve_rename_name(node_id, to)?;
                Some(RenameExpr::Explicit(vec![ColumnRename { from, to }]))
            }
        }
    }

    fn resolve_rename_name(
        &mut self,
        node_id: NodeId,
        key: &ParameterKey,
    ) -> Option<SchemaColumnRef> {
        let value = self.nodes.get(&node_id)?.parameters.get(key).cloned();
        let Some(value) = value else {
            self.invalid_parameter(node_id, key, "expected a string column name");
            return None;
        };
        let Some(name) = value.as_str() else {
            self.invalid_parameter(node_id, key, "expected a string column name");
            return None;
        };
        if name.is_empty() {
            self.invalid_parameter(node_id, key, "column name must not be empty");
            return None;
        }
        if name.trim() != name {
            self.invalid_parameter(
                node_id,
                key,
                "column name must not have leading or trailing whitespace",
            );
            return None;
        }
        Some(SchemaColumnRef(name.into()))
    }

    fn filter(
        &mut self,
        node_id: NodeId,
        input: SchemaFact,
        predicate_key: &ParameterKey,
    ) -> Option<SchemaFact> {
        let value = self.nodes.get(&node_id)?.parameters.get(predicate_key)?;
        let predicate = match serde_json::from_value::<
            crate::node_system::protocol::dataframe::FilterPredicate,
        >(value.clone())
        {
            Ok(predicate) => predicate,
            Err(_) => {
                let diagnostic = filter_shape_diagnostic(value, predicate_key);
                self.schema_issue_at_parameter(node_id, Some(predicate_key), diagnostic);
                return None;
            }
        };
        let Some(field) = input
            .fields
            .iter()
            .find(|field| field.name.0 == predicate.column)
        else {
            self.schema_issue_at_parameter(
                node_id,
                Some(predicate_key),
                CompilerDiagnostic::RelationalFilterColumnMissing {
                    field_name: predicate.column.into(),
                },
            );
            return None;
        };
        if !filter_operator_supported(field.scalar_type, predicate.operator) {
            self.schema_issue_at_parameter(
                node_id,
                Some(predicate_key),
                CompilerDiagnostic::RelationalFilterOperatorInvalid {
                    field_name: field.name.0.clone(),
                },
            );
            return None;
        }
        if !crate::node_system::protocol::dataframe::filter_comparison_is_compatible(
            field.scalar_type,
            predicate.operator,
            predicate.value.as_ref(),
        ) {
            self.schema_issue_at_parameter(
                node_id,
                Some(predicate_key),
                CompilerDiagnostic::RelationalFilterLiteralType {
                    field_name: field.name.0.clone(),
                },
            );
            return None;
        }
        Some(SchemaFact::new(
            SchemaExpr::Filter {
                input: Box::new(input.expression),
                predicate: Some(predicate_key.clone()),
            },
            input.fields,
        ))
    }

    fn project(
        &mut self,
        node_id: NodeId,
        input: SchemaFact,
        columns: ColumnSelectionExpr,
        parameter: Option<&ParameterKey>,
    ) -> Option<SchemaFact> {
        let ColumnSelectionExpr::Explicit(columns) = columns else {
            return Some(input);
        };
        if columns.is_empty() {
            self.schema_issue_at_parameter(
                node_id,
                parameter,
                CompilerDiagnostic::SchemaProjectEmpty {},
            );
            return None;
        }
        let available = input
            .fields
            .iter()
            .map(|field| (field.name.0.as_ref(), field))
            .collect::<BTreeMap<_, _>>();
        let mut seen = BTreeSet::new();
        let mut valid = true;
        for column in &columns {
            if !available.contains_key(column.0.as_ref()) {
                self.schema_issue_at_parameter(
                    node_id,
                    parameter,
                    CompilerDiagnostic::SchemaProjectFieldMissing {
                        field_name: column.0.clone(),
                    },
                );
                valid = false;
            }
            if !seen.insert(column.0.as_ref()) {
                self.schema_issue_at_parameter(
                    node_id,
                    parameter,
                    CompilerDiagnostic::SchemaProjectFieldDuplicate {
                        field_name: column.0.clone(),
                    },
                );
                valid = false;
            }
        }
        valid.then(|| {
            let fields = columns
                .iter()
                .map(|column| available[column.0.as_ref()].clone())
                .collect::<Vec<_>>();
            SchemaFact::new(
                SchemaExpr::Project {
                    input: Box::new(input.expression),
                    columns: ColumnSelectionExpr::Explicit(columns),
                },
                fields,
            )
        })
    }

    fn rename(
        &mut self,
        node_id: NodeId,
        input: SchemaFact,
        mapping: RenameExpr,
    ) -> Option<SchemaFact> {
        let RenameExpr::Explicit(renames) = mapping else {
            return None;
        };
        let available = input
            .fields
            .iter()
            .map(|field| field.name.0.as_ref())
            .collect::<BTreeSet<_>>();
        let renamed_sources = renames
            .iter()
            .map(|rename| rename.from.0.as_ref())
            .collect::<BTreeSet<_>>();
        let mut seen_sources = BTreeSet::new();
        let mut seen_targets = BTreeSet::new();
        let mut valid = true;
        for rename in &renames {
            let from = rename.from.0.as_ref();
            let to = rename.to.0.as_ref();
            if !available.contains(from) {
                self.schema_issue(
                    node_id,
                    CompilerDiagnostic::SchemaRenameFieldMissing {
                        source_name: from.into(),
                    },
                );
                valid = false;
            }
            if !seen_sources.insert(from) {
                self.schema_issue(
                    node_id,
                    CompilerDiagnostic::SchemaRenameSourceDuplicate {
                        source_name: from.into(),
                    },
                );
                valid = false;
            }
            if !seen_targets.insert(to) || (available.contains(to) && !renamed_sources.contains(to))
            {
                self.schema_issue(
                    node_id,
                    CompilerDiagnostic::SchemaRenameTargetConflict {
                        source_name: from.into(),
                        target_name: to.into(),
                    },
                );
                valid = false;
            }
        }
        if !valid {
            return None;
        }
        if renames.iter().all(|rename| rename.from.0 == rename.to.0) {
            return Some(input);
        }
        let by_source = renames
            .iter()
            .map(|rename| (rename.from.0.clone(), rename.to.clone()))
            .collect::<BTreeMap<_, _>>();
        let fields = input.fields.iter().map(|field| {
            let mut field = field.clone();
            field.name = by_source
                .get(field.name.0.as_ref())
                .cloned()
                .unwrap_or(field.name);
            field
        });
        Some(SchemaFact::new(
            SchemaExpr::Rename {
                input: Box::new(input.expression),
                mapping: RenameExpr::Explicit(renames),
            },
            fields,
        ))
    }

    fn schema_issue(&mut self, node_id: NodeId, diagnostic: CompilerDiagnostic) {
        self.schema_issue_at_parameter(node_id, None, diagnostic);
    }

    fn schema_issue_at_parameter(
        &mut self,
        node_id: NodeId,
        parameter: Option<&ParameterKey>,
        diagnostic: CompilerDiagnostic,
    ) {
        self.issues.push(SchemaAnalysisIssue {
            location: parameter.map_or(DiagnosticLocation::Node(node_id), |key| {
                DiagnosticLocation::Parameter {
                    node_id,
                    key: key.clone(),
                }
            }),
            diagnostic,
        });
    }

    fn invalid_parameter(&mut self, node_id: NodeId, key: &ParameterKey, _reason: &str) {
        self.issues.push(SchemaAnalysisIssue {
            location: DiagnosticLocation::Parameter {
                node_id,
                key: key.clone(),
            },
            diagnostic: CompilerDiagnostic::SchemaParameterInvalid {
                parameter_key: key.to_string().into(),
            },
        });
    }

    fn port_schema(&self, address: &PortAddress) -> Option<&SchemaExpr> {
        let node = self.nodes.get(&address.node_id)?;
        node.protocol
            .interface
            .ports
            .iter()
            .find(|spec| &spec.key == port_template(address))?
            .schema
            .as_ref()
    }

    fn port_address(&self, node_id: NodeId, key: &PortKey) -> Option<PortAddress> {
        self.nodes
            .get(&node_id)?
            .ports
            .iter()
            .find(|address| port_template(address) == key)
            .cloned()
    }
}

fn filter_operator_supported(
    scalar_type: RelationalScalarType,
    operator: crate::node_system::protocol::dataframe::FilterOperator,
) -> bool {
    use crate::node_system::protocol::dataframe::FilterOperator;
    if matches!(scalar_type, RelationalScalarType::Unknown) {
        return false;
    }
    if matches!(operator, FilterOperator::IsNull | FilterOperator::IsNotNull) {
        return true;
    }
    match scalar_type {
        RelationalScalarType::Boolean => {
            matches!(operator, FilterOperator::Equal | FilterOperator::NotEqual)
        }
        RelationalScalarType::Int64
        | RelationalScalarType::Float64
        | RelationalScalarType::String => true,
        RelationalScalarType::Date
        | RelationalScalarType::DateTime
        | RelationalScalarType::Unknown => false,
    }
}

fn filter_shape_diagnostic(
    value: &serde_json::Value,
    parameter_key: &ParameterKey,
) -> CompilerDiagnostic {
    let Some(object) = value.as_object() else {
        return CompilerDiagnostic::RelationalFilterLiteralType {
            field_name: parameter_key.to_string().into(),
        };
    };
    let field_name = object
        .get("column")
        .and_then(serde_json::Value::as_str)
        .filter(|column| !column.is_empty() && column.trim() == *column)
        .unwrap_or(parameter_key.as_str());
    if object
        .get("column")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|column| column.is_empty() || column.trim() != column)
    {
        return CompilerDiagnostic::RelationalFilterColumnMissing {
            field_name: field_name.into(),
        };
    }
    let operator = object.get("operator").and_then(serde_json::Value::as_str);
    match operator {
        Some("isNull" | "isNotNull") if object.contains_key("value") => {
            CompilerDiagnostic::RelationalFilterLiteralForbidden {
                field_name: field_name.into(),
            }
        }
        Some(
            "equal" | "notEqual" | "lessThan" | "lessThanOrEqual" | "greaterThan"
            | "greaterThanOrEqual",
        ) if !object.contains_key("value") => CompilerDiagnostic::RelationalFilterLiteralMissing {
            field_name: field_name.into(),
        },
        Some(
            "equal" | "notEqual" | "lessThan" | "lessThanOrEqual" | "greaterThan"
            | "greaterThanOrEqual" | "isNull" | "isNotNull",
        ) => CompilerDiagnostic::RelationalFilterLiteralType {
            field_name: field_name.into(),
        },
        _ => CompilerDiagnostic::RelationalFilterOperatorInvalid {
            field_name: field_name.into(),
        },
    }
}

fn port_template(address: &PortAddress) -> &PortKey {
    match &address.port {
        PortRef::Declared { key } => key,
        PortRef::Instance { template, .. } => template,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::DiagnosticLocation;
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::protocol::{NodeTypeId, SchemaField, SchemaFieldLineage};

    fn parameter_key(value: &str) -> ParameterKey {
        ParameterKey::new(value).unwrap()
    }

    fn stable_field(name: &str) -> SchemaField {
        SchemaField {
            name: SchemaColumnRef(name.into()),
            scalar_type: RelationalScalarType::String,
            lineage: Some(SchemaFieldLineage {
                source: "databases/main".into(),
                field: name.into(),
            }),
        }
    }

    fn two_parameter_mapping() -> RenameExpr {
        RenameExpr::FromParameters {
            from: parameter_key("from"),
            to: parameter_key("to"),
        }
    }

    fn resolve_builtin_rename(
        from: serde_json::Value,
        to: serde_json::Value,
    ) -> (Option<RenameExpr>, Vec<SchemaAnalysisIssue>) {
        let registry =
            std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
        let rename = registry
            .get(&NodeTypeId::new("yssbi.dataframe.rename").unwrap())
            .unwrap();
        let parameters = BTreeMap::from([(parameter_key("from"), from), (parameter_key("to"), to)]);
        let resolvers = SchemaResolverSet::new();
        let node_id = NodeId::new();
        let mut analyzer = SchemaAnalyzer::new(&resolvers);
        analyzer.add_node(node_id, rename.protocol(), &parameters, std::iter::empty());
        let mapping = analyzer.resolve_rename(node_id, &two_parameter_mapping());
        (mapping, analyzer.issues)
    }

    #[test]
    fn rename_dataframe_resolves_two_scalars_and_preserves_field_order() {
        let (mapping, issues) =
            resolve_builtin_rename(serde_json::json!("a"), serde_json::json!("renamed"));
        assert!(issues.is_empty());
        let mapping = mapping.expect("valid scalar parameters resolve");
        assert_eq!(
            mapping,
            RenameExpr::Explicit(vec![ColumnRename {
                from: SchemaColumnRef("a".into()),
                to: SchemaColumnRef("renamed".into()),
            }])
        );

        let resolvers = SchemaResolverSet::new();
        let mut analyzer = SchemaAnalyzer::new(&resolvers);
        let input_expression = SchemaExpr::Input(PortKey::new("raw").unwrap());
        let renamed = analyzer
            .rename(
                NodeId::new(),
                SchemaFact::new(
                    input_expression.clone(),
                    [SchemaColumnRef("a".into()), SchemaColumnRef("b".into())],
                ),
                mapping,
            )
            .expect("valid rename fact");
        assert_eq!(
            renamed.fields,
            vec![
                SchemaField {
                    name: SchemaColumnRef("renamed".into()),
                    scalar_type: RelationalScalarType::Unknown,
                    lineage: None,
                },
                SchemaField {
                    name: SchemaColumnRef("b".into()),
                    scalar_type: RelationalScalarType::Unknown,
                    lineage: None,
                },
            ]
        );
        assert!(matches!(
            renamed.expression,
            SchemaExpr::Rename { input, .. } if *input == input_expression
        ));
    }

    #[test]
    fn rename_dataframe_rejects_invalid_blank_and_padded_scalars_at_parameter() {
        for (from, to, key) in [
            (serde_json::json!(1), serde_json::json!("renamed"), "from"),
            (serde_json::json!("a"), serde_json::json!(false), "to"),
            (serde_json::json!(""), serde_json::json!("renamed"), "from"),
            (serde_json::json!("a"), serde_json::json!(""), "to"),
            (
                serde_json::json!(" a"),
                serde_json::json!("renamed"),
                "from",
            ),
            (serde_json::json!("a"), serde_json::json!("renamed "), "to"),
        ] {
            let (mapping, issues) = resolve_builtin_rename(from, to);
            assert!(mapping.is_none());
            assert_eq!(issues.len(), 1);
            assert_eq!(
                issues[0].diagnostic.definition().code,
                "compiler.schema.parameter_invalid"
            );
            assert!(matches!(
                &issues[0].location,
                DiagnosticLocation::Parameter { key: actual, .. } if actual.as_str() == key
            ));
        }
    }

    #[test]
    fn rename_dataframe_preserves_existing_object_parameter_mapping() {
        let registry =
            std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
        let rename = registry
            .get(&NodeTypeId::new("yssbi.dataframe.rename").unwrap())
            .unwrap();
        let mapping_key = parameter_key("mapping");
        let parameters =
            BTreeMap::from([(mapping_key.clone(), serde_json::json!({"a": "renamed"}))]);
        let resolvers = SchemaResolverSet::new();
        let node_id = NodeId::new();
        let mut analyzer = SchemaAnalyzer::new(&resolvers);
        analyzer.add_node(node_id, rename.protocol(), &parameters, std::iter::empty());

        let mapping = analyzer
            .resolve_rename(node_id, &RenameExpr::FromParameter(mapping_key))
            .expect("existing object mapping resolves");

        assert_eq!(
            mapping,
            RenameExpr::Explicit(vec![ColumnRename {
                from: SchemaColumnRef("a".into()),
                to: SchemaColumnRef("renamed".into()),
            }])
        );
        assert!(analyzer.issues.is_empty());
    }

    #[test]
    fn project_filter_and_rename_preserve_field_lineage() {
        let fields = vec![stable_field("customer_id"), stable_field("region")];
        let input = SchemaFact::new(
            SchemaExpr::Input(PortKey::new("raw").unwrap()),
            fields.clone(),
        );
        let resolvers = SchemaResolverSet::new();
        let mut analyzer = SchemaAnalyzer::new(&resolvers);

        let projected = analyzer
            .project(
                NodeId::new(),
                input.clone(),
                ColumnSelectionExpr::Explicit(vec![SchemaColumnRef("customer_id".into())]),
                None,
            )
            .unwrap();
        assert_eq!(projected.fields, vec![stable_field("customer_id")]);

        let (filtered, issues) = filter_with(
            input.clone(),
            serde_json::json!({
                "column": "region",
                "operator": "equal",
                "value": {"type": "string", "value": "west"}
            }),
        );
        assert!(issues.is_empty());
        assert_eq!(filtered.unwrap().fields, fields);

        let renamed = analyzer
            .rename(
                NodeId::new(),
                input,
                RenameExpr::Explicit(vec![ColumnRename {
                    from: SchemaColumnRef("customer_id".into()),
                    to: SchemaColumnRef("account_id".into()),
                }]),
            )
            .unwrap();
        assert_eq!(renamed.fields[0].name, SchemaColumnRef("account_id".into()));
        assert_eq!(
            renamed.fields[0].lineage,
            stable_field("customer_id").lineage
        );
        assert_eq!(renamed.fields[1], stable_field("region"));
    }

    #[test]
    fn project_and_rename_preserve_resolved_scalar_types() {
        let resolvers = SchemaResolverSet::new();
        let mut analyzer = SchemaAnalyzer::new(&resolvers);
        let input = SchemaFact::new(
            SchemaExpr::Input(PortKey::new("raw").unwrap()),
            [
                SchemaField {
                    name: SchemaColumnRef("amount".into()),
                    scalar_type: RelationalScalarType::Float64,
                    lineage: None,
                },
                SchemaField {
                    name: SchemaColumnRef("status".into()),
                    scalar_type: RelationalScalarType::String,
                    lineage: None,
                },
            ],
        );

        let projected = analyzer
            .project(
                NodeId::new(),
                input,
                ColumnSelectionExpr::Explicit(vec![SchemaColumnRef("amount".into())]),
                None,
            )
            .unwrap();
        let renamed = analyzer
            .rename(
                NodeId::new(),
                projected,
                RenameExpr::Explicit(vec![ColumnRename {
                    from: SchemaColumnRef("amount".into()),
                    to: SchemaColumnRef("total".into()),
                }]),
            )
            .unwrap();

        assert_eq!(
            renamed.fields,
            vec![SchemaField {
                name: SchemaColumnRef("total".into()),
                scalar_type: RelationalScalarType::Float64,
                lineage: None,
            }]
        );
    }

    fn filter_with(
        input: SchemaFact,
        predicate: serde_json::Value,
    ) -> (Option<SchemaFact>, Vec<SchemaAnalysisIssue>) {
        let registry =
            std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
        let protocol = registry
            .protocol(&NodeTypeId::new("yssbi.dataframe.rename").unwrap())
            .unwrap();
        let key = parameter_key("predicate");
        let parameters = BTreeMap::from([(key.clone(), predicate)]);
        let resolvers = SchemaResolverSet::new();
        let node_id = NodeId::new();
        let mut analyzer = SchemaAnalyzer::new(&resolvers);
        analyzer.add_node(node_id, protocol, &parameters, std::iter::empty());
        let fact = analyzer.filter(node_id, input, &key);
        (fact, analyzer.issues)
    }

    fn typed_input() -> SchemaFact {
        SchemaFact::new(
            SchemaExpr::Input(PortKey::new("raw").unwrap()),
            [
                SchemaField {
                    name: SchemaColumnRef("total".into()),
                    scalar_type: RelationalScalarType::Float64,
                    lineage: None,
                },
                SchemaField {
                    name: SchemaColumnRef("active".into()),
                    scalar_type: RelationalScalarType::Boolean,
                    lineage: None,
                },
            ],
        )
    }

    #[test]
    fn filter_diagnostics_use_exact_predicate_parameter_and_renamed_fields() {
        for (predicate, code) in [
            (
                serde_json::json!({"column":"amount","operator":"equal","value":{"type":"decimal","value":"1.5"}}),
                "compiler.relational.filter_column_missing",
            ),
            (
                serde_json::json!({"column":"active","operator":"lessThan","value":{"type":"boolean","value":true}}),
                "compiler.relational.filter_operator_invalid",
            ),
            (
                serde_json::json!({"column":"total","operator":"equal","value":{"type":"string","value":"x"}}),
                "compiler.relational.filter_literal_type",
            ),
            (
                serde_json::json!({"column":"total","operator":"equal"}),
                "compiler.relational.filter_literal_missing",
            ),
            (
                serde_json::json!({"column":"total","operator":"isNull","value":{"type":"decimal","value":"1.5"}}),
                "compiler.relational.filter_literal_forbidden",
            ),
            (
                serde_json::json!({"column":"","operator":"isNull"}),
                "compiler.relational.filter_column_missing",
            ),
        ] {
            let (fact, issues) = filter_with(typed_input(), predicate);
            assert!(fact.is_none());
            assert_eq!(issues.len(), 1);
            assert_eq!(issues[0].diagnostic.definition().code, code);
            assert!(matches!(
                &issues[0].location,
                DiagnosticLocation::Parameter { key, .. } if key.as_str() == "predicate"
            ));
        }

        let (fact, issues) = filter_with(
            typed_input(),
            serde_json::json!({"column":"total","operator":"greaterThan","value":{"type":"decimal","value":"1.5"}}),
        );
        assert!(issues.is_empty());
        assert_eq!(fact.unwrap().fields[0].name.0.as_ref(), "total");
    }

    #[test]
    fn project_diagnostics_use_exact_columns_parameter() {
        let resolvers = SchemaResolverSet::new();
        let mut analyzer = SchemaAnalyzer::new(&resolvers);
        let key = parameter_key("columns");

        assert!(
            analyzer
                .project(
                    NodeId::new(),
                    typed_input(),
                    ColumnSelectionExpr::Explicit(vec![SchemaColumnRef("missing".into())]),
                    Some(&key),
                )
                .is_none()
        );
        assert!(matches!(
            &analyzer.issues[0].location,
            DiagnosticLocation::Parameter { key, .. } if key.as_str() == "columns"
        ));
    }

    #[test]
    fn connected_filter_with_unavailable_source_schema_emits_one_port_dependency_issue() {
        let registry =
            std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
        let source_protocol = registry
            .protocol(&NodeTypeId::new("yssbi.dataframe.source.get").unwrap())
            .unwrap()
            .clone();
        let mut filter_protocol = registry
            .protocol(&NodeTypeId::new("yssbi.dataframe.rename").unwrap())
            .unwrap()
            .clone();
        filter_protocol.interface.ports[1].schema = Some(SchemaExpr::Filter {
            input: Box::new(SchemaExpr::Input(PortKey::new("source").unwrap())),
            predicate: Some(parameter_key("predicate")),
        });
        let source_parameters = BTreeMap::from([(
            parameter_key("dataframe"),
            serde_json::json!("databases/missing"),
        )]);
        let filter_parameters = BTreeMap::from([(
            parameter_key("predicate"),
            serde_json::json!({"column":"missing","operator":"equal","value":{"type":"string","value":"x"}}),
        )]);
        let source_node = NodeId::new();
        let filter_node = NodeId::new();
        let source_output = PortAddress::declared(source_node, PortKey::new("dataframe").unwrap());
        let filter_source = PortAddress::declared(filter_node, PortKey::new("source").unwrap());
        let filter_result = PortAddress::declared(filter_node, PortKey::new("result").unwrap());
        let resolvers = SchemaResolverSet::new();
        let mut analyzer = SchemaAnalyzer::new(&resolvers);
        analyzer.add_node(
            source_node,
            &source_protocol,
            &source_parameters,
            [source_output.clone()].into_iter(),
        );
        analyzer.add_node(
            filter_node,
            &filter_protocol,
            &filter_parameters,
            [filter_source.clone(), filter_result].into_iter(),
        );
        analyzer.add_connection(source_output.clone(), filter_source);

        let (_, _, issues) = analyzer.analyze();

        assert_eq!(issues.len(), 1);
        assert_eq!(
            issues[0].diagnostic.definition().code,
            "compiler.schema.resolver_missing"
        );
        assert_eq!(issues[0].location, DiagnosticLocation::Port(source_output));
        assert!(!issues.iter().any(|issue| {
            issue
                .diagnostic
                .definition()
                .code
                .starts_with("compiler.relational.filter_")
        }));
    }

    struct FailingConnectedSourceResolver;

    impl SchemaResolver for FailingConnectedSourceResolver {
        fn resolve(
            &self,
            _: &mut SchemaResolutionContext<'_, '_>,
        ) -> Result<SchemaFact, SchemaResolutionError> {
            Err(SchemaResolutionError::new("source schema unavailable"))
        }
    }

    #[test]
    fn connected_filter_with_failed_source_schema_emits_one_port_dependency_issue() {
        let registry =
            std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
        let source_protocol = registry
            .protocol(&NodeTypeId::new("yssbi.dataframe.source.get").unwrap())
            .unwrap()
            .clone();
        let mut filter_protocol = registry
            .protocol(&NodeTypeId::new("yssbi.dataframe.rename").unwrap())
            .unwrap()
            .clone();
        filter_protocol.interface.ports[1].schema = Some(SchemaExpr::Filter {
            input: Box::new(SchemaExpr::Input(PortKey::new("source").unwrap())),
            predicate: Some(parameter_key("predicate")),
        });
        let source_parameters = BTreeMap::from([(
            parameter_key("dataframe"),
            serde_json::json!("databases/missing"),
        )]);
        let filter_parameters = BTreeMap::from([(
            parameter_key("predicate"),
            serde_json::json!({"column":"missing","operator":"equal","value":{"type":"string","value":"x"}}),
        )]);
        let source_node = NodeId::new();
        let filter_node = NodeId::new();
        let source_output = PortAddress::declared(source_node, PortKey::new("dataframe").unwrap());
        let filter_source = PortAddress::declared(filter_node, PortKey::new("source").unwrap());
        let filter_result = PortAddress::declared(filter_node, PortKey::new("result").unwrap());
        let mut resolvers = SchemaResolverSet::new();
        resolvers.insert(
            SchemaResolverId::new(crate::node_system::catalog::DATAFRAME_RESOURCE_SCHEMA_RESOLVER)
                .unwrap(),
            FailingConnectedSourceResolver,
        );
        let mut analyzer = SchemaAnalyzer::new(&resolvers);
        analyzer.add_node(
            source_node,
            &source_protocol,
            &source_parameters,
            [source_output.clone()].into_iter(),
        );
        analyzer.add_node(
            filter_node,
            &filter_protocol,
            &filter_parameters,
            [filter_source.clone(), filter_result].into_iter(),
        );
        analyzer.add_connection(source_output.clone(), filter_source);

        let (_, _, issues) = analyzer.analyze();

        assert_eq!(issues.len(), 1);
        assert_eq!(
            issues[0].diagnostic.definition().code,
            "compiler.schema.resolver_failed"
        );
        assert_eq!(issues[0].location, DiagnosticLocation::Port(source_output));
        assert!(!issues.iter().any(|issue| {
            issue
                .diagnostic
                .definition()
                .code
                .starts_with("compiler.relational.filter_")
        }));
    }

    #[test]
    fn rename_dataframe_same_name_returns_unchanged_schema_fact() {
        let resolvers = SchemaResolverSet::new();
        let mut analyzer = SchemaAnalyzer::new(&resolvers);
        let input = SchemaFact::new(
            SchemaExpr::Input(PortKey::new("raw").unwrap()),
            [SchemaColumnRef("a".into()), SchemaColumnRef("b".into())],
        );

        let renamed = analyzer
            .rename(
                NodeId::new(),
                input.clone(),
                RenameExpr::Explicit(vec![ColumnRename {
                    from: SchemaColumnRef("a".into()),
                    to: SchemaColumnRef("a".into()),
                }]),
            )
            .expect("same-name rename is valid");

        assert_eq!(renamed, input);
        assert!(analyzer.issues.is_empty());
    }
}

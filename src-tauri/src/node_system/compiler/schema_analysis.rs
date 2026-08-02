use crate::node_system::analysis::DiagnosticLocation;
use crate::node_system::document::{ConnectionId, NodeId, PortAddress, PortRef};
use crate::node_system::protocol::{
    ColumnRename, ColumnSelectionExpr, NodeProtocol, ParameterKey, PortKey, RenameExpr,
    SchemaColumnRef, SchemaDependency, SchemaExpr, SchemaResolverId,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaResolutionError {
    pub message: Box<str>,
}

impl SchemaResolutionError {
    pub fn new(message: impl Into<Box<str>>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaFact {
    pub expression: SchemaExpr,
    pub fields: Vec<SchemaColumnRef>,
}

impl SchemaFact {
    pub fn new(expression: SchemaExpr, fields: impl IntoIterator<Item = SchemaColumnRef>) -> Self {
        Self {
            expression,
            fields: fields.into_iter().collect(),
        }
    }
}

pub struct SchemaResolutionContext<'a> {
    pub node_id: NodeId,
    pub parameters: &'a BTreeMap<ParameterKey, serde_json::Value>,
    pub port_dependencies: &'a BTreeMap<PortKey, Option<SchemaFact>>,
    pub interface_dependencies: &'a [crate::node_system::protocol::InterfaceResolverId],
}

pub trait SchemaResolver: Send + Sync {
    fn resolve(
        &self,
        context: &SchemaResolutionContext<'_>,
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
    pub code: &'static str,
    pub location: DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>,
    pub detail: String,
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

    pub fn analyze(mut self) -> (BTreeMap<PortAddress, SchemaExpr>, Vec<SchemaAnalysisIssue>) {
        let addresses = self
            .nodes
            .values()
            .flat_map(|node| node.ports.iter().cloned())
            .collect::<Vec<_>>();
        for address in addresses {
            self.evaluate_port(&address);
        }
        let facts = self
            .facts
            .into_iter()
            .map(|(address, fact)| (address, fact.expression))
            .collect();
        (facts, self.issues)
    }

    fn evaluate_port(&mut self, address: &PortAddress) -> Option<SchemaFact> {
        if let Some(fact) = self.facts.get(address) {
            return Some(fact.clone());
        }
        if !self.active.insert(address.clone()) {
            return None;
        }

        let expression = self.port_schema(address).cloned();
        let result = if let Some(expression) = expression {
            self.evaluate_expr(address.node_id, &expression)
        } else if let Some(source) = self.sources.get(address).cloned() {
            self.evaluate_port(&source)
        } else {
            None
        };
        self.active.remove(address);
        if let Some(fact) = result.clone() {
            self.facts.insert(address.clone(), fact);
        }
        result
    }

    fn evaluate_expr(&mut self, node_id: NodeId, expression: &SchemaExpr) -> Option<SchemaFact> {
        match expression {
            SchemaExpr::Input(key) => self
                .port_address(node_id, key)
                .and_then(|address| self.sources.get(&address).cloned().or(Some(address)))
                .and_then(|address| self.evaluate_port(&address)),
            SchemaExpr::Filter { input } => self.evaluate_expr(node_id, input),
            SchemaExpr::Project { input, columns } => {
                let input = self.evaluate_expr(node_id, input)?;
                let columns = self.resolve_columns(node_id, columns)?;
                self.project(node_id, input, columns)
            }
            SchemaExpr::Append { inputs } => {
                let inputs = inputs
                    .iter()
                    .map(|input| self.evaluate_expr(node_id, input))
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
                let input = self.evaluate_expr(node_id, input)?;
                let mapping = self.resolve_rename(node_id, mapping)?;
                self.rename(node_id, input, mapping)
            }
            SchemaExpr::Derived {
                resolver,
                dependencies,
            } => self.resolve_derived(node_id, resolver, dependencies),
        }
    }

    fn resolve_derived(
        &mut self,
        node_id: NodeId,
        resolver_id: &SchemaResolverId,
        dependencies: &[SchemaDependency],
    ) -> Option<SchemaFact> {
        let Some(resolver) = self.resolvers.get(resolver_id) else {
            self.issues.push(SchemaAnalysisIssue {
                code: "compiler.schema.resolver_missing",
                location: DiagnosticLocation::Node(node_id),
                detail: resolver_id.to_string(),
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
                        .and_then(|address| self.evaluate_port(&address));
                    port_dependencies.insert(key.clone(), schema);
                }
                SchemaDependency::Parameter(_) => {}
                SchemaDependency::Interface(id) => interface_dependencies.push(id.clone()),
            }
        }
        interface_dependencies.sort();
        let parameters = &self.nodes.get(&node_id)?.parameters;
        let context = SchemaResolutionContext {
            node_id,
            parameters,
            port_dependencies: &port_dependencies,
            interface_dependencies: &interface_dependencies,
        };
        match resolver.resolve(&context) {
            Ok(schema) => Some(schema),
            Err(error) => {
                self.issues.push(SchemaAnalysisIssue {
                    code: "compiler.schema.resolver_failed",
                    location: DiagnosticLocation::Node(node_id),
                    detail: error.message.into_string(),
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

    fn project(
        &mut self,
        node_id: NodeId,
        input: SchemaFact,
        columns: ColumnSelectionExpr,
    ) -> Option<SchemaFact> {
        let ColumnSelectionExpr::Explicit(columns) = columns else {
            return Some(input);
        };
        let available = input
            .fields
            .iter()
            .map(|field| field.0.as_ref())
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        let mut valid = true;
        for column in &columns {
            if !available.contains(column.0.as_ref()) {
                self.schema_issue(
                    node_id,
                    "compiler.schema.project_field_missing",
                    column.0.to_string(),
                );
                valid = false;
            }
            if !seen.insert(column.0.as_ref()) {
                self.schema_issue(
                    node_id,
                    "compiler.schema.project_field_duplicate",
                    column.0.to_string(),
                );
                valid = false;
            }
        }
        valid.then(|| {
            SchemaFact::new(
                SchemaExpr::Project {
                    input: Box::new(input.expression),
                    columns: ColumnSelectionExpr::Explicit(columns.clone()),
                },
                columns,
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
            .map(|field| field.0.as_ref())
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
                    "compiler.schema.rename_field_missing",
                    from.to_owned(),
                );
                valid = false;
            }
            if !seen_sources.insert(from) {
                self.schema_issue(
                    node_id,
                    "compiler.schema.rename_source_duplicate",
                    from.to_owned(),
                );
                valid = false;
            }
            if !seen_targets.insert(to) || (available.contains(to) && !renamed_sources.contains(to))
            {
                self.schema_issue(
                    node_id,
                    "compiler.schema.rename_target_conflict",
                    to.to_owned(),
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
            by_source
                .get(field.0.as_ref())
                .cloned()
                .unwrap_or_else(|| field.clone())
        });
        Some(SchemaFact::new(
            SchemaExpr::Rename {
                input: Box::new(input.expression),
                mapping: RenameExpr::Explicit(renames),
            },
            fields,
        ))
    }

    fn schema_issue(&mut self, node_id: NodeId, code: &'static str, detail: String) {
        self.issues.push(SchemaAnalysisIssue {
            code,
            location: DiagnosticLocation::Node(node_id),
            detail,
        });
    }

    fn invalid_parameter(&mut self, node_id: NodeId, key: &ParameterKey, detail: &str) {
        self.issues.push(SchemaAnalysisIssue {
            code: "compiler.schema.parameter_invalid",
            location: DiagnosticLocation::Parameter {
                node_id,
                key: key.clone(),
            },
            detail: detail.into(),
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
    use crate::node_system::catalog::build_builtin_registry;
    use crate::node_system::protocol::NodeTypeId;

    fn parameter_key(value: &str) -> ParameterKey {
        ParameterKey::new(value).unwrap()
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
        let registry = build_builtin_registry();
        let rename = registry
            .get(&NodeTypeId::new("yssbi.dataframe.rename").unwrap())
            .unwrap();
        let parameters = BTreeMap::from([(parameter_key("from"), from), (parameter_key("to"), to)]);
        let resolvers = SchemaResolverSet::new();
        let node_id = NodeId::new();
        let mut analyzer = SchemaAnalyzer::new(&resolvers);
        analyzer.add_node(node_id, &rename.protocol, &parameters, std::iter::empty());
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
                SchemaColumnRef("renamed".into()),
                SchemaColumnRef("b".into())
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
            assert_eq!(issues[0].code, "compiler.schema.parameter_invalid");
            assert!(matches!(
                &issues[0].location,
                DiagnosticLocation::Parameter { key: actual, .. } if actual.as_str() == key
            ));
        }
    }

    #[test]
    fn rename_dataframe_preserves_existing_object_parameter_mapping() {
        let registry = build_builtin_registry();
        let rename = registry
            .get(&NodeTypeId::new("yssbi.dataframe.rename").unwrap())
            .unwrap();
        let mapping_key = parameter_key("mapping");
        let parameters =
            BTreeMap::from([(mapping_key.clone(), serde_json::json!({"a": "renamed"}))]);
        let resolvers = SchemaResolverSet::new();
        let node_id = NodeId::new();
        let mut analyzer = SchemaAnalyzer::new(&resolvers);
        analyzer.add_node(node_id, &rename.protocol, &parameters, std::iter::empty());

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

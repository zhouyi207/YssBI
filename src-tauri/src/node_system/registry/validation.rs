use super::model::*;
use crate::node_system::protocol::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryValidationError {
    DuplicateProvider(ProviderId),
    DuplicateNode(NodeTypeId),
    DuplicateType(TypeId),
    DuplicateTypeConstructor(TypeConstructorId),
    DuplicateTypeClass(TypeClassId),
    DuplicateCategory(NodeCategoryId),
    DuplicateI18nKey(I18nKey),
    DuplicateInterfaceResolver(InterfaceResolverId),
    DuplicateSchemaResolver(SchemaResolverId),
    InvalidNode {
        node: NodeTypeId,
        reason: String,
    },
    InvalidType {
        id: TypeId,
        reason: String,
    },
    InvalidTypeConstructor {
        id: TypeConstructorId,
        reason: String,
    },
    InvalidCategory {
        id: NodeCategoryId,
        reason: String,
    },
}

impl std::fmt::Display for RegistryValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use RegistryValidationError::*;
        match self {
            DuplicateProvider(id) => write!(f, "provider '{id}' is already registered"),
            DuplicateNode(id) => write!(f, "node type '{id}' is already registered"),
            DuplicateType(id) => write!(f, "type '{id}' is already registered"),
            DuplicateTypeConstructor(id) => {
                write!(f, "type constructor '{id}' is already registered")
            }
            DuplicateTypeClass(id) => write!(f, "type class '{id}' is already registered"),
            DuplicateCategory(id) => write!(f, "category '{id}' is already registered"),
            DuplicateI18nKey(id) => write!(f, "i18n key '{id}' is declared by multiple providers"),
            DuplicateInterfaceResolver(id) => {
                write!(f, "interface resolver '{id}' is already registered")
            }
            DuplicateSchemaResolver(id) => {
                write!(f, "schema resolver '{id}' is already registered")
            }
            InvalidNode { node, reason } => write!(f, "invalid node '{node}': {reason}"),
            InvalidType { id, reason } => write!(f, "invalid type '{id}': {reason}"),
            InvalidTypeConstructor { id, reason } => {
                write!(f, "invalid type constructor '{id}': {reason}")
            }
            InvalidCategory { id, reason } => write!(f, "invalid category '{id}': {reason}"),
        }
    }
}
impl std::error::Error for RegistryValidationError {}

pub(crate) struct ValidatedParts {
    pub nodes: BTreeMap<NodeTypeId, std::sync::Arc<RegisteredNode>>,
    pub types: TypeRegistry,
    pub categories: CategoryRegistry,
    pub i18n: I18nManifest,
}

pub(crate) fn validate(
    providers: &[ProviderRegistration],
) -> Result<ValidatedParts, RegistryValidationError> {
    let mut provider_ids = BTreeSet::new();
    let mut nodes = BTreeMap::new();
    let mut types = TypeRegistry::default();
    let mut categories = CategoryRegistry::default();
    let mut i18n = I18nManifest::default();
    let mut interface_resolvers = BTreeSet::new();
    let mut schema_resolvers = BTreeSet::new();

    for provider in providers {
        if !provider_ids.insert(provider.provider.clone()) {
            return Err(RegistryValidationError::DuplicateProvider(
                provider.provider.clone(),
            ));
        }
        for item in &provider.types {
            if types.types.insert(item.id.clone(), item.clone()).is_some() {
                return Err(RegistryValidationError::DuplicateType(item.id.clone()));
            }
        }
        for item in &provider.type_constructors {
            if item.arity == 0 {
                return Err(RegistryValidationError::InvalidTypeConstructor {
                    id: item.id.clone(),
                    reason: "arity must be positive".into(),
                });
            }
            if types
                .constructors
                .insert(item.id.clone(), item.clone())
                .is_some()
            {
                return Err(RegistryValidationError::DuplicateTypeConstructor(
                    item.id.clone(),
                ));
            }
        }
        for id in &provider.type_classes {
            if !types.classes.insert(id.clone()) {
                return Err(RegistryValidationError::DuplicateTypeClass(id.clone()));
            }
        }
        for item in &provider.categories {
            if categories
                .categories
                .insert(item.id.clone(), item.clone())
                .is_some()
            {
                return Err(RegistryValidationError::DuplicateCategory(item.id.clone()));
            }
        }
        for key in &provider.i18n.keys {
            if !i18n.keys.insert(key.clone()) {
                return Err(RegistryValidationError::DuplicateI18nKey(key.clone()));
            }
        }
        for id in &provider.interface_resolvers {
            if !interface_resolvers.insert(id.clone()) {
                return Err(RegistryValidationError::DuplicateInterfaceResolver(
                    id.clone(),
                ));
            }
        }
        for id in &provider.schema_resolvers {
            if !schema_resolvers.insert(id.clone()) {
                return Err(RegistryValidationError::DuplicateSchemaResolver(id.clone()));
            }
        }
        for node in &provider.nodes {
            let id = node.protocol.type_id.clone();
            if nodes
                .insert(id.clone(), std::sync::Arc::new(node.clone()))
                .is_some()
            {
                return Err(RegistryValidationError::DuplicateNode(id));
            }
        }
    }

    for item in types.types.values() {
        require_i18n(&i18n, &item.title_key).map_err(|reason| {
            RegistryValidationError::InvalidType {
                id: item.id.clone(),
                reason,
            }
        })?;
        for class in &item.classes {
            if !types.classes.contains(class) {
                return Err(RegistryValidationError::InvalidType {
                    id: item.id.clone(),
                    reason: format!("unknown type class '{class}'"),
                });
            }
        }
    }
    for item in types.constructors.values() {
        require_i18n(&i18n, &item.title_key).map_err(|reason| {
            RegistryValidationError::InvalidTypeConstructor {
                id: item.id.clone(),
                reason,
            }
        })?;
    }
    validate_categories(&categories, &i18n)?;
    for node in nodes.values() {
        validate_node(
            node,
            &types,
            &categories,
            &i18n,
            &interface_resolvers,
            &schema_resolvers,
        )?;
    }
    Ok(ValidatedParts {
        nodes,
        types,
        categories,
        i18n,
    })
}

fn validate_categories(
    categories: &CategoryRegistry,
    i18n: &I18nManifest,
) -> Result<(), RegistryValidationError> {
    for category in categories.categories.values() {
        require_i18n(i18n, &category.title_key).map_err(|reason| {
            RegistryValidationError::InvalidCategory {
                id: category.id.clone(),
                reason,
            }
        })?;
        let mut seen = BTreeSet::from([category.id.clone()]);
        let mut parent = category.parent.as_ref();
        while let Some(id) = parent {
            let Some(found) = categories.categories.get(id) else {
                return Err(RegistryValidationError::InvalidCategory {
                    id: category.id.clone(),
                    reason: format!("unknown parent '{id}'"),
                });
            };
            if !seen.insert(id.clone()) {
                return Err(RegistryValidationError::InvalidCategory {
                    id: category.id.clone(),
                    reason: "category parent cycle".into(),
                });
            }
            parent = found.parent.as_ref();
        }
    }
    Ok(())
}

fn validate_node(
    node: &RegisteredNode,
    types: &TypeRegistry,
    categories: &CategoryRegistry,
    i18n: &I18nManifest,
    interface_resolvers: &BTreeSet<InterfaceResolverId>,
    schema_resolvers: &BTreeSet<SchemaResolverId>,
) -> Result<(), RegistryValidationError> {
    let protocol = &node.protocol;
    let fail = |reason: String| RegistryValidationError::InvalidNode {
        node: protocol.type_id.clone(),
        reason,
    };
    match (&node.implementation, node.structural_role) {
        (Some(implementation), None) => {
            if implementation.capability() != ImplementationKind::CompilerLowering {
                return Err(fail(
                    "leaf implementation does not provide lowerer capability".into(),
                ));
            }
        }
        (None, Some(_)) => {}
        (None, None) => return Err(fail("leaf node has no implementation".into())),
        (Some(_), Some(_)) => {
            return Err(fail(
                "leaf implementation and structural role are mutually exclusive".into(),
            ));
        }
    }
    match (protocol.managed_role, protocol.scope, node.structural_role) {
        (
            Some(ManagedNodeRole::EventBegin),
            NodeScope::Event,
            Some(StructuralNodeRole::EventBegin),
        )
        | (
            Some(ManagedNodeRole::FunctionEntry),
            NodeScope::Function,
            Some(StructuralNodeRole::FunctionEntry),
        )
        | (
            Some(ManagedNodeRole::FunctionReturn),
            NodeScope::Function,
            Some(StructuralNodeRole::FunctionReturn),
        )
        | (None, _, _) => {}
        _ => {
            return Err(fail(
                "managed role, scope, and structural role are inconsistent".into(),
            ));
        }
    }
    match (protocol.execution.purity, protocol.execution.effects) {
        (Purity::Pure, EffectSemantics::None)
        | (Purity::Effectful, EffectSemantics::Ordered | EffectSemantics::Exclusive) => {}
        (Purity::Pure, _) => return Err(fail("pure nodes cannot declare effects".into())),
        (Purity::Effectful, EffectSemantics::None) => {
            return Err(fail("effectful nodes must declare effect ordering".into()));
        }
    }
    if !categories
        .categories
        .contains_key(&protocol.catalog.category_id)
    {
        return Err(fail(format!(
            "unknown category '{}'",
            protocol.catalog.category_id
        )));
    }
    for key in catalog_i18n(protocol) {
        require_i18n(i18n, key).map_err(fail)?;
    }

    NodeInterfaceProtocol::new(
        protocol.interface.ports.to_vec(),
        protocol.interface.type_parameters.to_vec(),
        protocol.interface.type_constraints.to_vec(),
    )
    .and_then(|interface| interface.with_member_groups(protocol.interface.member_groups.to_vec()))
    .map_err(|e| fail(e.to_string()))?;
    let ports: BTreeMap<_, _> = protocol
        .interface
        .ports
        .iter()
        .map(|p| (&p.key, p))
        .collect();
    let parameters: BTreeSet<_> = protocol
        .parameters
        .parameters
        .iter()
        .map(|p| &p.key)
        .collect();
    if parameters.len() != protocol.parameters.parameters.len() {
        return Err(fail("duplicate parameter key".into()));
    }
    for port in &protocol.interface.ports {
        require_i18n(i18n, &port.label_key).map_err(&fail)?;
        validate_type_expr(&port.value_type, types, &protocol.interface.type_parameters)
            .map_err(&fail)?;
        if let PortInstances::Derived { resolver } = &port.instances {
            if !interface_resolvers.contains(resolver) {
                return Err(fail(format!("unknown interface resolver '{resolver}'")));
            }
        }
        if let Some(schema) = &port.schema {
            validate_schema(
                schema,
                &ports,
                &parameters,
                interface_resolvers,
                schema_resolvers,
            )
            .map_err(&fail)?;
        }
    }
    for parameter in &protocol.parameters.parameters {
        require_i18n(i18n, &parameter.title_key).map_err(&fail)?;
        if let Some(key) = &parameter.description_key {
            require_i18n(i18n, key).map_err(&fail)?;
        }
        validate_type_expr(
            &parameter.value_type,
            types,
            &protocol.interface.type_parameters,
        )
        .map_err(&fail)?;
        if let Some(default) = &parameter.default_value {
            if default.value_type != parameter.value_type {
                return Err(fail(format!(
                    "parameter '{}' default type does not match",
                    parameter.key
                )));
            }
        }
        validate_parameter_constraints(parameter).map_err(&fail)?;
    }
    for constraint in &protocol.interface.type_constraints {
        validate_constraint(
            constraint,
            &ports,
            &parameters,
            types,
            &protocol.interface.type_parameters,
        )
        .map_err(&fail)?;
    }
    Ok(())
}

fn require_i18n(manifest: &I18nManifest, key: &I18nKey) -> Result<(), String> {
    if manifest.keys.contains(key) {
        Ok(())
    } else {
        Err(format!("missing i18n key '{key}'"))
    }
}
fn catalog_i18n(protocol: &NodeProtocol) -> impl Iterator<Item = &I18nKey> {
    std::iter::once(&protocol.catalog.title_key)
        .chain(protocol.catalog.description_key.iter())
        .chain(protocol.catalog.documentation_key.iter())
        .chain(protocol.catalog.aliases_key.iter())
}

fn validate_type_expr(
    expr: &TypeExpr,
    types: &TypeRegistry,
    parameters: &[TypeParameterId],
) -> Result<(), String> {
    match expr {
        TypeExpr::Concrete(id) if !types.types.contains_key(id) => {
            Err(format!("unknown type '{id}'"))
        }
        TypeExpr::Generic(id) if !parameters.contains(id) => {
            Err(format!("unknown type parameter '{id}'"))
        }
        TypeExpr::Applied {
            constructor,
            arguments,
        } => {
            let Some(reg) = types.constructors.get(constructor) else {
                return Err(format!("unknown type constructor '{constructor}'"));
            };
            if arguments.len() != reg.arity as usize {
                return Err(format!(
                    "type constructor '{constructor}' expects {} arguments",
                    reg.arity
                ));
            }
            for arg in arguments {
                validate_type_expr(arg, types, parameters)?;
            }
            Ok(())
        }
        TypeExpr::Union(items) if items.len() < 2 => {
            Err("union type must contain at least two alternatives".into())
        }
        TypeExpr::Union(items) => {
            for item in items {
                validate_type_expr(item, types, parameters)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_term(
    term: &TypeTerm,
    ports: &BTreeMap<&PortKey, &PortSpec>,
    parameters: &BTreeSet<&ParameterKey>,
    types: &TypeRegistry,
    type_parameters: &[TypeParameterId],
) -> Result<(), String> {
    match term {
        TypeTerm::Expr(expr) => validate_type_expr(expr, types, type_parameters),
        TypeTerm::Port(key) if !ports.contains_key(key) => {
            Err(format!("constraint references unknown port '{key}'"))
        }
        TypeTerm::Parameter(key) if !parameters.contains(key) => {
            Err(format!("constraint references unknown parameter '{key}'"))
        }
        _ => Ok(()),
    }
}
fn validate_constraint(
    c: &TypeConstraint,
    ports: &BTreeMap<&PortKey, &PortSpec>,
    parameters: &BTreeSet<&ParameterKey>,
    types: &TypeRegistry,
    type_parameters: &[TypeParameterId],
) -> Result<(), String> {
    let terms: Vec<&TypeTerm> = match c {
        TypeConstraint::Equal(a, b)
        | TypeConstraint::Assignable(a, b)
        | TypeConstraint::ElementOf(a, b) => vec![a, b],
        TypeConstraint::Implements(a, class) => {
            if !types.classes.contains(class) {
                return Err(format!("unknown type class '{class}'"));
            }
            vec![a]
        }
        TypeConstraint::OneOf(a, choices) => std::iter::once(a).chain(choices).collect(),
    };
    for term in terms {
        validate_term(term, ports, parameters, types, type_parameters)?;
    }
    Ok(())
}

fn validate_schema(
    expr: &SchemaExpr,
    ports: &BTreeMap<&PortKey, &PortSpec>,
    parameters: &BTreeSet<&ParameterKey>,
    interface_resolvers: &BTreeSet<InterfaceResolverId>,
    schema_resolvers: &BTreeSet<SchemaResolverId>,
) -> Result<(), String> {
    let input = |key: &PortKey| match ports.get(key) {
        Some(p) if p.direction == PortDirection::Input && p.kind == PortKind::Data => Ok(()),
        Some(_) => Err(format!("schema source port '{key}' is not a data input")),
        None => Err(format!("schema references unknown port '{key}'")),
    };
    match expr {
        SchemaExpr::Input(key) => input(key),
        SchemaExpr::Project {
            input: nested,
            columns,
        } => {
            validate_schema(
                nested,
                ports,
                parameters,
                interface_resolvers,
                schema_resolvers,
            )?;
            if let ColumnSelectionExpr::FromParameter(key) = columns {
                if !parameters.contains(key) {
                    return Err(format!("schema references unknown parameter '{key}'"));
                }
            }
            Ok(())
        }
        SchemaExpr::Append { inputs } => {
            if inputs.is_empty() {
                return Err("schema append requires an input".into());
            }
            for nested in inputs {
                validate_schema(
                    nested,
                    ports,
                    parameters,
                    interface_resolvers,
                    schema_resolvers,
                )?;
            }
            Ok(())
        }
        SchemaExpr::Rename {
            input: nested,
            mapping,
        } => {
            validate_schema(
                nested,
                ports,
                parameters,
                interface_resolvers,
                schema_resolvers,
            )?;
            if let RenameExpr::FromParameter(key) = mapping {
                if !parameters.contains(key) {
                    return Err(format!("schema references unknown parameter '{key}'"));
                }
            }
            Ok(())
        }
        SchemaExpr::Filter { input: nested } => validate_schema(
            nested,
            ports,
            parameters,
            interface_resolvers,
            schema_resolvers,
        ),
        SchemaExpr::Derived {
            resolver,
            dependencies,
        } => {
            if !schema_resolvers.contains(resolver) {
                return Err(format!("unknown schema resolver '{resolver}'"));
            }
            for dependency in dependencies {
                match dependency {
                    SchemaDependency::Port(key) => input(key)?,
                    SchemaDependency::Parameter(key) if !parameters.contains(key) => {
                        return Err(format!("schema references unknown parameter '{key}'"));
                    }
                    SchemaDependency::Interface(id) if !interface_resolvers.contains(id) => {
                        return Err(format!("unknown interface resolver '{id}'"));
                    }
                    _ => {}
                }
            }
            Ok(())
        }
    }
}

fn validate_parameter_constraints(parameter: &ParameterSpec) -> Result<(), String> {
    for constraint in &parameter.constraints {
        match constraint {
            ParameterConstraint::IntegerRange {
                min: Some(min),
                max: Some(max),
            } if min > max => {
                return Err(format!(
                    "parameter '{}' integer range is inverted",
                    parameter.key
                ));
            }
            ParameterConstraint::Length {
                min: Some(min),
                max: Some(max),
            } if min > max => {
                return Err(format!(
                    "parameter '{}' length range is inverted",
                    parameter.key
                ));
            }
            ParameterConstraint::OneOf(values) if values.is_empty() => {
                return Err(format!("parameter '{}' has empty choices", parameter.key));
            }
            _ => {}
        }
    }
    Ok(())
}

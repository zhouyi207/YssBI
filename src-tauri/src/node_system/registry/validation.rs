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
    DuplicateNominalValidator(TypeId),
    MissingNominalValidator(TypeId),
    RawJsonNominalPayload(TypeId),
    NominalRegistrationIdExhausted,
    InvalidNominalTypeId {
        value: Box<str>,
        source: InvalidSemanticId,
    },
    InvalidNode {
        node: NodeTypeId,
        reason: String,
    },
    InvalidNodeProtocol {
        node: NodeTypeId,
        source: ProtocolError,
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
            DuplicateNominalValidator(id) => {
                write!(f, "nominal validator '{id}' is already registered")
            }
            MissingNominalValidator(id) => {
                write!(f, "built-in nominal type '{id}' has no validator")
            }
            RawJsonNominalPayload(id) => {
                write!(f, "nominal codec '{id}' cannot prepare raw JSON values")
            }
            NominalRegistrationIdExhausted => {
                write!(f, "nominal registration ID space is exhausted")
            }
            InvalidNominalTypeId { value, source } => {
                write!(f, "invalid built-in nominal type ID '{value}': {source}")
            }
            InvalidNode { node, reason } => write!(f, "invalid node '{node}': {reason}"),
            InvalidNodeProtocol { node, source } => {
                write!(f, "invalid node protocol '{node}': {source}")
            }
            InvalidType { id, reason } => write!(f, "invalid type '{id}': {reason}"),
            InvalidTypeConstructor { id, reason } => {
                write!(f, "invalid type constructor '{id}': {reason}")
            }
            InvalidCategory { id, reason } => write!(f, "invalid category '{id}': {reason}"),
        }
    }
}
impl std::error::Error for RegistryValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidNominalTypeId { source, .. } => Some(source),
            Self::InvalidNodeProtocol { source, .. } => Some(source),
            _ => None,
        }
    }
}

struct BuiltinNominalTypeIds {
    project_columns: TypeId,
    filter_predicate: TypeId,
}

fn required_nominal_type_id(value: &str) -> Result<TypeId, RegistryValidationError> {
    TypeId::new(value).map_err(|source| RegistryValidationError::InvalidNominalTypeId {
        value: value.into(),
        source,
    })
}

fn builtin_nominal_type_ids() -> Result<BuiltinNominalTypeIds, RegistryValidationError> {
    Ok(BuiltinNominalTypeIds {
        project_columns: required_nominal_type_id(
            crate::node_system::protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
        )?,
        filter_predicate: required_nominal_type_id(
            crate::node_system::protocol::dataframe::FILTER_PREDICATE_TYPE_ID,
        )?,
    })
}

pub(crate) struct ValidatedParts {
    pub nodes: BTreeMap<NodeTypeId, std::sync::Arc<RegisteredNode>>,
    pub node_providers: BTreeMap<NodeTypeId, ProviderId>,
    pub types: TypeRegistry,
    pub type_providers: BTreeMap<TypeId, ProviderId>,
    pub categories: CategoryRegistry,
    pub i18n: I18nManifest,
}

pub(crate) fn validate(
    providers: &[ProviderRegistration],
    nominal_validators: &BTreeMap<TypeId, super::NominalParameterValidator>,
) -> Result<ValidatedParts, RegistryValidationError> {
    let mut provider_ids = BTreeSet::new();
    let mut nodes = BTreeMap::new();
    let mut node_providers = BTreeMap::new();
    let mut types = TypeRegistry::default();
    let mut type_providers = BTreeMap::new();
    let mut categories = CategoryRegistry::default();
    let mut i18n = I18nManifest::default();
    let mut interface_resolvers = BTreeSet::new();
    let mut schema_resolvers = BTreeSet::new();
    let nominal_type_ids = builtin_nominal_type_ids()?;

    for provider in providers {
        if !provider_ids.insert(provider.provider.clone()) {
            return Err(RegistryValidationError::DuplicateProvider(
                provider.provider.clone(),
            ));
        }
        for item in &provider.types {
            match types.types.entry(item.id.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(item.clone());
                    type_providers.insert(item.id.clone(), provider.provider.clone());
                }
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err(RegistryValidationError::DuplicateType(item.id.clone()));
                }
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
            match nodes.entry(id.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(std::sync::Arc::new(node.clone()));
                    node_providers.insert(id, provider.provider.clone());
                }
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err(RegistryValidationError::DuplicateNode(id));
                }
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
    for id in [
        &nominal_type_ids.project_columns,
        &nominal_type_ids.filter_predicate,
    ] {
        if types.types.contains_key(id) && !nominal_validators.contains_key(id) {
            return Err(RegistryValidationError::MissingNominalValidator(id.clone()));
        }
    }
    for node in nodes.values() {
        validate_node(
            node,
            &types,
            &categories,
            &i18n,
            &interface_resolvers,
            &schema_resolvers,
            &nominal_type_ids,
        )?;
    }
    Ok(ValidatedParts {
        nodes,
        node_providers,
        types,
        type_providers,
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
    nominal_type_ids: &BuiltinNominalTypeIds,
) -> Result<(), RegistryValidationError> {
    let protocol = &node.protocol;
    validate_execution(protocol.execution).map_err(|source| {
        RegistryValidationError::InvalidNodeProtocol {
            node: protocol.type_id.clone(),
            source,
        }
    })?;
    let fail = |reason: String| RegistryValidationError::InvalidNode {
        node: protocol.type_id.clone(),
        reason,
    };
    match (
        &node.implementation,
        node.structural_role,
        node.transparent_role,
    ) {
        (Some(implementation), None, None)
            if implementation.capability() == ImplementationKind::CompilerLowering => {}
        (Some(_), None, None) => {
            return Err(fail(
                "leaf implementation does not provide lowerer capability".into(),
            ));
        }
        (None, Some(_), None) | (None, None, Some(_)) => {}
        (None, None, None) => return Err(fail("node has no executable interpretation".into())),
        (Some(_), Some(_), None) => {
            return Err(fail(
                "leaf implementation and structural role are mutually exclusive".into(),
            ));
        }
        _ => {
            return Err(fail(
                "leaf, structural, and transparent behaviors are mutually exclusive".into(),
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
    .map_err(|source| RegistryValidationError::InvalidNodeProtocol {
        node: protocol.type_id.clone(),
        source,
    })?;
    let ports: BTreeMap<_, _> = protocol
        .interface
        .ports
        .iter()
        .map(|p| (&p.key, p))
        .collect();
    let parameter_specs: BTreeMap<_, _> = protocol
        .parameters
        .parameters
        .iter()
        .map(|parameter| (&parameter.key, parameter))
        .collect();
    let parameters = parameter_specs.keys().copied().collect::<BTreeSet<_>>();
    if parameter_specs.len() != protocol.parameters.parameters.len() {
        return Err(fail("duplicate parameter key".into()));
    }
    for port in &protocol.interface.ports {
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
                &parameter_specs,
                interface_resolvers,
                schema_resolvers,
                nominal_type_ids,
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
    if let NodeInstanceDisplaySpec::ResourceParameter { parameter, kind } =
        &protocol.instance_display
    {
        let Some(parameter_spec) = parameter_specs.get(parameter) else {
            return Err(fail(format!(
                "instance display parameter '{parameter}' does not exist"
            )));
        };
        match &parameter_spec.editor {
            ParameterEditorSpec::Resource {
                kind: parameter_kind,
            } if parameter_kind == kind => {}
            ParameterEditorSpec::Resource {
                kind: parameter_kind,
            } => {
                return Err(fail(format!(
                    "instance display resource kind '{kind:?}' is incompatible with parameter '{parameter}' kind '{parameter_kind:?}'"
                )));
            }
            _ => {
                return Err(fail(format!(
                    "instance display parameter '{parameter}' must use the resource editor"
                )));
            }
        }
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

fn validate_schema_parameter(
    key: &ParameterKey,
    parameters: &BTreeMap<&ParameterKey, &ParameterSpec>,
) -> Result<(), String> {
    parameters
        .contains_key(key)
        .then_some(())
        .ok_or_else(|| format!("schema references unknown parameter '{key}'"))
}

fn validate_schema_parameter_type(
    key: &ParameterKey,
    parameters: &BTreeMap<&ParameterKey, &ParameterSpec>,
    expected: &TypeId,
) -> Result<(), String> {
    validate_schema_parameter(key, parameters)?;
    if parameters[key].value_type == TypeExpr::Concrete(expected.clone()) {
        Ok(())
    } else {
        Err(format!(
            "schema parameter '{key}' must have nominal type '{expected}'"
        ))
    }
}

#[cfg(test)]
mod nominal_schema_tests {
    use super::*;

    #[test]
    fn invalid_builtin_nominal_type_id_preserves_identity_source() {
        let error = required_nominal_type_id("Bad Nominal Type").unwrap_err();
        assert!(matches!(
            &error,
            RegistryValidationError::InvalidNominalTypeId { value, source }
                if value.as_ref() == "Bad Nominal Type"
                    && source == &TypeId::new("Bad Nominal Type").unwrap_err()
        ));
        assert!(
            std::error::Error::source(&error)
                .and_then(|source| source.downcast_ref::<InvalidSemanticId>())
                .is_some()
        );
    }

    fn parameter(key: &str, value_type: TypeExpr) -> ParameterSpec {
        ParameterSpec {
            key: ParameterKey::new(key).unwrap(),
            title_key: I18nKey::new(format!("parameters.{key}.title")).unwrap(),
            description_key: None,
            value_type,
            default_value: None,
            constraints: vec![ParameterConstraint::Required],
            editor: ParameterEditorSpec::Auto,
            presentation: ParameterPresentation::DetailPanel,
        }
    }

    fn ports() -> BTreeMap<&'static PortKey, &'static PortSpec> {
        let key: &'static PortKey = Box::leak(Box::new(PortKey::new("source").unwrap()));
        let port: &'static PortSpec = Box::leak(Box::new(PortSpec {
            key: key.clone(),
            title: "Source".into(),
            direction: PortDirection::Input,
            kind: PortKind::Data,
            value_type: TypeExpr::Unknown,
            instances: PortInstances::Declared,
            connections: ConnectionsPerPort::Single,
            input_binding: None,
            consumption: Some(InputConsumption::Streaming),
            production: None,
            editor: PortEditorSpec::Default,
            schema: None,
        }));
        BTreeMap::from([(key, port)])
    }

    #[test]
    fn project_and_filter_schema_require_exact_nominal_parameter_types() {
        let source = || Box::new(SchemaExpr::Input(PortKey::new("source").unwrap()));
        let project_key = ParameterKey::new("columns").unwrap();
        let filter_key = ParameterKey::new("predicate").unwrap();
        let project = SchemaExpr::Project {
            input: source(),
            columns: ColumnSelectionExpr::FromParameter(project_key.clone()),
        };
        let filter = SchemaExpr::Filter {
            input: source(),
            predicate: Some(filter_key.clone()),
        };
        let port_map = ports();
        let interface_resolvers = BTreeSet::<InterfaceResolverId>::new();
        let schema_resolvers = BTreeSet::<SchemaResolverId>::new();
        let nominal_type_ids = builtin_nominal_type_ids().unwrap();

        for (expression, key, expected_type) in [
            (
                project,
                project_key,
                crate::node_system::protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
            ),
            (
                filter,
                filter_key,
                crate::node_system::protocol::dataframe::FILTER_PREDICATE_TYPE_ID,
            ),
        ] {
            let wrong = parameter(key.as_str(), TypeExpr::Unknown);
            let wrong_parameters = BTreeMap::from([(&wrong.key, &wrong)]);
            assert!(
                validate_schema(
                    &expression,
                    &port_map,
                    &wrong_parameters,
                    &interface_resolvers,
                    &schema_resolvers,
                    &nominal_type_ids,
                )
                .is_err()
            );

            let exact = parameter(
                key.as_str(),
                TypeExpr::Concrete(TypeId::new(expected_type).unwrap()),
            );
            let exact_parameters = BTreeMap::from([(&exact.key, &exact)]);
            assert!(
                validate_schema(
                    &expression,
                    &port_map,
                    &exact_parameters,
                    &interface_resolvers,
                    &schema_resolvers,
                    &nominal_type_ids,
                )
                .is_ok()
            );
        }
    }
}

fn validate_schema(
    expr: &SchemaExpr,
    ports: &BTreeMap<&PortKey, &PortSpec>,
    parameters: &BTreeMap<&ParameterKey, &ParameterSpec>,
    interface_resolvers: &BTreeSet<InterfaceResolverId>,
    schema_resolvers: &BTreeSet<SchemaResolverId>,
    nominal_type_ids: &BuiltinNominalTypeIds,
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
                nominal_type_ids,
            )?;
            if let ColumnSelectionExpr::FromParameter(key) = columns {
                validate_schema_parameter_type(key, parameters, &nominal_type_ids.project_columns)?;
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
                    nominal_type_ids,
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
                nominal_type_ids,
            )?;
            match mapping {
                RenameExpr::FromParameter(key) => {
                    validate_schema_parameter(key, parameters)?;
                }
                RenameExpr::FromParameters { from, to } => {
                    validate_schema_parameter(from, parameters)?;
                    validate_schema_parameter(to, parameters)?;
                }
                RenameExpr::Explicit(_) => {}
            }
            Ok(())
        }
        SchemaExpr::Filter {
            input: nested,
            predicate,
        } => {
            if let Some(predicate) = predicate {
                validate_schema_parameter_type(
                    predicate,
                    parameters,
                    &nominal_type_ids.filter_predicate,
                )?;
            }
            validate_schema(
                nested,
                ports,
                parameters,
                interface_resolvers,
                schema_resolvers,
                nominal_type_ids,
            )
        }
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
                    SchemaDependency::Parameter(key) if !parameters.contains_key(key) => {
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

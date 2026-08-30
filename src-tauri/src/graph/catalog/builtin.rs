use super::diagnostics::{
    COMPILER_DIAGNOSTIC_DEFINITIONS, CompilerDiagnosticDefinitionError,
    validate_compiler_diagnostic_definitions,
};
use super::{control, core_nodes, dataframe, distribution, plot, project, statistics};
use crate::graph::catalog::{Aliases, BuiltinCatalog, Message, Text};
use crate::graph::registry::*;
use yss_graph_protocol::*;

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const PROVIDER: &str = "yssbi.builtin";

pub struct BuiltinNodeSystem {
    pub registry: Arc<NodeRegistry>,
    pub catalog: Arc<BuiltinCatalog>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinInitializationError {
    Assembly(BuiltinAssemblyError),
    Localization(crate::graph::catalog::I18nBundleValidationError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinAssemblyError {
    DiagnosticDefinitions {
        source: CompilerDiagnosticDefinitionError,
    },
    InvalidSemanticId {
        value: Box<str>,
        source: InvalidSemanticId,
    },
    InvalidProtocol {
        node_type: Box<str>,
        source: ProtocolError,
    },
    InvalidParameterSchema {
        node_type: Box<str>,
        source: ParameterSchemaError,
    },
    InvalidDecimal {
        node_type: Box<str>,
        source: InvalidDecimal,
    },
    InvalidDefaultBinding {
        node_type: Box<str>,
        source: ProtocolError,
    },
    LocalizationConflict {
        locale: Box<str>,
        key: Box<str>,
    },
    UnsupportedBuiltinConfiguration {
        context: &'static str,
        value: Box<str>,
    },
    UnsupportedStatisticsPredictionFamily {
        family: Box<str>,
    },
    Registration(NodeRegistrationError),
}

impl std::fmt::Display for BuiltinInitializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Assembly(error) => error.fmt(formatter),
            Self::Localization(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BuiltinInitializationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Assembly(error) => Some(error),
            Self::Localization(error) => Some(error),
        }
    }
}

impl std::fmt::Display for BuiltinAssemblyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DiagnosticDefinitions { source } => write!(
                formatter,
                "built-in compiler diagnostic definitions are invalid: {source}",
            ),
            Self::InvalidSemanticId { value, source } => {
                write!(
                    formatter,
                    "invalid built-in semantic ID '{value}': {source}"
                )
            }
            Self::InvalidProtocol { node_type, source } => {
                write!(
                    formatter,
                    "built-in protocol '{node_type}' is invalid: {source}"
                )
            }
            Self::InvalidParameterSchema { node_type, source } => write!(
                formatter,
                "built-in parameter schema '{node_type}' is invalid: {source}",
            ),
            Self::InvalidDecimal { node_type, source } => write!(
                formatter,
                "built-in decimal for '{node_type}' is invalid: {source}",
            ),
            Self::InvalidDefaultBinding { node_type, source } => write!(
                formatter,
                "built-in default binding for '{node_type}' is invalid: {source}",
            ),
            Self::LocalizationConflict { locale, key } => write!(
                formatter,
                "built-in localization key '{key}' has conflicting values for '{locale}'",
            ),
            Self::UnsupportedBuiltinConfiguration { context, value } => {
                write!(formatter, "unsupported built-in {context}: '{value}'")
            }
            Self::UnsupportedStatisticsPredictionFamily { family } => write!(
                formatter,
                "statistics prediction does not support the '{family}' model family",
            ),
            Self::Registration(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BuiltinAssemblyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DiagnosticDefinitions { source } => Some(source),
            Self::InvalidSemanticId { source, .. } => Some(source),
            Self::InvalidProtocol { source, .. } => Some(source),
            Self::InvalidParameterSchema { source, .. } => Some(source),
            Self::InvalidDecimal { source, .. } => Some(source),
            Self::InvalidDefaultBinding { source, .. } => Some(source),
            Self::LocalizationConflict { .. }
            | Self::UnsupportedBuiltinConfiguration { .. }
            | Self::UnsupportedStatisticsPredictionFamily { .. } => None,
            Self::Registration(error) => Some(error),
        }
    }
}

impl From<BuiltinAssemblyError> for BuiltinInitializationError {
    fn from(error: BuiltinAssemblyError) -> Self {
        Self::Assembly(error)
    }
}

impl From<NodeRegistrationError> for BuiltinInitializationError {
    fn from(error: NodeRegistrationError) -> Self {
        Self::Assembly(BuiltinAssemblyError::Registration(error))
    }
}

impl From<crate::graph::catalog::I18nBundleValidationError> for BuiltinInitializationError {
    fn from(error: crate::graph::catalog::I18nBundleValidationError) -> Self {
        Self::Localization(error)
    }
}

pub(crate) fn assembled_interface(
    node_type: &str,
    ports: Vec<PortSpec>,
    type_parameters: Vec<TypeParameterId>,
    type_constraints: Vec<TypeConstraint>,
    member_groups: Vec<PortMemberGroupSpec>,
) -> Result<NodeInterfaceProtocol, BuiltinAssemblyError> {
    NodeInterfaceProtocol::new(ports, type_parameters, type_constraints)
        .and_then(|interface| interface.with_member_groups(member_groups))
        .map_err(|source| match &source {
            ProtocolError::InvalidPortContract { reason, .. } if reason.contains("default") => {
                BuiltinAssemblyError::InvalidDefaultBinding {
                    node_type: node_type.into(),
                    source,
                }
            }
            _ => BuiltinAssemblyError::InvalidProtocol {
                node_type: node_type.into(),
                source,
            },
        })
}

pub(crate) fn assembled_parameters(
    node_type: &str,
    parameters: Vec<ParameterSpec>,
) -> Result<ParameterSchema, BuiltinAssemblyError> {
    ParameterSchema::new(parameters).map_err(|source| {
        BuiltinAssemblyError::InvalidParameterSchema {
            node_type: node_type.into(),
            source,
        }
    })
}

pub(crate) fn assembled_decimal(
    node_type: &str,
    value: &'static str,
) -> Result<CanonicalDecimal, BuiltinAssemblyError> {
    CanonicalDecimal::new(value).map_err(|source| BuiltinAssemblyError::InvalidDecimal {
        node_type: node_type.into(),
        source,
    })
}

#[derive(Debug, Default)]
pub(crate) struct ProviderFragment {
    pub types: Vec<TypeRegistration>,
    pub type_constructors: Vec<TypeConstructorRegistration>,
    pub categories: Vec<CategoryRegistration>,
    pub i18n: I18nManifest,
    pub interface_resolvers: Vec<InterfaceResolverId>,
    pub schema_resolvers: Vec<SchemaResolverId>,
    pub nodes: Vec<RegisteredNode>,
    pub messages: Vec<(&'static str, &'static str, Message)>,
}

impl ProviderFragment {
    pub(crate) fn merge(&mut self, fragment: Self) {
        self.types.extend(fragment.types);
        self.type_constructors.extend(fragment.type_constructors);
        self.categories.extend(fragment.categories);
        self.i18n.keys.extend(fragment.i18n.keys);
        self.interface_resolvers
            .extend(fragment.interface_resolvers);
        self.schema_resolvers.extend(fragment.schema_resolvers);
        self.nodes.extend(fragment.nodes);
        self.messages.extend(fragment.messages);
    }

    pub(crate) fn finish(mut self) -> Result<Self, BuiltinAssemblyError> {
        let message_keys = self
            .messages
            .iter()
            .map(|(_, key, _)| iid(key))
            .collect::<Result<Vec<_>, _>>()?;
        self.i18n.keys.extend(message_keys);
        self.types.sort_by(|left, right| left.id.cmp(&right.id));
        self.type_constructors
            .sort_by(|left, right| left.id.cmp(&right.id));
        self.categories
            .sort_by(|left, right| left.id.cmp(&right.id));
        self.interface_resolvers.sort();
        self.schema_resolvers.sort();
        self.nodes
            .sort_by(|left, right| left.protocol().type_id.cmp(&right.protocol().type_id));

        let mut messages = BTreeMap::new();
        for (locale, key, message) in self.messages {
            if let Some(existing) = messages.insert((locale, key), message.clone()) {
                if existing != message {
                    return Err(BuiltinAssemblyError::LocalizationConflict {
                        locale: locale.into(),
                        key: key.into(),
                    });
                }
            }
        }
        self.messages = messages
            .into_iter()
            .map(|((locale, key), message)| (locale, key, message))
            .collect();
        Ok(self)
    }
}

#[derive(Clone, Copy)]
struct FamilySpec {
    id: &'static str,
    title: &'static str,
    zh_title: &'static str,
    aliases: &'static [&'static str],
    zh_aliases: &'static [&'static str],
    kernel: &'static str,
}

const NUMERIC: &[FamilySpec] = &[
    FamilySpec {
        id: "add",
        title: "Add",
        zh_title: "加法",
        aliases: &["plus", "sum", "+"],
        zh_aliases: &["相加", "求和", "+"],
        kernel: "numeric.add",
    },
    FamilySpec {
        id: "subtract",
        title: "Subtract",
        zh_title: "减法",
        aliases: &["minus", "difference", "-"],
        zh_aliases: &["相减", "差", "-"],
        kernel: "numeric.subtract",
    },
    FamilySpec {
        id: "multiply",
        title: "Multiply",
        zh_title: "乘法",
        aliases: &["times", "product", "*"],
        zh_aliases: &["相乘", "积", "*"],
        kernel: "numeric.multiply",
    },
    FamilySpec {
        id: "divide",
        title: "Divide",
        zh_title: "除法",
        aliases: &["quotient", "/"],
        zh_aliases: &["相除", "商", "/"],
        kernel: "numeric.divide",
    },
];

const COMPARISONS: &[FamilySpec] = &[
    FamilySpec {
        id: "equal",
        title: "Equal",
        zh_title: "等于",
        aliases: &["equals", "=="],
        zh_aliases: &["相等", "=="],
        kernel: "compare.equal",
    },
    FamilySpec {
        id: "not_equal",
        title: "Not Equal",
        zh_title: "不等于",
        aliases: &["different", "!="],
        zh_aliases: &["不相等", "!="],
        kernel: "compare.not_equal",
    },
    FamilySpec {
        id: "less",
        title: "Less Than",
        zh_title: "小于",
        aliases: &["lower", "<"],
        zh_aliases: &["更小", "<"],
        kernel: "compare.less",
    },
    FamilySpec {
        id: "less_equal",
        title: "Less or Equal",
        zh_title: "小于等于",
        aliases: &["at most", "<="],
        zh_aliases: &["不大于", "<="],
        kernel: "compare.less_equal",
    },
    FamilySpec {
        id: "greater",
        title: "Greater Than",
        zh_title: "大于",
        aliases: &["higher", ">"],
        zh_aliases: &["更大", ">"],
        kernel: "compare.greater",
    },
    FamilySpec {
        id: "greater_equal",
        title: "Greater or Equal",
        zh_title: "大于等于",
        aliases: &["at least", ">="],
        zh_aliases: &["不小于", ">="],
        kernel: "compare.greater_equal",
    },
];

const LOGIC: &[FamilySpec] = &[
    FamilySpec {
        id: "and",
        title: "And",
        zh_title: "且",
        aliases: &["all", "&&"],
        zh_aliases: &["与", "并且", "&&"],
        kernel: "logic.and",
    },
    FamilySpec {
        id: "or",
        title: "Or",
        zh_title: "或",
        aliases: &["any", "||"],
        zh_aliases: &["或者", "||"],
        kernel: "logic.or",
    },
];

pub fn build_builtin_node_system() -> Result<BuiltinNodeSystem, BuiltinInitializationError> {
    let (provider, catalog, alias_keys) = assemble_builtin_parts()?;
    validate_builtin_bundle(provider, catalog, alias_keys)
}

fn validate_builtin_bundle(
    mut provider: ProviderRegistration,
    catalog: BuiltinCatalog,
    alias_keys: BTreeSet<I18nKey>,
) -> Result<BuiltinNodeSystem, BuiltinInitializationError> {
    let mut builder = NodeRegistryBuilder::new();
    let handles = register_builtin_nominal_validators(&mut builder)?;
    super::dataframe::bind_nominal_handles(&mut provider, handles);
    builder
        .register_provider(provider)
        .map_err(BuiltinAssemblyError::Registration)?;
    let registry = Arc::new(
        builder
            .freeze()
            .map_err(BuiltinAssemblyError::Registration)?,
    );
    catalog.validate(&registry.catalog_manifest().i18n, &alias_keys)?;
    Ok(BuiltinNodeSystem {
        registry,
        catalog: Arc::new(catalog),
    })
}

#[cfg(test)]
pub(crate) fn builtin_bundle_parts_for_test()
-> Result<(ProviderRegistration, BuiltinCatalog, BTreeSet<I18nKey>), BuiltinInitializationError> {
    assemble_builtin_parts().map_err(Into::into)
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinAssemblyTestFault {
    InvalidSemanticId(&'static str),
    InvalidProtocol(&'static str),
    InvalidRegistryProtocol,
    LocalizationConflict,
    DuplicateRegistration,
}

#[cfg(test)]
pub(crate) fn build_builtin_node_system_with_test_fault(
    fault: BuiltinAssemblyTestFault,
) -> Result<BuiltinNodeSystem, BuiltinInitializationError> {
    let parts = match fault {
        BuiltinAssemblyTestFault::InvalidSemanticId(value) => {
            assemble_builtin_parts_with(move |fragment| {
                fragment.nodes.push(leaf(
                    protocol(value, "constants", Vec::new(), Vec::new(), pure())?,
                    "test.invalid_semantic_id",
                ));
                Ok(())
            })
        }
        BuiltinAssemblyTestFault::InvalidProtocol(node_type) => {
            assemble_builtin_parts_with(move |_| {
                let port = data_port("duplicate", "Duplicate", PortDirection::Input, "core.bool")?;
                assembled_interface(
                    node_type,
                    vec![port.clone(), port],
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )?;
                Ok(())
            })
        }
        BuiltinAssemblyTestFault::InvalidRegistryProtocol => {
            let mut parts = assemble_builtin_parts()?;
            let node = parts
                .0
                .nodes
                .iter_mut()
                .find(|node| node.protocol().type_id.as_str() == "yssbi.constant.bool")
                .expect("built-in test fixture must contain bool constant");
            let mut protocol = node.protocol().clone();
            let duplicate = protocol.interface.ports[0].clone();
            protocol.interface.ports = vec![duplicate.clone(), duplicate].into_boxed_slice();
            *node = RegisteredNode::leaf(
                Arc::new(protocol),
                node.implementation()
                    .expect("built-in bool constant must be executable")
                    .clone(),
            );
            Ok(parts)
        }
        BuiltinAssemblyTestFault::LocalizationConflict => assemble_builtin_parts_with(|fragment| {
            fragment.messages.extend([
                ("en-US", "nodes.test.title", Text("First")),
                ("en-US", "nodes.test.title", Text("Second")),
            ]);
            Ok(())
        }),
        BuiltinAssemblyTestFault::DuplicateRegistration => {
            assemble_builtin_parts_with(|fragment| {
                fragment.nodes.push(leaf(
                    constant_protocol("yssbi.constant.bool", "core.bool", Value::Bool(false))?,
                    "test.duplicate_registration",
                ));
                Ok(())
            })
        }
    }?;
    validate_builtin_bundle(parts.0, parts.1, parts.2)
}

#[cfg(test)]
pub(crate) fn validate_builtin_bundle_for_test(
    provider: ProviderRegistration,
    catalog: BuiltinCatalog,
    alias_keys: BTreeSet<I18nKey>,
) -> Result<BuiltinNodeSystem, BuiltinInitializationError> {
    validate_builtin_bundle(provider, catalog, alias_keys)
}

fn register_builtin_nominal_validators(
    builder: &mut NodeRegistryBuilder,
) -> Result<super::dataframe::DataframeNominalHandles, BuiltinAssemblyError> {
    let parse_type_id = |value| sid(value, TypeId::new);
    let project_columns = builder
        .register_nominal_codec(
            parse_type_id(yss_graph_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID)?,
            parse_type_id(yss_graph_protocol::dataframe::PROJECT_COLUMNS_VALIDATOR_ID)?,
            yss_graph_protocol::dataframe::DATAFRAME_NOMINAL_CODEC_VERSION,
            yss_graph_protocol::dataframe::prepare_project_columns_json,
        )
        .map_err(BuiltinAssemblyError::Registration)?;
    let filter_predicate = builder
        .register_nominal_codec(
            parse_type_id(yss_graph_protocol::dataframe::FILTER_PREDICATE_TYPE_ID)?,
            parse_type_id(yss_graph_protocol::dataframe::FILTER_PREDICATE_VALIDATOR_ID)?,
            yss_graph_protocol::dataframe::DATAFRAME_NOMINAL_CODEC_VERSION,
            yss_graph_protocol::dataframe::prepare_filter_predicate_json,
        )
        .map_err(BuiltinAssemblyError::Registration)?;
    Ok(super::dataframe::DataframeNominalHandles {
        project_columns,
        filter_predicate,
    })
}

#[cfg(test)]
pub(crate) fn register_builtin_nominal_validators_for_test(
    builder: &mut NodeRegistryBuilder,
) -> Result<(), BuiltinInitializationError> {
    register_builtin_nominal_validators(builder)
        .map(|_| ())
        .map_err(Into::into)
}

fn assemble_builtin_parts()
-> Result<(ProviderRegistration, BuiltinCatalog, BTreeSet<I18nKey>), BuiltinAssemblyError> {
    assemble_builtin_parts_with(|_| Ok(()))
}

fn assemble_builtin_parts_with(
    inject: impl FnOnce(&mut ProviderFragment) -> Result<(), BuiltinAssemblyError>,
) -> Result<(ProviderRegistration, BuiltinCatalog, BTreeSet<I18nKey>), BuiltinAssemblyError> {
    validate_compiler_diagnostic_definitions(COMPILER_DIAGNOSTIC_DEFINITIONS)
        .map_err(|source| BuiltinAssemblyError::DiagnosticDefinitions { source })?;

    let mut fragment = ProviderFragment::default();
    let messages = &mut fragment.messages;
    let nodes = &mut fragment.nodes;
    add_shared_messages(messages);
    add_diagnostic_messages(messages);

    let zero = assembled_decimal("yssbi.constant.float64", "0")?;
    for (kind, title, zh, value) in [
        ("bool", "Boolean Constant", "布尔常量", Value::Bool(false)),
        (
            "string",
            "String Constant",
            "字符串常量",
            Value::String("".into()),
        ),
        (
            "int64",
            "Int64 Constant",
            "64 位整数常量",
            Value::Integer(0),
        ),
        (
            "float64",
            "Float64 Constant",
            "64 位浮点数常量",
            Value::Decimal(zero),
        ),
    ] {
        let id = leak(format!("yssbi.constant.{kind}"));
        add_node_messages(
            messages,
            id,
            title,
            zh,
            &["constant", "literal", "value"],
            &["常量", "字面量", "值"],
        );
        let ty = leak(format!("core.{kind}"));
        nodes.push(leaf(
            constant_protocol(id, ty, value)?,
            leak(format!("constant.{kind}")),
        ));
    }

    for ty in ["int64", "float64"] {
        for spec in NUMERIC {
            let id = leak(format!("yssbi.numeric.{}.{}", spec.id, ty));
            add_node_messages(
                messages,
                id,
                spec.title,
                spec.zh_title,
                spec.aliases,
                spec.zh_aliases,
            );
            nodes.push(leaf(
                binary_protocol(
                    id,
                    "numeric",
                    leak(format!("core.{ty}")),
                    leak(format!("core.{ty}")),
                )?,
                leak(format!("{}.{}", spec.kernel, ty)),
            ));
        }
    }
    for spec in COMPARISONS {
        let id = leak(format!("yssbi.logic.{}", spec.id));
        add_node_messages(
            messages,
            id,
            spec.title,
            spec.zh_title,
            spec.aliases,
            spec.zh_aliases,
        );
        let protocol = if matches!(spec.id, "equal" | "not_equal") {
            equality_protocol(id)?
        } else {
            binary_protocol(id, "logic", "core.float64", "core.bool")?
        };
        nodes.push(leaf(protocol, spec.kernel));
    }
    for spec in LOGIC {
        let id = leak(format!("yssbi.logic.{}", spec.id));
        add_node_messages(
            messages,
            id,
            spec.title,
            spec.zh_title,
            spec.aliases,
            spec.zh_aliases,
        );
        nodes.push(leaf(
            binary_protocol(id, "logic", "core.bool", "core.bool")?,
            spec.kernel,
        ));
    }
    add_node_messages(
        messages,
        "yssbi.logic.not",
        "Not",
        "非",
        &["invert", "!"],
        &["取反", "!"],
    );
    nodes.push(leaf(
        unary_protocol("yssbi.logic.not", "logic", "core.bool", "core.bool")?,
        "logic.not",
    ));

    control::register(nodes, messages)?;
    project::register(nodes, messages)?;

    fragment.types.extend(
        ["bool", "string", "int64", "float64"]
            .into_iter()
            .map(|name| {
                Ok(TypeRegistration {
                    id: sid(leak(format!("core.{name}")), TypeId::new)?,
                    title_key: iid(leak(format!("types.{name}.title")))?,
                    classes: if matches!(name, "int64" | "float64") {
                        BTreeSet::from([sid(NUMERIC_TYPE_CLASS_ID, TypeClassId::new)?])
                    } else {
                        BTreeSet::new()
                    },
                })
            })
            .collect::<Result<Vec<_>, BuiltinAssemblyError>>()?,
    );
    fragment.categories.extend(
        [
            ("constants", 10),
            ("numeric", 20),
            ("logic", 30),
            ("control", 40),
            ("project", 50),
        ]
        .into_iter()
        .map(|(name, order)| {
            Ok(CategoryRegistration {
                id: sid(name, NodeCategoryId::new)?,
                title_key: iid(leak(format!("categories.{name}.title")))?,
                parent: None,
                order,
            })
        })
        .collect::<Result<Vec<_>, BuiltinAssemblyError>>()?,
    );
    fragment
        .interface_resolvers
        .extend(project::builtin_function_interface_resolver_ids());
    fragment.merge(core_nodes::build_provider_fragment()?);
    fragment.merge(dataframe::build_provider_fragment()?);
    fragment.merge(statistics::build_provider_fragment()?);
    fragment.merge(distribution::build_provider_fragment()?);
    fragment.merge(plot::build_provider_fragment()?);
    inject(&mut fragment)?;
    let fragment = fragment.finish()?;
    let (required_i18n, alias_keys) =
        i18n_requirements(&fragment.types, &fragment.categories, &fragment.nodes)?;
    let mut i18n = fragment.i18n;
    i18n.keys.extend(required_i18n.keys);
    let catalog = BuiltinCatalog::new(&fragment.messages).map_err(|source| {
        BuiltinAssemblyError::InvalidProtocol {
            node_type: PROVIDER.into(),
            source,
        }
    })?;
    let mut provider = ProviderRegistration::new(sid(PROVIDER, ProviderId::new)?);
    provider.types = fragment.types.into_boxed_slice();
    provider.type_constructors = fragment.type_constructors.into_boxed_slice();
    provider.type_classes = vec![sid(NUMERIC_TYPE_CLASS_ID, TypeClassId::new)?].into_boxed_slice();
    provider.categories = fragment.categories.into_boxed_slice();
    provider.i18n = i18n;
    provider.interface_resolvers = fragment.interface_resolvers.into_boxed_slice();
    provider.schema_resolvers = fragment.schema_resolvers.into_boxed_slice();
    provider.nodes = fragment.nodes.into_boxed_slice();
    Ok((provider, catalog, alias_keys))
}

pub(super) fn leaf(protocol: NodeProtocol, kernel: &'static str) -> RegisteredNode {
    RegisteredNode::leaf(Arc::new(protocol), CatalogNodeImplementation::new(kernel))
}

struct CatalogNodeImplementation {
    identity: Box<str>,
}

impl CatalogNodeImplementation {
    fn new(kernel: &str) -> Self {
        Self {
            identity: if kernel.starts_with("yssbi.") {
                kernel.to_owned().into_boxed_str()
            } else {
                format!("yssbi.{kernel}").into_boxed_str()
            },
        }
    }
}

impl crate::graph::registry::NodeImplementation for CatalogNodeImplementation {
    fn capability(&self) -> crate::graph::registry::ImplementationKind {
        crate::graph::registry::ImplementationKind::CompilerLowering
    }

    fn implementation_identity(&self) -> &str {
        &self.identity
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl From<CatalogNodeImplementation> for crate::graph::registry::LeafImplementation {
    fn from(value: CatalogNodeImplementation) -> Self {
        crate::graph::registry::LeafImplementation::from_arc(Arc::new(value))
    }
}

fn constant_protocol(
    id: &'static str,
    ty: &'static str,
    value: Value,
) -> Result<NodeProtocol, BuiltinAssemblyError> {
    let editor = match ty {
        "core.bool" => ParameterEditorSpec::Toggle,
        "core.int64" | "core.float64" => ParameterEditorSpec::Number,
        "core.string" => ParameterEditorSpec::Text { multiline: false },
        _ => {
            return Err(BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
                context: "constant type",
                value: ty.into(),
            });
        }
    };
    protocol(
        id,
        "constants",
        vec![data_port("value", "Value", PortDirection::Output, ty)?],
        vec![ParameterSpec {
            key: sid("value", ParameterKey::new)?,
            title_key: iid("parameters.value.title")?,
            description_key: Some(iid("parameters.value.description")?),
            value_type: concrete(ty)?,
            default_value: Some(ParameterValue {
                value_type: concrete(ty)?,
                value,
            }),
            constraints: vec![],
            editor,
            presentation: ParameterPresentation::InlineAndDetail,
        }],
        pure(),
    )
}

fn binary_protocol(
    id: &'static str,
    category: &'static str,
    input: &'static str,
    output: &'static str,
) -> Result<NodeProtocol, BuiltinAssemblyError> {
    protocol(
        id,
        category,
        vec![
            data_port("left", "Left", PortDirection::Input, input)?,
            data_port("right", "Right", PortDirection::Input, input)?,
            data_port("result", "Result", PortDirection::Output, output)?,
        ],
        vec![],
        pure(),
    )
}

fn equality_protocol(id: &'static str) -> Result<NodeProtocol, BuiltinAssemblyError> {
    let value = sid("value", TypeParameterId::new)?;
    let mut protocol = protocol(id, "logic", vec![], vec![], pure())?;
    protocol.interface = assembled_interface(
        id,
        vec![
            data_port_expr(
                "left",
                "Left",
                PortDirection::Input,
                TypeExpr::Generic(value.clone()),
            )?,
            data_port_expr(
                "right",
                "Right",
                PortDirection::Input,
                TypeExpr::Generic(value.clone()),
            )?,
            data_port("result", "Result", PortDirection::Output, "core.bool")?,
        ],
        vec![value],
        vec![],
        vec![],
    )?;
    Ok(protocol)
}

fn unary_protocol(
    id: &'static str,
    category: &'static str,
    input: &'static str,
    output: &'static str,
) -> Result<NodeProtocol, BuiltinAssemblyError> {
    protocol(
        id,
        category,
        vec![
            data_port("input", "Input", PortDirection::Input, input)?,
            data_port("result", "Result", PortDirection::Output, output)?,
        ],
        vec![],
        pure(),
    )
}

fn protocol(
    id: &'static str,
    category: &'static str,
    ports: Vec<PortSpec>,
    parameters: Vec<ParameterSpec>,
    execution: ExecutionSemantics,
) -> Result<NodeProtocol, BuiltinAssemblyError> {
    Ok(NodeProtocol {
        type_id: sid(id, NodeTypeId::new)?,
        catalog: NodeCatalogProtocol {
            title_key: iid(leak(format!("nodes.{id}.title")))?,
            documentation_key: Some(iid(leak(format!("nodes.{id}.documentation")))?),
            aliases_key: Some(iid(leak(format!("nodes.{id}.aliases")))?),
            category_id: sid(category, NodeCategoryId::new)?,
            icon_id: sid(leak(format!("builtin.{category}")), IconId::new)?,
            style_id: sid("builtin.default", NodeStyleId::new)?,
            hidden: false,
        },
        interface: assembled_interface(id, ports, vec![], vec![], vec![])?,
        parameters: assembled_parameters(id, parameters)?,
        instance_display: NodeInstanceDisplaySpec::Static,
        execution,
        scope: NodeScope::Any,
        managed_role: None,
    })
}

fn data_port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
    ty: &'static str,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port_expr(key, title, direction, concrete(ty)?)
}

fn data_port_expr(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: sid(key, PortKey::new)?,
        title: title.into(),
        direction,
        kind: PortKind::Data,
        value_type,
        instances: PortInstances::Declared,
        connections: ConnectionsPerPort::Single,
        input_binding: (direction == PortDirection::Input).then_some(InputBindingSpec {
            literal_policy: LiteralPolicy::Allowed,
            default_value: None,
        }),
        consumption: None,
        production: (direction == PortDirection::Output)
            .then_some(OutputProduction::FullyMaterialized),
        editor: PortEditorSpec::Default,
        schema: None,
    })
}

fn concrete(id: &'static str) -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(TypeExpr::Concrete(sid(id, TypeId::new)?))
}
fn pure() -> ExecutionSemantics {
    ExecutionSemantics {
        determinism: Determinism::Deterministic,
        purity: Purity::Pure,
        evaluation: EvaluationPolicy::DemandDriven,
        cache: CachePolicy::PerRun,
        effects: EffectSemantics::None,
        idempotent: false,
        retry: None,
    }
}

pub(super) fn iid(value: &'static str) -> Result<I18nKey, BuiltinAssemblyError> {
    sid(value, I18nKey::new)
}
pub(super) fn sid<T>(
    value: &'static str,
    make: impl FnOnce(&'static str) -> Result<T, InvalidSemanticId>,
) -> Result<T, BuiltinAssemblyError> {
    make(value).map_err(|source| BuiltinAssemblyError::InvalidSemanticId {
        value: value.into(),
        source,
    })
}
fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn i18n_requirements(
    types: &[TypeRegistration],
    categories: &[CategoryRegistration],
    nodes: &[RegisteredNode],
) -> Result<(I18nManifest, BTreeSet<I18nKey>), BuiltinAssemblyError> {
    let mut keys = BTreeSet::new();
    let mut alias_keys = BTreeSet::new();
    keys.extend(types.iter().map(|item| item.title_key.clone()));
    keys.extend(categories.iter().map(|item| item.title_key.clone()));
    for node in nodes {
        let protocol = node.protocol();
        keys.insert(protocol.catalog.title_key.clone());
        keys.extend(protocol.catalog.documentation_key.iter().cloned());
        if let Some(key) = &protocol.catalog.aliases_key {
            keys.insert(key.clone());
            alias_keys.insert(key.clone());
        }
        for parameter in &protocol.parameters.parameters {
            keys.insert(parameter.title_key.clone());
            keys.extend(parameter.description_key.iter().cloned());
        }
    }
    for definition in COMPILER_DIAGNOSTIC_DEFINITIONS {
        keys.insert(I18nKey::new(definition.message_key).map_err(|source| {
            BuiltinAssemblyError::InvalidSemanticId {
                value: definition.message_key.into(),
                source,
            }
        })?);
    }
    Ok((I18nManifest { keys }, alias_keys))
}

fn add_diagnostic_messages(out: &mut Vec<(&'static str, &'static str, Message)>) {
    for definition in COMPILER_DIAGNOSTIC_DEFINITIONS {
        out.extend(
            definition
                .templates
                .iter()
                .map(|template| (template.locale, definition.message_key, Text(template.text))),
        );
    }
}

fn add_shared_messages(out: &mut Vec<(&'static str, &'static str, Message)>) {
    for (key, en, zh) in [
        ("types.bool.title", "Boolean", "布尔"),
        ("types.string.title", "String", "字符串"),
        ("types.int64.title", "Int64", "64 位整数"),
        ("types.float64.title", "Float64", "64 位浮点数"),
        ("categories.constants.title", "Constants", "常量"),
        ("categories.numeric.title", "Numeric", "数值"),
        ("categories.logic.title", "Logic", "逻辑"),
        ("categories.control.title", "Flow Control", "流程控制"),
        ("categories.project.title", "Project", "项目"),
        ("parameters.value.title", "Value", "值"),
        (
            "parameters.value.description",
            "The constant value.",
            "常量的值。",
        ),
    ] {
        out.push(("en-US", key, Text(en)));
        out.push(("zh-CN", key, Text(zh)));
    }
}
fn add_node_messages(
    out: &mut Vec<(&'static str, &'static str, Message)>,
    id: &'static str,
    en: &'static str,
    zh: &'static str,
    en_aliases: &'static [&'static str],
    zh_aliases: &'static [&'static str],
) {
    let title = leak(format!("nodes.{id}.title"));
    let docs = leak(format!("nodes.{id}.documentation"));
    let aliases = leak(format!("nodes.{id}.aliases"));
    out.extend([
        ("en-US", title, Text(en)),
        ("zh-CN", title, Text(zh)),
        (
            "en-US",
            docs,
            Text("This node is part of the trusted built-in provider."),
        ),
        ("zh-CN", docs, Text("此节点属于可信内建 provider。")),
        ("en-US", aliases, Aliases(en_aliases)),
        ("zh-CN", aliases, Aliases(zh_aliases)),
    ]);
}

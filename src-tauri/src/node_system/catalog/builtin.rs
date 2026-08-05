use super::localization::{Aliases, BuiltinCatalog, Message, Text};
use super::{control, core_nodes, dataframe, distribution, plot, project, statistics};
use crate::node_system::compiler::{
    LoweredKernel, LoweredNode, LoweringContext, LoweringError, NodeImplementation, NodeLowerer,
    builtin_function_interface_resolver_ids,
};
use crate::node_system::plan::{CompiledParameterHandle, KernelHandle};
use crate::node_system::protocol::*;
use crate::node_system::registry::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const PROVIDER: &str = "yssbi.builtin";

pub struct BuiltinNodeSystem {
    pub registry: Arc<NodeRegistry>,
    pub catalog: Arc<BuiltinCatalog>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinInitializationError {
    Registration(NodeRegistrationError),
    Localization(super::localization::I18nBundleValidationError),
}

impl std::fmt::Display for BuiltinInitializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registration(error) => error.fmt(formatter),
            Self::Localization(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BuiltinInitializationError {}

impl From<NodeRegistrationError> for BuiltinInitializationError {
    fn from(error: NodeRegistrationError) -> Self {
        Self::Registration(error)
    }
}

impl From<super::localization::I18nBundleValidationError> for BuiltinInitializationError {
    fn from(error: super::localization::I18nBundleValidationError) -> Self {
        Self::Localization(error)
    }
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

    pub(crate) fn finish(mut self) -> Self {
        self.i18n
            .keys
            .extend(self.messages.iter().map(|(_, key, _)| iid(key)));
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
            match messages.insert((locale, key), message.clone()) {
                Some(existing) => assert_eq!(
                    existing, message,
                    "built-in localization key '{key}' has conflicting values for '{locale}'",
                ),
                None => {}
            }
        }
        self.messages = messages
            .into_iter()
            .map(|((locale, key), message)| (locale, key, message))
            .collect();
        self
    }
}

const COMPILER_DIAGNOSTIC_CODES: &[&str] = &[
    "compiler.connection.input_direction",
    "compiler.connection.kind_mismatch",
    "compiler.connection.limit",
    "compiler.connection.order_forbidden",
    "compiler.connection.order_required",
    "compiler.connection.output_direction",
    "compiler.control.ambiguous_output",
    "compiler.control.binding_invalid",
    "compiler.control.binding_port_missing",
    "compiler.control.binding_required",
    "compiler.control.call.resource_parameter_missing",
    "compiler.control.call.target_invalid",
    "compiler.control.control_port_required",
    "compiler.control.cycle",
    "compiler.control.data_port_required",
    "compiler.control.entry.output_required",
    "compiler.control.leaf_without_operation",
    "compiler.control.loop.carried_required",
    "compiler.control.loop.max_iterations_required",
    "compiler.control.managed_role_mismatch",
    "compiler.control.no_entry",
    "compiler.control.return.input_required",
    "compiler.control.return_has_successor",
    "compiler.control.shared_region",
    "compiler.control.unreachable",
    "compiler.control.value_missing",
    "compiler.dependency.value_cycle",
    "compiler.document.connection_id_mismatch",
    "compiler.document.node_id_mismatch",
    "compiler.input.conflicting_bindings",
    "compiler.input.literal_forbidden",
    "compiler.input.not_input",
    "compiler.input.unbound",
    "compiler.input.unknown_port",
    "compiler.interface.basis_mismatch",
    "compiler.interface.duplicate_locator",
    "compiler.interface.identity_none_connection",
    "compiler.interface.identity_none_override",
    "compiler.interface.resolver_failed",
    "compiler.interface.resolver_missing",
    "compiler.lowering.effect_contract",
    "compiler.lowering.failed",
    "compiler.lowering.implementation_missing",
    "compiler.lowering.resource_conflict",
    "compiler.lowering.result_duplicate",
    "compiler.lowering.result_port",
    "compiler.node.disappeared",
    "compiler.node.unknown",
    "compiler.parameter.invalid",
    "compiler.parameter.required",
    "compiler.parameter.unknown",
    "compiler.plan.effect_consumer_missing",
    "compiler.plan.effect_producer_missing",
    "compiler.plan.invalid",
    "compiler.plan.invalid_node_id",
    "compiler.plan.value_consumer_missing",
    "compiler.plan.value_producer_missing",
    "compiler.port.binding_not_instance",
    "compiler.port.instance_not_allowed",
    "compiler.port.orphan",
    "compiler.port.unknown",
    "compiler.registry.type_mismatch",
    "compiler.relational.backend_mismatch",
    "compiler.relational.filter_column_missing",
    "compiler.relational.filter_literal_forbidden",
    "compiler.relational.filter_literal_missing",
    "compiler.relational.filter_literal_type",
    "compiler.relational.filter_operator_invalid",
    "compiler.relational.fragment_unplanned",
    "compiler.relational.input_binding_missing",
    "compiler.relational.planning_failed",
    "compiler.schema.parameter_invalid",
    "compiler.schema.project_empty",
    "compiler.schema.project_field_duplicate",
    "compiler.schema.project_field_missing",
    "compiler.schema.rename_field_missing",
    "compiler.schema.rename_source_duplicate",
    "compiler.schema.rename_target_conflict",
    "compiler.schema.resolver_failed",
    "compiler.schema.resolver_missing",
    "compiler.semantic.invalid",
    "compiler.type.incompatible",
];

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

struct KernelLowerer(&'static str);
impl NodeLowerer for KernelLowerer {
    fn lower(&self, context: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Ok(LoweredNode {
            kernel: LoweredKernel::Native(
                KernelHandle::new(if self.0.starts_with("yssbi.") {
                    self.0.to_string()
                } else {
                    format!("yssbi.{}", self.0)
                })
                .map_err(|e| LoweringError::new(e.to_string()))?,
            ),
            parameters: CompiledParameterHandle::new(format!("node.{}", context.node_id))
                .map_err(|e| LoweringError::new(e.to_string()))?,
        })
    }
}

pub fn build_builtin_node_system() -> Result<BuiltinNodeSystem, BuiltinInitializationError> {
    let (provider, catalog, alias_keys) = assemble_builtin_parts();
    validate_builtin_bundle(provider, catalog, alias_keys)
}

fn validate_builtin_bundle(
    provider: ProviderRegistration,
    catalog: BuiltinCatalog,
    alias_keys: BTreeSet<I18nKey>,
) -> Result<BuiltinNodeSystem, BuiltinInitializationError> {
    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(provider)?;
    register_builtin_nominal_validators(&mut builder)?;
    let registry = Arc::new(builder.freeze()?);
    catalog.validate(&registry.catalog_manifest().i18n, &alias_keys)?;
    Ok(BuiltinNodeSystem {
        registry,
        catalog: Arc::new(catalog),
    })
}

#[cfg(test)]
pub(crate) fn builtin_bundle_parts_for_test()
-> (ProviderRegistration, BuiltinCatalog, BTreeSet<I18nKey>) {
    assemble_builtin_parts()
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
) -> Result<(), NodeRegistrationError> {
    let parse_type_id = |value| {
        TypeId::new(value).map_err(|error| {
            NodeRegistrationError::InvalidProtocol(ProtocolError::InvalidIdentity(
                error.to_string(),
            ))
        })
    };
    builder.register_nominal_validator(
        parse_type_id(crate::node_system::parameter_types::dataframe::PROJECT_COLUMNS_TYPE_ID)?,
        parse_type_id(
            crate::node_system::parameter_types::dataframe::PROJECT_COLUMNS_VALIDATOR_ID,
        )?,
        crate::node_system::parameter_types::dataframe::DATAFRAME_NOMINAL_CODEC_VERSION,
        crate::node_system::parameter_types::dataframe::validate_project_columns_json,
    )?;
    builder.register_nominal_validator(
        parse_type_id(crate::node_system::parameter_types::dataframe::FILTER_PREDICATE_TYPE_ID)?,
        parse_type_id(
            crate::node_system::parameter_types::dataframe::FILTER_PREDICATE_VALIDATOR_ID,
        )?,
        crate::node_system::parameter_types::dataframe::DATAFRAME_NOMINAL_CODEC_VERSION,
        crate::node_system::parameter_types::dataframe::validate_filter_predicate_json,
    )?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn register_builtin_nominal_validators_for_test(
    builder: &mut NodeRegistryBuilder,
) -> Result<(), BuiltinInitializationError> {
    register_builtin_nominal_validators(builder).map_err(Into::into)
}

fn assemble_builtin_parts() -> (ProviderRegistration, BuiltinCatalog, BTreeSet<I18nKey>) {
    let mut fragment = ProviderFragment::default();
    let messages = &mut fragment.messages;
    let nodes = &mut fragment.nodes;
    add_shared_messages(messages);
    add_diagnostic_messages(messages);

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
            Value::Decimal(CanonicalDecimal::new("0").unwrap()),
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
            constant_protocol(id, ty, value),
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
                ),
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
            equality_protocol(id)
        } else {
            binary_protocol(id, "logic", "core.float64", "core.bool")
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
            binary_protocol(id, "logic", "core.bool", "core.bool"),
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
        unary_protocol("yssbi.logic.not", "logic", "core.bool", "core.bool"),
        "logic.not",
    ));

    control::register(nodes, messages);
    project::register(nodes, messages);

    fragment.types.extend(
        ["bool", "string", "int64", "float64"].map(|name| TypeRegistration {
            id: sid(leak(format!("core.{name}")), TypeId::new),
            title_key: iid(leak(format!("types.{name}.title"))),
            classes: BTreeSet::new(),
        }),
    );
    fragment.categories.extend(
        [
            ("constants", 10),
            ("numeric", 20),
            ("logic", 30),
            ("control", 40),
            ("project", 50),
        ]
        .map(|(name, order)| CategoryRegistration {
            id: sid(name, NodeCategoryId::new),
            title_key: iid(leak(format!("categories.{name}.title"))),
            parent: None,
            order,
        }),
    );
    fragment
        .interface_resolvers
        .extend(builtin_function_interface_resolver_ids());
    for family in [
        core_nodes::build_provider_fragment(),
        dataframe::build_provider_fragment(),
        statistics::build_provider_fragment(),
        distribution::build_provider_fragment(),
        plot::build_provider_fragment(),
    ] {
        fragment.merge(family);
    }
    let fragment = fragment.finish();
    let (required_i18n, alias_keys) =
        i18n_requirements(&fragment.types, &fragment.categories, &fragment.nodes);
    let mut i18n = fragment.i18n;
    i18n.keys.extend(required_i18n.keys);
    let catalog = BuiltinCatalog::new(&fragment.messages);
    let mut provider = ProviderRegistration::new(sid(PROVIDER, ProviderId::new));
    provider.types = fragment.types.into_boxed_slice();
    provider.type_constructors = fragment.type_constructors.into_boxed_slice();
    provider.categories = fragment.categories.into_boxed_slice();
    provider.i18n = i18n;
    provider.interface_resolvers = fragment.interface_resolvers.into_boxed_slice();
    provider.schema_resolvers = fragment.schema_resolvers.into_boxed_slice();
    provider.nodes = fragment.nodes.into_boxed_slice();
    (provider, catalog, alias_keys)
}

pub(super) fn leaf(protocol: NodeProtocol, kernel: &'static str) -> RegisteredNode {
    RegisteredNode::leaf(
        Arc::new(protocol),
        Arc::new(NodeImplementation::new(KernelLowerer(kernel))),
    )
}

fn constant_protocol(id: &'static str, ty: &'static str, value: Value) -> NodeProtocol {
    protocol(
        id,
        "constants",
        vec![data_port("value", PortDirection::Output, ty)],
        vec![ParameterSpec {
            key: sid("value", ParameterKey::new),
            title_key: iid("parameters.value.title"),
            description_key: Some(iid("parameters.value.description")),
            value_type: concrete(ty),
            default_value: Some(ParameterValue {
                value_type: concrete(ty),
                value,
            }),
            constraints: vec![],
            editor: ParameterEditorSpec::Auto,
        }],
        pure(),
    )
}

fn binary_protocol(
    id: &'static str,
    category: &'static str,
    input: &'static str,
    output: &'static str,
) -> NodeProtocol {
    protocol(
        id,
        category,
        vec![
            data_port("left", PortDirection::Input, input),
            data_port("right", PortDirection::Input, input),
            data_port("result", PortDirection::Output, output),
        ],
        vec![],
        pure(),
    )
}
fn equality_protocol(id: &'static str) -> NodeProtocol {
    let value = sid("value", TypeParameterId::new);
    let mut protocol = protocol(id, "logic", vec![], vec![], pure());
    protocol.interface = NodeInterfaceProtocol::new(
        vec![
            data_port_expr(
                "left",
                PortDirection::Input,
                TypeExpr::Generic(value.clone()),
            ),
            data_port_expr(
                "right",
                PortDirection::Input,
                TypeExpr::Generic(value.clone()),
            ),
            data_port("result", PortDirection::Output, "core.bool"),
        ],
        vec![value],
        vec![],
    )
    .expect("equality interface");
    protocol
}

fn unary_protocol(
    id: &'static str,
    category: &'static str,
    input: &'static str,
    output: &'static str,
) -> NodeProtocol {
    protocol(
        id,
        category,
        vec![
            data_port("input", PortDirection::Input, input),
            data_port("result", PortDirection::Output, output),
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
) -> NodeProtocol {
    NodeProtocol {
        type_id: sid(id, NodeTypeId::new),
        catalog: NodeCatalogProtocol {
            title_key: iid(leak(format!("nodes.{id}.title"))),
            description_key: Some(iid(leak(format!("nodes.{id}.description")))),
            documentation_key: Some(iid(leak(format!("nodes.{id}.documentation")))),
            aliases_key: Some(iid(leak(format!("nodes.{id}.aliases")))),
            category_id: sid(category, NodeCategoryId::new),
            icon_id: sid(leak(format!("builtin.{category}")), IconId::new),
            style_id: sid("builtin.default", NodeStyleId::new),
            hidden: false,
        },
        interface: NodeInterfaceProtocol::new(ports, vec![], vec![]).expect("built-in interface"),
        parameters: ParameterSchema::new(parameters).expect("built-in parameters"),
        execution,
        scope: NodeScope::Any,
        managed_role: None,
    }
}

fn data_port(key: &'static str, direction: PortDirection, ty: &'static str) -> PortSpec {
    data_port_expr(key, direction, concrete(ty))
}

fn data_port_expr(key: &'static str, direction: PortDirection, value_type: TypeExpr) -> PortSpec {
    PortSpec {
        key: sid(key, PortKey::new),
        label_key: iid(leak(format!("ports.{key}.label"))),
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
    }
}

fn concrete(id: &'static str) -> TypeExpr {
    TypeExpr::Concrete(sid(id, TypeId::new))
}
fn pure() -> ExecutionSemantics {
    ExecutionSemantics {
        determinism: Determinism::Deterministic,
        purity: Purity::Pure,
        evaluation: EvaluationPolicy::DemandDriven,
        cache: CachePolicy::PerRun,
        effects: EffectSemantics::None,
    }
}

pub(super) fn iid(value: &'static str) -> I18nKey {
    sid(value, I18nKey::new)
}
pub(super) fn sid<T>(
    value: &'static str,
    make: impl FnOnce(&'static str) -> Result<T, InvalidSemanticId>,
) -> T {
    make(value).unwrap()
}
fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn i18n_requirements(
    types: &[TypeRegistration],
    categories: &[CategoryRegistration],
    nodes: &[RegisteredNode],
) -> (I18nManifest, BTreeSet<I18nKey>) {
    let mut keys = BTreeSet::new();
    let mut alias_keys = BTreeSet::new();
    keys.extend(types.iter().map(|item| item.title_key.clone()));
    keys.extend(categories.iter().map(|item| item.title_key.clone()));
    for node in nodes {
        let protocol = node.protocol();
        keys.insert(protocol.catalog.title_key.clone());
        keys.extend(protocol.catalog.description_key.iter().cloned());
        keys.extend(protocol.catalog.documentation_key.iter().cloned());
        if let Some(key) = &protocol.catalog.aliases_key {
            keys.insert(key.clone());
            alias_keys.insert(key.clone());
        }
        keys.extend(
            protocol
                .interface
                .ports
                .iter()
                .map(|port| port.label_key.clone()),
        );
        for parameter in &protocol.parameters.parameters {
            keys.insert(parameter.title_key.clone());
            keys.extend(parameter.description_key.iter().cloned());
        }
    }
    keys.extend(COMPILER_DIAGNOSTIC_CODES.iter().map(|code| {
        I18nKey::new(format!("diagnostics.{code}"))
            .expect("compiler diagnostic codes form valid i18n keys")
    }));
    (I18nManifest { keys }, alias_keys)
}

fn add_diagnostic_messages(out: &mut Vec<(&'static str, &'static str, Message)>) {
    for code in COMPILER_DIAGNOSTIC_CODES {
        let key = leak(format!("diagnostics.{code}"));
        out.push(("en-US", key, Text("Compiler diagnostic: {detail}")));
        out.push(("zh-CN", key, Text("编译诊断：{detail}")));
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
        ("ports.value.label", "Value", "值"),
        ("ports.left.label", "Left", "左值"),
        ("ports.right.label", "Right", "右值"),
        ("ports.result.label", "Result", "结果"),
        ("ports.input.label", "Input", "输入"),
        ("ports.enter.label", "Enter", "进入"),
        ("ports.condition.label", "Condition", "条件"),
        ("ports.true.label", "True", "真"),
        ("ports.false.label", "False", "假"),
        ("ports.then.label", "Then", "然后"),
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
    let description = leak(format!("nodes.{id}.description"));
    let docs = leak(format!("nodes.{id}.documentation"));
    let aliases = leak(format!("nodes.{id}.aliases"));
    out.extend([
        ("en-US", title, Text(en)),
        ("zh-CN", title, Text(zh)),
        ("en-US", description, Text("Built-in deterministic node.")),
        ("zh-CN", description, Text("内置确定性节点。")),
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

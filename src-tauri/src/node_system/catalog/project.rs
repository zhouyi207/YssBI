use super::builtin::{iid, leaf, sid};
use super::localization::{Aliases, Message, Text};
use crate::node_system::compiler::{
    FUNCTION_CALL_ARGUMENTS_RESOLVER, FUNCTION_CALL_RESULTS_RESOLVER,
    FUNCTION_ENTRY_PARAMETERS_RESOLVER, FUNCTION_RETURN_RESULTS_RESOLVER,
};
use crate::node_system::protocol::*;
use crate::node_system::registry::{RegisteredNode, StructuralNodeRole};
use std::sync::Arc;

pub(super) fn register(
    nodes: &mut Vec<RegisteredNode>,
    messages: &mut Vec<(&'static str, &'static str, Message)>,
) {
    for (id, en, zh, aliases, zh_aliases) in [
        (
            "yssbi.project.event.begin",
            "Event Begin",
            "事件开始",
            &["event", "start", "entry"][..],
            &["事件", "开始", "入口"][..],
        ),
        (
            "yssbi.project.function.entry",
            "Function Entry",
            "函数入口",
            &["function", "arguments", "parameters"][..],
            &["函数", "入口", "参数"][..],
        ),
        (
            "yssbi.project.function.return",
            "Function Return",
            "函数返回",
            &["function", "result", "exit"][..],
            &["函数", "返回", "结果"][..],
        ),
        (
            "yssbi.project.function.call",
            "Call Function",
            "调用函数",
            &["call", "invoke", "function"][..],
            &["调用", "执行", "函数"][..],
        ),
        (
            "yssbi.project.variable.get",
            "Get Variable",
            "读取变量",
            &["get", "read", "variable"][..],
            &["读取", "获取", "变量"][..],
        ),
        (
            "yssbi.project.variable.set",
            "Set Variable",
            "写入变量",
            &["set", "write", "variable"][..],
            &["写入", "设置", "变量"][..],
        ),
    ] {
        add_node_messages(messages, id, en, zh, aliases, zh_aliases);
    }

    nodes.extend([
        RegisteredNode::structural(
            Arc::new(event_begin_protocol()),
            StructuralNodeRole::EventBegin,
        ),
        RegisteredNode::structural(
            Arc::new(function_entry_protocol()),
            StructuralNodeRole::FunctionEntry,
        ),
        RegisteredNode::structural(
            Arc::new(function_return_protocol()),
            StructuralNodeRole::FunctionReturn,
        ),
        RegisteredNode::structural(Arc::new(function_call_protocol()), StructuralNodeRole::Call),
        leaf(variable_get_protocol(), "project.variable.get"),
        leaf(variable_set_protocol(), "project.variable.set"),
    ]);
    add_messages(messages);
}

fn event_begin_protocol() -> NodeProtocol {
    protocol(
        "yssbi.project.event.begin",
        vec![control_port("then", PortDirection::Output)],
        vec![],
        vec![],
        NodeScope::Event,
        Some(ManagedNodeRole::EventBegin),
        structural(),
    )
}

fn function_entry_protocol() -> NodeProtocol {
    protocol(
        "yssbi.project.function.entry",
        vec![
            control_port("then", PortDirection::Output),
            derived_data_port(
                "parameters",
                PortDirection::Output,
                FUNCTION_ENTRY_PARAMETERS_RESOLVER,
            ),
        ],
        vec![],
        vec![resource_parameter("function")],
        NodeScope::Function,
        Some(ManagedNodeRole::FunctionEntry),
        structural(),
    )
}

fn function_return_protocol() -> NodeProtocol {
    protocol(
        "yssbi.project.function.return",
        vec![
            control_port("enter", PortDirection::Input),
            derived_data_port(
                "results",
                PortDirection::Input,
                FUNCTION_RETURN_RESULTS_RESOLVER,
            ),
        ],
        vec![],
        vec![resource_parameter("function")],
        NodeScope::Function,
        Some(ManagedNodeRole::FunctionReturn),
        structural(),
    )
}

fn function_call_protocol() -> NodeProtocol {
    protocol(
        "yssbi.project.function.call",
        vec![
            control_port("enter", PortDirection::Input),
            derived_data_port(
                "arguments",
                PortDirection::Input,
                FUNCTION_CALL_ARGUMENTS_RESOLVER,
            ),
            derived_data_port(
                "results",
                PortDirection::Output,
                FUNCTION_CALL_RESULTS_RESOLVER,
            ),
            control_port("then", PortDirection::Output),
        ],
        vec![],
        vec![resource_parameter("target")],
        NodeScope::Any,
        None,
        structural(),
    )
}

fn variable_get_protocol() -> NodeProtocol {
    let generic = sid("value", TypeParameterId::new);
    protocol(
        "yssbi.project.variable.get",
        vec![data_port(
            "value",
            PortDirection::Output,
            TypeExpr::Generic(generic.clone()),
            PortInstances::Declared,
        )],
        vec![generic],
        vec![resource_parameter("variable")],
        NodeScope::Any,
        None,
        pure(),
    )
}

fn variable_set_protocol() -> NodeProtocol {
    let generic = sid("value", TypeParameterId::new);
    protocol(
        "yssbi.project.variable.set",
        vec![
            control_port("enter", PortDirection::Input),
            data_port(
                "value",
                PortDirection::Input,
                TypeExpr::Generic(generic.clone()),
                PortInstances::Declared,
            ),
            control_port("then", PortDirection::Output),
        ],
        vec![generic],
        vec![resource_parameter("variable")],
        NodeScope::Any,
        None,
        structural(),
    )
}

fn protocol(
    id: &'static str,
    ports: Vec<PortSpec>,
    type_parameters: Vec<TypeParameterId>,
    parameters: Vec<ParameterSpec>,
    scope: NodeScope,
    managed_role: Option<ManagedNodeRole>,
    execution: ExecutionSemantics,
) -> NodeProtocol {
    NodeProtocol {
        type_id: sid(id, NodeTypeId::new),
        catalog: NodeCatalogProtocol {
            title_key: node_key(id, "title"),
            description_key: Some(node_key(id, "description")),
            documentation_key: Some(node_key(id, "documentation")),
            aliases_key: Some(node_key(id, "aliases")),
            category_id: sid("project", NodeCategoryId::new),
            icon_id: sid("builtin.project", IconId::new),
            style_id: sid("builtin.default", NodeStyleId::new),
            hidden: false,
        },
        interface: NodeInterfaceProtocol::new(ports, type_parameters, vec![])
            .expect("built-in project interface"),
        parameters: ParameterSchema::new(parameters).expect("built-in project parameters"),
        execution,
        scope,
        managed_role,
    }
}

fn control_port(key: &'static str, direction: PortDirection) -> PortSpec {
    port(
        key,
        direction,
        PortKind::Control,
        TypeExpr::Unknown,
        PortInstances::Declared,
    )
}

fn derived_data_port(
    key: &'static str,
    direction: PortDirection,
    resolver: &'static str,
) -> PortSpec {
    data_port(
        key,
        direction,
        TypeExpr::Unknown,
        PortInstances::Derived {
            resolver: sid(resolver, InterfaceResolverId::new),
        },
    )
}

fn data_port(
    key: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
    instances: PortInstances,
) -> PortSpec {
    port(key, direction, PortKind::Data, value_type, instances)
}

fn port(
    key: &'static str,
    direction: PortDirection,
    kind: PortKind,
    value_type: TypeExpr,
    instances: PortInstances,
) -> PortSpec {
    PortSpec {
        key: sid(key, PortKey::new),
        label_key: iid(Box::leak(format!("ports.{key}.label").into_boxed_str())),
        direction,
        kind,
        value_type,
        instances,
        connections: ConnectionsPerPort::Single,
        input_binding: (kind == PortKind::Data && direction == PortDirection::Input).then_some(
            InputBindingSpec {
                literal_policy: LiteralPolicy::Forbidden,
                default_value: None,
            },
        ),
        consumption: None,
        production: (kind == PortKind::Data && direction == PortDirection::Output)
            .then_some(OutputProduction::FullyMaterialized),
        editor: PortEditorSpec::Default,
        schema: None,
    }
}

fn resource_parameter(key: &'static str) -> ParameterSpec {
    ParameterSpec {
        key: sid(key, ParameterKey::new),
        title_key: iid(Box::leak(
            format!("parameters.{key}.title").into_boxed_str(),
        )),
        description_key: Some(iid(Box::leak(
            format!("parameters.{key}.description").into_boxed_str(),
        ))),
        value_type: TypeExpr::Concrete(sid("core.string", TypeId::new)),
        default_value: None,
        constraints: vec![ParameterConstraint::Required],
        editor: ParameterEditorSpec::Resource,
    }
}

fn pure() -> ExecutionSemantics {
    ExecutionSemantics {
        determinism: Determinism::EnvironmentDependent,
        purity: Purity::Pure,
        evaluation: EvaluationPolicy::DemandDriven,
        cache: CachePolicy::PerRun,
        effects: EffectSemantics::None,
    }
}

fn structural() -> ExecutionSemantics {
    ExecutionSemantics {
        determinism: Determinism::EnvironmentDependent,
        purity: Purity::Effectful,
        evaluation: EvaluationPolicy::EagerWhenRegionEntered,
        cache: CachePolicy::None,
        effects: EffectSemantics::Ordered,
    }
}

fn node_key(id: &'static str, suffix: &'static str) -> I18nKey {
    iid(Box::leak(format!("nodes.{id}.{suffix}").into_boxed_str()))
}

fn add_messages(out: &mut Vec<(&'static str, &'static str, Message)>) {
    for (key, en, zh) in [
        ("ports.parameters.label", "Parameters", "参数"),
        ("ports.arguments.label", "Arguments", "实参"),
        ("ports.results.label", "Results", "结果"),
        ("parameters.function.title", "Function", "函数"),
        (
            "parameters.function.description",
            "Bound function resource.",
            "绑定的函数资源。",
        ),
        ("parameters.target.title", "Target Function", "目标函数"),
        (
            "parameters.target.description",
            "Function resource to invoke.",
            "要调用的函数资源。",
        ),
        ("parameters.variable.title", "Variable", "变量"),
        (
            "parameters.variable.description",
            "Bound project variable resource.",
            "绑定的项目变量资源。",
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
    let title: &'static str = Box::leak(format!("nodes.{id}.title").into_boxed_str());
    let description: &'static str = Box::leak(format!("nodes.{id}.description").into_boxed_str());
    let documentation: &'static str =
        Box::leak(format!("nodes.{id}.documentation").into_boxed_str());
    let aliases: &'static str = Box::leak(format!("nodes.{id}.aliases").into_boxed_str());
    out.extend([
        ("en-US", title, Text(en)),
        ("zh-CN", title, Text(zh)),
        ("en-US", description, Text("Integrates project resources with graph execution.")),
        ("zh-CN", description, Text("将项目资源接入图执行。")),
        ("en-US", documentation, Text("Resource identity is stored as a stable node parameter and resolved from the compilation snapshot.")),
        ("zh-CN", documentation, Text("资源身份作为稳定节点参数保存，并从编译快照解析。")),
        ("en-US", aliases, Aliases(en_aliases)),
        ("zh-CN", aliases, Aliases(zh_aliases)),
    ]);
}

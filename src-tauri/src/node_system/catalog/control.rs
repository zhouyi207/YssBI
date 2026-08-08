use super::builtin::{BuiltinAssemblyError, assembled_interface, assembled_parameters, iid, sid};
use super::localization::{Aliases, Message, Text};
use crate::node_system::protocol::*;
use crate::node_system::registry::{RegisteredNode, StructuralNodeRole};
use std::sync::Arc;

pub(super) fn register(
    nodes: &mut Vec<RegisteredNode>,
    messages: &mut Vec<(&'static str, &'static str, Message)>,
) -> Result<(), BuiltinAssemblyError> {
    add_node_messages(
        messages,
        "yssbi.control.branch",
        "Branch",
        "分支",
        &["if", "condition"],
        &["如果", "条件"],
    );
    nodes.push(RegisteredNode::structural(
        Arc::new(branch_protocol()?),
        StructuralNodeRole::Branch,
    ));

    add_node_messages(
        messages,
        "yssbi.control.sequence",
        "Sequence",
        "序列",
        &["then", "ordered"],
        &["然后", "顺序"],
    );
    nodes.push(RegisteredNode::structural(
        Arc::new(sequence_protocol()?),
        StructuralNodeRole::Sequence,
    ));

    add_node_messages(
        messages,
        "yssbi.control.loop",
        "Loop",
        "循环",
        &["iterate", "repeat", "while"],
        &["迭代", "重复", "当"],
    );
    nodes.push(RegisteredNode::structural(
        Arc::new(loop_protocol()?),
        StructuralNodeRole::Loop,
    ));

    add_messages(messages);
    Ok(())
}

fn branch_protocol() -> Result<NodeProtocol, BuiltinAssemblyError> {
    protocol(
        "yssbi.control.branch",
        vec![
            control_port("enter", PortDirection::Input, PortInstances::Declared)?,
            data_port(
                "condition",
                PortDirection::Input,
                TypeExpr::Concrete(sid("core.bool", TypeId::new)?),
                PortInstances::Declared,
            )?,
            data_port(
                "then_source",
                PortDirection::Input,
                TypeExpr::Unknown,
                PortInstances::UserCreated { min: 0, max: None },
            )?,
            data_port(
                "else_source",
                PortDirection::Input,
                TypeExpr::Unknown,
                PortInstances::UserCreated { min: 0, max: None },
            )?,
            control_port("true", PortDirection::Output, PortInstances::Declared)?,
            control_port("false", PortDirection::Output, PortInstances::Declared)?,
            data_port(
                "result",
                PortDirection::Output,
                TypeExpr::Unknown,
                PortInstances::UserCreated { min: 0, max: None },
            )?,
        ],
        vec![],
        vec![member_group(
            &["then_source", "else_source", "result"],
            0,
            None,
        )?],
    )
}

fn sequence_protocol() -> Result<NodeProtocol, BuiltinAssemblyError> {
    protocol(
        "yssbi.control.sequence",
        vec![
            control_port("enter", PortDirection::Input, PortInstances::Declared)?,
            control_port(
                "then",
                PortDirection::Output,
                PortInstances::UserCreated { min: 1, max: None },
            )?,
        ],
        vec![],
        vec![],
    )
}

fn loop_protocol() -> Result<NodeProtocol, BuiltinAssemblyError> {
    protocol(
        "yssbi.control.loop",
        vec![
            control_port("enter", PortDirection::Input, PortInstances::Declared)?,
            data_port(
                "condition",
                PortDirection::Input,
                TypeExpr::Concrete(sid("core.bool", TypeId::new)?),
                PortInstances::Declared,
            )?,
            data_port(
                "initial_source",
                PortDirection::Input,
                TypeExpr::Unknown,
                PortInstances::UserCreated { min: 0, max: None },
            )?,
            data_port(
                "next_source",
                PortDirection::Input,
                TypeExpr::Unknown,
                PortInstances::UserCreated { min: 0, max: None },
            )?,
            data_port(
                "body_input",
                PortDirection::Output,
                TypeExpr::Unknown,
                PortInstances::UserCreated { min: 0, max: None },
            )?,
            data_port(
                "result",
                PortDirection::Output,
                TypeExpr::Unknown,
                PortInstances::UserCreated { min: 0, max: None },
            )?,
            control_port("body", PortDirection::Output, PortInstances::Declared)?,
            control_port("then", PortDirection::Output, PortInstances::Declared)?,
        ],
        vec![parameter(
            "max_iterations",
            TypeExpr::Concrete(sid("core.int64", TypeId::new)?),
            ParameterEditorSpec::Number,
            vec![
                ParameterConstraint::Required,
                ParameterConstraint::IntegerRange {
                    min: Some(1),
                    max: None,
                },
            ],
        )?],
        vec![member_group(
            &["initial_source", "body_input", "next_source", "result"],
            1,
            None,
        )?],
    )
}

fn protocol(
    id: &'static str,
    ports: Vec<PortSpec>,
    parameters: Vec<ParameterSpec>,
    member_groups: Vec<PortMemberGroupSpec>,
) -> Result<NodeProtocol, BuiltinAssemblyError> {
    Ok(NodeProtocol {
        type_id: sid(id, NodeTypeId::new)?,
        catalog: NodeCatalogProtocol {
            title_key: iid(Box::leak(format!("nodes.{id}.title").into_boxed_str()))?,
            description_key: Some(iid(Box::leak(
                format!("nodes.{id}.description").into_boxed_str(),
            ))?),
            documentation_key: Some(iid(Box::leak(
                format!("nodes.{id}.documentation").into_boxed_str(),
            ))?),
            aliases_key: Some(iid(Box::leak(
                format!("nodes.{id}.aliases").into_boxed_str(),
            ))?),
            category_id: sid("control", NodeCategoryId::new)?,
            icon_id: sid("builtin.control", IconId::new)?,
            style_id: sid("builtin.default", NodeStyleId::new)?,
            hidden: false,
        },
        interface: assembled_interface(id, ports, vec![], vec![], member_groups)?,
        parameters: assembled_parameters(id, parameters)?,
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            purity: Purity::Effectful,
            evaluation: EvaluationPolicy::EagerWhenRegionEntered,
            cache: CachePolicy::Disabled,
            effects: EffectSemantics::Ordered,
        },
        scope: NodeScope::Any,
        managed_role: None,
    })
}

fn control_port(
    key: &'static str,
    direction: PortDirection,
    instances: PortInstances,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(
        key,
        direction,
        PortKind::Control,
        TypeExpr::Unknown,
        instances,
    )
}

fn data_port(
    key: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
    instances: PortInstances,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(key, direction, PortKind::Data, value_type, instances)
}

fn port(
    key: &'static str,
    direction: PortDirection,
    kind: PortKind,
    value_type: TypeExpr,
    instances: PortInstances,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: sid(key, PortKey::new)?,
        label_key: iid(Box::leak(format!("ports.{key}.label").into_boxed_str()))?,
        direction,
        kind,
        value_type,
        instances,
        connections: ConnectionsPerPort::Single,
        input_binding: (kind == PortKind::Data && direction == PortDirection::Input).then_some(
            InputBindingSpec {
                literal_policy: LiteralPolicy::Allowed,
                default_value: None,
            },
        ),
        consumption: None,
        production: (kind == PortKind::Data && direction == PortDirection::Output)
            .then_some(OutputProduction::FullyMaterialized),
        editor: PortEditorSpec::Default,
        schema: None,
    })
}

fn member_group(
    templates: &[&'static str],
    min: u16,
    max: Option<u16>,
) -> Result<PortMemberGroupSpec, BuiltinAssemblyError> {
    Ok(PortMemberGroupSpec {
        templates: templates
            .iter()
            .map(|template| sid(*template, PortKey::new))
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice(),
        min,
        max,
    })
}

fn parameter(
    key: &'static str,
    value_type: TypeExpr,
    editor: ParameterEditorSpec,
    constraints: Vec<ParameterConstraint>,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    Ok(ParameterSpec {
        key: sid(key, ParameterKey::new)?,
        title_key: iid(Box::leak(
            format!("parameters.{key}.title").into_boxed_str(),
        ))?,
        description_key: Some(iid(Box::leak(
            format!("parameters.{key}.description").into_boxed_str(),
        ))?),
        value_type,
        default_value: None,
        constraints,
        editor,
    })
}

fn add_messages(out: &mut Vec<(&'static str, &'static str, Message)>) {
    for (key, en, zh) in [
        ("ports.body.label", "Body", "循环体"),
        ("ports.then_source.label", "Then Source", "真分支来源"),
        ("ports.else_source.label", "Else Source", "假分支来源"),
        ("ports.initial_source.label", "Initial Source", "初始来源"),
        ("ports.next_source.label", "Next Source", "下一值来源"),
        ("ports.body_input.label", "Body Input", "循环体输入"),
        (
            "parameters.max_iterations.title",
            "Maximum Iterations",
            "最大迭代次数",
        ),
        (
            "parameters.max_iterations.description",
            "Positive safety limit for loop iterations.",
            "循环迭代次数的正整数安全上限。",
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
        (
            "en-US",
            description,
            Text("Defines explicit structured control flow."),
        ),
        ("zh-CN", description, Text("定义显式的结构化控制流。")),
        (
            "en-US",
            documentation,
            Text("The compiler lowers this node as a structured control region."),
        ),
        (
            "zh-CN",
            documentation,
            Text("编译器将此节点降低为结构化控制区域。"),
        ),
        ("en-US", aliases, Aliases(en_aliases)),
        ("zh-CN", aliases, Aliases(zh_aliases)),
    ]);
}

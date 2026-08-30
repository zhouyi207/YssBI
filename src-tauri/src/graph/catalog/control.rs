use super::builtin::{BuiltinAssemblyError, assembled_interface, assembled_parameters, iid, sid};
use crate::graph::catalog::{Aliases, Message, Text};
use crate::graph::registry::{RegisteredNode, StructuralNodeRole};
use std::sync::Arc;
use yss_graph_protocol::*;

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
    let bool_type = TypeExpr::Concrete(sid("core.bool", TypeId::new)?);
    let mut condition = data_port(
        "condition",
        "Condition",
        PortDirection::Input,
        bool_type.clone(),
        PortInstances::Declared,
    )?;
    condition.input_binding = Some(InputBindingSpec {
        literal_policy: LiteralPolicy::Allowed,
        default_value: Some(TypedValue {
            value_type: bool_type,
            value: Value::Bool(true),
        }),
    });
    protocol(
        "yssbi.control.branch",
        vec![
            control_port(
                "enter",
                "Enter",
                PortDirection::Input,
                PortInstances::Declared,
            )?,
            condition,
            data_port(
                "then_source",
                "Then Source",
                PortDirection::Input,
                TypeExpr::Unknown,
                PortInstances::UserCreated { min: 0, max: None },
            )?,
            data_port(
                "else_source",
                "Else Source",
                PortDirection::Input,
                TypeExpr::Unknown,
                PortInstances::UserCreated { min: 0, max: None },
            )?,
            control_port(
                "true",
                "True",
                PortDirection::Output,
                PortInstances::Declared,
            )?,
            control_port(
                "false",
                "False",
                PortDirection::Output,
                PortInstances::Declared,
            )?,
            data_port(
                "result",
                "Result",
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
            control_port(
                "enter",
                "Enter",
                PortDirection::Input,
                PortInstances::Declared,
            )?,
            control_port(
                "then",
                "Then",
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
            control_port(
                "enter",
                "Enter",
                PortDirection::Input,
                PortInstances::Declared,
            )?,
            data_port(
                "condition",
                "Condition",
                PortDirection::Input,
                TypeExpr::Concrete(sid("core.bool", TypeId::new)?),
                PortInstances::Declared,
            )?,
            data_port(
                "initial_source",
                "Initial Source",
                PortDirection::Input,
                TypeExpr::Unknown,
                PortInstances::UserCreated { min: 0, max: None },
            )?,
            data_port(
                "next_source",
                "Next Source",
                PortDirection::Input,
                TypeExpr::Unknown,
                PortInstances::UserCreated { min: 0, max: None },
            )?,
            data_port(
                "body_input",
                "Body Input",
                PortDirection::Output,
                TypeExpr::Unknown,
                PortInstances::UserCreated { min: 0, max: None },
            )?,
            data_port(
                "result",
                "Result",
                PortDirection::Output,
                TypeExpr::Unknown,
                PortInstances::UserCreated { min: 0, max: None },
            )?,
            control_port(
                "body",
                "Body",
                PortDirection::Output,
                PortInstances::Declared,
            )?,
            control_port(
                "then",
                "Then",
                PortDirection::Output,
                PortInstances::Declared,
            )?,
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
        instance_display: NodeInstanceDisplaySpec::Static,
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            purity: Purity::Effectful,
            evaluation: EvaluationPolicy::EagerWhenRegionEntered,
            cache: CachePolicy::Disabled,
            effects: EffectSemantics::Ordered,
            idempotent: false,
            retry: None,
        },
        scope: NodeScope::Any,
        managed_role: None,
    })
}

fn control_port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
    instances: PortInstances,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(
        key,
        title,
        direction,
        PortKind::Control,
        TypeExpr::Unknown,
        instances,
    )
}

fn data_port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
    instances: PortInstances,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(key, title, direction, PortKind::Data, value_type, instances)
}

fn port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
    kind: PortKind,
    value_type: TypeExpr,
    instances: PortInstances,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: sid(key, PortKey::new)?,
        title: title.into(),
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
        presentation: ParameterPresentation::DetailPanel,
    })
}

fn add_messages(out: &mut Vec<(&'static str, &'static str, Message)>) {
    for (key, en, zh) in [
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
    let documentation: &'static str =
        Box::leak(format!("nodes.{id}.documentation").into_boxed_str());
    let aliases: &'static str = Box::leak(format!("nodes.{id}.aliases").into_boxed_str());
    out.extend([
        ("en-US", title, Text(en)),
        ("zh-CN", title, Text(zh)),
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

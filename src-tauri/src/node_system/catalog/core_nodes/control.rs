use super::support::*;
use crate::node_system::protocol::*;
use crate::node_system::registry::StructuralNodeRole;

pub(super) fn register(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    register_do(fragment)?;
    register_merge(fragment)?;
    register_sleep(fragment)?;
    Ok(())
}

fn register_do(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    const ID: &str = "yssbi.control.do";
    fragment.add_node_messages(&NodeTextSpec {
        id: ID,
        title: "Do",
        zh_title: "执行",
        documentation: "The no-op kernel preserves an explicit sequencing point in the execution plan.",
        zh_documentation: "空操作 kernel 在执行计划中保留显式顺序点。",
        aliases: &["do", "no-op", "noop", "sequencing point"],
        zh_aliases: &["执行", "空操作", "顺序点"],
    })?;
    fragment.nodes.push(leaf(
        protocol(
            ID,
            "control",
            vec![
                control_port(
                    "enter",
                    "Enter",
                    PortDirection::Input,
                    PortInstances::Declared,
                )?,
                effect_port("effect_in", "Effect In", PortDirection::Input)?,
                control_port(
                    "then",
                    "Then",
                    PortDirection::Output,
                    PortInstances::Declared,
                )?,
                effect_port("effect_out", "Effect Out", PortDirection::Output)?,
            ],
            vec![],
            vec![],
            vec![],
            effectful(),
        )?,
        ID,
    ));
    Ok(())
}

fn register_merge(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    const ID: &str = "yssbi.control.merge";
    fragment.add_node_messages(&NodeTextSpec {
        id: ID,
        title: "Merge Control Flow",
        zh_title: "合并控制流",
        documentation: "Incoming ports have stable instance identities and preserve deterministic document order.",
        zh_documentation: "输入端口具有稳定实例身份，并保持确定性的文档顺序。",
        aliases: &["merge", "join", "control join", "converge"],
        zh_aliases: &["合并", "汇合", "控制流汇合"],
    })?;
    fragment.nodes.push(structural(
        protocol(
            ID,
            "control",
            vec![
                control_port(
                    "enter",
                    "Enter",
                    PortDirection::Input,
                    PortInstances::UserCreated {
                        min: 2,
                        max: Some(8),
                    },
                )?,
                control_port(
                    "then",
                    "Then",
                    PortDirection::Output,
                    PortInstances::Declared,
                )?,
            ],
            vec![],
            vec![],
            vec![],
            effectful(),
        )?,
        StructuralNodeRole::Sequence,
    ));
    Ok(())
}

fn register_sleep(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    const ID: &str = "yssbi.control.sleep";
    fragment.add_node_messages(&NodeTextSpec {
        id: ID,
        title: "Sleep",
        zh_title: "等待",
        documentation: "Duration is measured in seconds and must be between zero and sixty. Cancellation is checked while waiting.",
        zh_documentation: "时长以秒为单位，范围为零到六十；等待期间会检查取消状态。",
        aliases: &["sleep", "wait", "delay", "seconds"],
        zh_aliases: &["等待", "延迟", "秒"],
    })?;
    let mut duration = data_port(
        "duration",
        "Duration (seconds)",
        PortDirection::Input,
        concrete("core.float64")?,
    )?;
    duration.input_binding = Some(InputBindingSpec {
        literal_policy: LiteralPolicy::Allowed,
        default_value: Some(TypedValue {
            value_type: concrete("core.float64")?,
            value: Value::Decimal(assembled_decimal(ID, "1")?),
        }),
    });
    fragment.nodes.push(leaf(
        protocol(
            ID,
            "control",
            vec![
                control_port(
                    "enter",
                    "Enter",
                    PortDirection::Input,
                    PortInstances::Declared,
                )?,
                effect_port("effect_in", "Effect In", PortDirection::Input)?,
                duration,
                control_port(
                    "then",
                    "Then",
                    PortDirection::Output,
                    PortInstances::Declared,
                )?,
                effect_port("effect_out", "Effect Out", PortDirection::Output)?,
            ],
            vec![],
            vec![],
            vec![],
            effectful(),
        )?,
        ID,
    ));
    Ok(())
}

fn effect_port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: semantic(key, PortKey::new)?,
        title: title.into(),
        direction,
        kind: PortKind::Effect,
        value_type: TypeExpr::Unknown,
        instances: PortInstances::Declared,
        connections: if direction == PortDirection::Input {
            ConnectionsPerPort::Single
        } else {
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            }
        },
        input_binding: None,
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    })
}

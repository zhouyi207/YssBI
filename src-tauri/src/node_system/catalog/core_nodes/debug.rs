use super::support::*;
use crate::node_system::protocol::*;

pub(super) fn register(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    register_print(fragment)?;
    register_view(fragment)?;
    Ok(())
}

fn register_print(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    const ID: &str = "yssbi.debug.print";
    fragment.add_node_messages(&NodeTextSpec {
        id: ID,
        title: "Print",
        zh_title: "打印",
        description: "Writes a string to the run log as an ordered effect.",
        zh_description: "将字符串作为有序副作用写入运行日志。",
        documentation: "The message is emitted through the runtime logging backend and is correlated with the current run and activation.",
        zh_documentation: "消息通过运行时日志后端发出，并关联当前 run 与 activation。",
        aliases: &["print", "log", "debug output", "message"],
        zh_aliases: &["打印", "日志", "调试输出", "消息"],
    })?;
    add_port_messages(
        fragment,
        ID,
        &[
            ("enter", "Enter", "进入"),
            ("message", "Message", "消息"),
            ("then", "Then", "然后"),
        ],
    )?;
    let mut message = data_port(
        ID,
        "message",
        PortDirection::Input,
        concrete("core.string")?,
    )?;
    message.input_binding = Some(InputBindingSpec {
        literal_policy: LiteralPolicy::Allowed,
        default_value: Some(TypedValue {
            value_type: concrete("core.string")?,
            value: Value::String("Hello, World!".into()),
        }),
    });
    fragment.nodes.push(leaf(
        protocol(
            ID,
            "debug",
            vec![
                control_port(ID, "enter", PortDirection::Input, PortInstances::Declared)?,
                message,
                control_port(ID, "then", PortDirection::Output, PortInstances::Declared)?,
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

fn register_view(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    const ID: &str = "yssbi.debug.view";
    fragment.add_node_messages(&NodeTextSpec {
        id: ID,
        title: "View Data",
        zh_title: "查看数据",
        description: "Captures an immutable, replayable snapshot for a result inspector.",
        zh_description: "为结果查看器捕获不可变、可重放的快照。",
        documentation: "The runtime returns a snapshot artifact; opening and paging the inspector remains a UI concern.",
        zh_documentation: "运行时返回快照 artifact；打开查看器及分页仍由 UI 负责。",
        aliases: &["view", "inspect", "preview", "snapshot", "data inspector"],
        zh_aliases: &["查看", "检查", "预览", "快照", "数据查看器"],
    })?;
    add_port_messages(
        fragment,
        ID,
        &[
            ("enter", "Enter", "进入"),
            ("data", "Data", "数据"),
            ("snapshot", "Snapshot", "快照"),
            ("then", "Then", "然后"),
        ],
    )?;
    let value_type = TypeParameterId::new("value").map_err(|source| {
        BuiltinAssemblyError::InvalidSemanticId {
            value: "value".into(),
            source,
        }
    })?;
    fragment.nodes.push(leaf(
        protocol(
            ID,
            "debug",
            vec![
                control_port(ID, "enter", PortDirection::Input, PortInstances::Declared)?,
                data_port(
                    ID,
                    "data",
                    PortDirection::Input,
                    TypeExpr::Generic(value_type.clone()),
                )?,
                data_port(
                    ID,
                    "snapshot",
                    PortDirection::Output,
                    TypeExpr::Generic(value_type.clone()),
                )?,
                control_port(ID, "then", PortDirection::Output, PortInstances::Declared)?,
            ],
            vec![value_type],
            vec![],
            vec![],
            effectful(),
        )?,
        ID,
    ));
    Ok(())
}

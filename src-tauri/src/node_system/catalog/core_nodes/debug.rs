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
        documentation: "The runtime sends the message to the Output panel with its run, source graph, and source node identity. Program output is kept separate from diagnostic logs.",
        zh_documentation: "运行时将消息连同 run、来源图和来源节点标识发送到“输出”面板。程序输出与诊断日志相互独立。",
        aliases: &["print", "output", "debug output", "message"],
        zh_aliases: &["打印", "输出", "调试输出", "消息"],
    })?;
    let mut message = data_port(
        "message",
        "Message",
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
                control_port(
                    "enter",
                    "Enter",
                    PortDirection::Input,
                    PortInstances::Declared,
                )?,
                message,
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
        documentation: "After activation succeeds, the runtime requests an inspector for the input ResultId and continues through Then without copying or materializing data.",
        zh_documentation: "激活成功后，运行时请求查看输入 ResultId，并通过“然后”继续，不复制或物化数据。",
        aliases: &["view", "inspect", "preview", "data inspector"],
        zh_aliases: &["查看", "检查", "预览", "数据查看器"],
    })?;
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
                control_port(
                    "enter",
                    "Enter",
                    PortDirection::Input,
                    PortInstances::Declared,
                )?,
                data_port(
                    "data",
                    "Data",
                    PortDirection::Input,
                    TypeExpr::Generic(value_type.clone()),
                )?,
                control_port(
                    "then",
                    "Then",
                    PortDirection::Output,
                    PortInstances::Declared,
                )?,
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

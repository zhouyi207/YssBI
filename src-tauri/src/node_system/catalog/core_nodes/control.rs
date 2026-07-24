use super::support::*;
use crate::node_system::protocol::*;
use crate::node_system::registry::StructuralNodeRole;

pub(super) fn register(fragment: &mut ProviderFragment) {
    register_do(fragment);
    register_merge(fragment);
    register_sleep(fragment);
}

fn register_do(fragment: &mut ProviderFragment) {
    const ID: &str = "yssbi.control.do";
    fragment.add_node_messages(&NodeTextSpec {
        id: ID,
        title: "Do",
        zh_title: "执行",
        description: "Creates an explicit ordered control-flow step without changing data.",
        zh_description: "创建一个不改变数据的显式有序控制流步骤。",
        documentation: "The no-op kernel preserves an explicit sequencing point in the execution plan.",
        zh_documentation: "空操作 kernel 在执行计划中保留显式顺序点。",
        aliases: &["do", "no-op", "noop", "sequencing point"],
        zh_aliases: &["执行", "空操作", "顺序点"],
    });
    add_port_messages(
        fragment,
        ID,
        &[("enter", "Enter", "进入"), ("then", "Then", "然后")],
    );
    fragment.nodes.push(leaf(
        protocol(
            ID,
            "control",
            vec![
                control_port(ID, "enter", PortDirection::Input, PortInstances::Declared),
                control_port(ID, "then", PortDirection::Output, PortInstances::Declared),
            ],
            vec![],
            vec![],
            vec![],
            effectful(),
        ),
        ID,
    ));
}

fn register_merge(fragment: &mut ProviderFragment) {
    const ID: &str = "yssbi.control.merge";
    fragment.add_node_messages(&NodeTextSpec {
        id: ID,
        title: "Merge Control Flow",
        zh_title: "合并控制流",
        description: "Joins one of several incoming structured control paths into one continuation.",
        zh_description: "将多个结构化控制路径之一合并到单一后续路径。",
        documentation: "Incoming ports have stable instance identities and preserve deterministic document order.",
        zh_documentation: "输入端口具有稳定实例身份，并保持确定性的文档顺序。",
        aliases: &["merge", "join", "control join", "converge"],
        zh_aliases: &["合并", "汇合", "控制流汇合"],
    });
    add_port_messages(
        fragment,
        ID,
        &[("enter", "Enter", "进入"), ("then", "Then", "然后")],
    );
    fragment.nodes.push(structural(
        protocol(
            ID,
            "control",
            vec![
                control_port(
                    ID,
                    "enter",
                    PortDirection::Input,
                    PortInstances::UserCreated {
                        min: 2,
                        max: Some(8),
                    },
                ),
                control_port(ID, "then", PortDirection::Output, PortInstances::Declared),
            ],
            vec![],
            vec![],
            vec![],
            effectful(),
        ),
        StructuralNodeRole::Sequence,
    ));
}

fn register_sleep(fragment: &mut ProviderFragment) {
    const ID: &str = "yssbi.control.sleep";
    fragment.add_node_messages(&NodeTextSpec {
        id: ID,
        title: "Sleep",
        zh_title: "等待",
        description: "Waits for a bounded duration before continuing ordered execution.",
        zh_description: "等待受限时长后继续有序执行。",
        documentation: "Duration is measured in seconds and must be between zero and sixty. Cancellation is checked while waiting.",
        zh_documentation: "时长以秒为单位，范围为零到六十；等待期间会检查取消状态。",
        aliases: &["sleep", "wait", "delay", "seconds"],
        zh_aliases: &["等待", "延迟", "秒"],
    });
    add_port_messages(
        fragment,
        ID,
        &[
            ("enter", "Enter", "进入"),
            ("duration", "Duration (seconds)", "时长（秒）"),
            ("then", "Then", "然后"),
        ],
    );
    let mut duration = data_port(
        ID,
        "duration",
        PortDirection::Input,
        concrete("core.float64"),
    );
    duration.input_binding = Some(InputBindingSpec {
        literal_policy: LiteralPolicy::Allowed,
        default_value: Some(TypedValue {
            value_type: concrete("core.float64"),
            value: Value::Decimal(CanonicalDecimal::new("1").expect("canonical duration")),
        }),
    });
    fragment.nodes.push(leaf(
        protocol(
            ID,
            "control",
            vec![
                control_port(ID, "enter", PortDirection::Input, PortInstances::Declared),
                duration,
                control_port(ID, "then", PortDirection::Output, PortInstances::Declared),
            ],
            vec![],
            vec![],
            vec![],
            effectful(),
        ),
        ID,
    ));
}

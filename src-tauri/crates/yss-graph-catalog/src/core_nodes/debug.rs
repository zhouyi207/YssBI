use super::support::*;
use yss_graph_protocol::*;

pub(super) fn register(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    register_view(fragment)?;
    Ok(())
}

fn register_view(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    const ID: &str = "yssbi.debug.view";
    fragment.add_node_messages(&NodeTextSpec {
        id: ID,
        title: "View Data",
        zh_title: "查看数据",
        documentation: "Marks its data input as an explicit graph result that can be opened in the inspector without copying or materializing data.",
        zh_documentation: "将数据输入标记为可在检查器中打开的显式图结果，不复制或物化数据。",
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
            vec![data_port(
                "data",
                "Data",
                PortDirection::Input,
                TypeExpr::Generic(value_type.clone()),
            )?],
            vec![value_type],
            vec![],
            vec![],
            pure(),
        )?,
        ID,
    ));
    Ok(())
}

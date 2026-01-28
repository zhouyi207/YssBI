use crate::nodes::definition::NodeDefinition;
use crate::nodes::types::PinDefinition;
use serde_json::Value;

pub fn get_nodes() -> Vec<NodeDefinition> {
    vec![
        create_call_function(),
        create_call_macro(),
    ]
}

fn create_call_function() -> NodeDefinition {
    NodeDefinition {
        node_type: "call_function".into(),
        category: vec!["Function".into()],
        title: "Call Function".into(),
        ui_style: "default".into(),
        description: Some("Call a defined function".into()),
        inputs: vec![PinDefinition {
            name: "In".into(),
            pin_type: "exec".into(),
            default_value: None,
            is_array: false,
        }],
        outputs: vec![PinDefinition {
            name: "Out".into(),
            pin_type: "exec".into(),
            default_value: None,
            is_array: false,
        }],
        data_processor: Some(|_ctx, _node, _pin_id| Value::Null),
        flow_processor: Some(|ctx, node| {
            let sub_graph_id = node.sub_graph_id.as_ref().ok_or("Missing subGraphId")?;
            let sub_graph_id_clone = sub_graph_id.clone();
            let node_title = node.title.clone();
            let node_id = node.id.clone();

            // 找到入口节点
            let entry_node_id = ctx
                .find_node_by(&|n| {
                    n.node_type == "function_entry"
                        && (n.sub_graph_id.as_ref() == Some(&sub_graph_id_clone)
                            || n.title == node_title)
                })
                .ok_or(format!(
                    "Function entry not found for subGraphId {}",
                    sub_graph_id
                ))?;

            ctx.push_call_stack(node_id);
            ctx.log(format!("Calling function: {}", node.title));
            ctx.run_flow(&entry_node_id, "Then")?;
            ctx.pop_call_stack();
            ctx.log(format!("Returned from function: {}", node.title));

            Ok("Out".to_string())
        }),
    }
}

fn create_call_macro() -> NodeDefinition {
    NodeDefinition {
        node_type: "call_macro".into(),
        category: vec!["Macro".into()],
        title: "Call Macro".into(),
        ui_style: "default".into(),
        description: Some("Call a defined macro".into()),
        inputs: vec![PinDefinition {
            name: "In".into(),
            pin_type: "exec".into(),
            default_value: None,
            is_array: false,
        }],
        outputs: vec![PinDefinition {
            name: "Out".into(),
            pin_type: "exec".into(),
            default_value: None,
            is_array: false,
        }],
        data_processor: None,
        flow_processor: Some(|ctx, node| {
            let sub_graph_id = node.sub_graph_id.as_ref().ok_or("Missing subGraphId")?;
            let sub_graph_id_clone = sub_graph_id.clone();
            let node_title = node.title.clone();
            let node_id = node.id.clone();

            let entry_node_id = ctx
                .find_node_by(&|n| {
                    n.node_type == "macro_inputs"
                        && (n.sub_graph_id.as_ref() == Some(&sub_graph_id_clone)
                            || n.title == node_title)
                })
                .ok_or("Macro entry not found")?;

            ctx.push_call_stack(node_id);
            ctx.run_flow(&entry_node_id, "In")?;
            ctx.pop_call_stack();

            Ok("Out".to_string())
        }),
    }
}

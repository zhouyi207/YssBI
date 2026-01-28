use crate::executor::node::definition::NodeDefinition;
use crate::executor::node::data::PinDefinition;

pub fn get_nodes() -> Vec<NodeDefinition> {
    vec![
        create_plot(),
    ]
}

fn create_plot() -> NodeDefinition {
    NodeDefinition {
        node_type: "plot".into(),
        category: vec!["Visualization".into()],
        title: "Plot".into(),
        ui_style: "default".into(),
        description: Some("Open a new plot window for data visualization".into()),
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
            let label = format!("plot-{}", node.id);
            let title = format!("Plot - {}", node.title);
            let url = "index.html#/plot".to_string();
            
            match ctx.open_window(label.clone(), title, url) {
                Ok(_) => {
                    ctx.log(format!("[Plot] Window '{}' opened successfully", label));
                    Ok("Out".to_string())
                }
                Err(e) => {
                    ctx.log(format!("[Error] Failed to open plot window: {}", e));
                    Err(e)
                }
            }
        }),
    }
}

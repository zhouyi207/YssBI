use crate::nodes::definition::NodeDefinition;
use crate::nodes::types::PinDefinition;

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
            // 生成唯一的窗口标签
            let label = format!("plot-{}", node.id);
            let title = format!("Plot - {}", node.title);
            
            // 打开新窗口显示 plot 页面
            // 这里使用 index.html，你可以创建一个专门的 plot.html
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

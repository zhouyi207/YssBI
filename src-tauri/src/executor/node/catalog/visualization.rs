use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericOutExecPin, GenericInExecPin};

pub fn register(registry: &NodeRegistry) {
    let plot = GenericNode::new_prototype("plot", "Plot");
    plot.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    plot.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));
    
    plot.set_flow_processor(Box::new(|ctx, _node| {
        // 使用高精度时间戳和随机数创建唯一的窗口标签
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(); // 使用纳秒精度
        
        // 添加随机数确保唯一性
        let random_id = uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("0").to_string();
        let window_label = format!("plot_window_{}_{}", timestamp, random_id);
        
        ctx.log(format!("Plot node executing: Creating window with label: {}", window_label));
        
        // 添加小延迟避免快速连续创建窗口时的冲突
        std::thread::sleep(std::time::Duration::from_millis(20));
        
        // 异步创建窗口，不阻塞主线程
        match ctx.open_window_async(window_label.clone(), "Data Plot".into(), "#/plot".into()) {
            Ok(_) => {
                ctx.log(format!("Plot window '{}' creation initiated successfully", window_label));
                Ok("Out".into())
            }
            Err(e) => {
                ctx.log(format!("Failed to create plot window '{}': {}", window_label, e));
                // 即使窗口创建失败，也继续执行流程，不阻塞整个执行
                ctx.log("Continuing execution despite window creation failure".to_string());
                Ok("Out".into())
            }
        }
    }));
    
    let mut plot = plot;
    plot.set_metadata(vec!["Visualization".into()], "default".into(), Some("Open a new plot window for data visualization".into()));
    registry.register("plot".into(), Arc::new(plot));
}

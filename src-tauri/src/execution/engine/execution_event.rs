use crate::execution::{Presentation, SourceDescriptor};
use serde::Serialize;

/// 执行事件（通过 Tauri Channel 流式发送到前端）
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum ExecutionEvent {
    /// 执行开始
    ExecutionStart,

    /// 执行完成
    #[serde(rename_all = "camelCase")]
    ExecutionComplete { has_error: bool },

    /// 节点开始执行
    #[serde(rename_all = "camelCase")]
    NodeStart { node_id: String },

    /// 节点执行完成
    #[serde(rename_all = "camelCase")]
    NodeComplete {
        node_id: String,
        /// 后端计算耗时（毫秒），用于性能分析
        duration_ms: u64,
    },

    /// 节点执行出错
    #[serde(rename_all = "camelCase")]
    NodeError {
        node_id: String,
        error: String,
        /// 后端计算耗时（毫秒），用于性能分析
        duration_ms: u64,
    },

    /// 数据取数：消费者节点开始执行时声明依赖的 data 输入连线。
    #[serde(rename_all = "camelCase")]
    ConnectionActive {
        from_pin_id: String,
        to_pin_id: String,
    },

    /// 数据流动：上游产出可用值后，值沿 output→input 连线传向消费者。
    #[serde(rename_all = "camelCase")]
    ConnectionFlow {
        from_pin_id: String,
        to_pin_id: String,
    },

    /// 请求前端打开新窗口（source 已注册，窗口只消费 sourceId）
    #[serde(rename_all = "camelCase")]
    OpenSourceWindow {
        source_id: String,
        presentation: Presentation,
        window_title: String,
    },

    /// 数据输出 pin 已注册为可检查 source。
    #[serde(rename_all = "camelCase")]
    PinResultReady {
        graph_id: String,
        node_id: String,
        pin_id: String,
        source_id: String,
        descriptor: SourceDescriptor,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::Presentation;

    #[test]
    fn open_source_window_serializes_for_channel() {
        let event = ExecutionEvent::OpenSourceWindow {
            source_id: "window_test".into(),
            presentation: Presentation::Inspector,
            window_title: "View: (null)".into(),
        };
        let json = serde_json::to_value(&event).expect("serialize");
        assert_eq!(json["event"], "openSourceWindow");
        assert_eq!(json["data"]["sourceId"], "window_test");
        assert_eq!(json["data"]["presentation"]["kind"], "inspector");
        assert_eq!(json["data"]["windowTitle"], "View: (null)");
    }
}

use serde::Serialize;

/// 执行事件（通过 Tauri Channel 流式发送到前端）
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum ExecutionEvent {
    /// 执行开始
    ExecutionStart,

    /// 执行完成
    #[serde(rename_all = "camelCase")]
    ExecutionComplete {
        has_error: bool,
    },

    /// 节点开始执行
    #[serde(rename_all = "camelCase")]
    NodeStart {
        node_id: String,
    },

    /// 节点执行完成
    #[serde(rename_all = "camelCase")]
    NodeComplete {
        node_id: String,
    },

    /// 节点执行出错
    #[serde(rename_all = "camelCase")]
    NodeError {
        node_id: String,
        error: String,
    },

    /// 连接激活（数据/控制流经过该连接）
    #[serde(rename_all = "camelCase")]
    ConnectionActive {
        from_pin_id: String,
        to_pin_id: String,
    },
}

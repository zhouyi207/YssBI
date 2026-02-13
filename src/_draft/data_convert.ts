// ==================== 后端数据结构转换 ====================

// 注意：后端 Rust 使用 #[serde(rename = "type")]，所以 JSON 字段名是 "type"
// 前端和后端的字段名在 JSON 层面是一致的，不需要转换

/**
 * 将前端 SubGraphData 转换为后端格式
 * 目前前后端使用相同的 JSON 字段名，直接返回
 */
function toBackendSubGraphData(data: SubGraphData): SubGraphData {
    return data;
}
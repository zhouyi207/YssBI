//! 节点定义模块
//!
//! 定义 NodeDefinition 结构体及其序列化逻辑。

use crate::executor::{DataProcessor, FlowProcessor};
use super::data::PinDefinition;
use serde::{Deserialize, Serialize};

/// 节点元数据定义
#[derive(Clone)]
pub struct NodeDefinition {
    /// 节点类型标识符
    pub node_type: String,
    /// 所属分类 (层级路径)
    pub category: Vec<String>,
    /// 显示标题
    pub title: String,
    /// 输入针脚定义
    pub inputs: Vec<PinDefinition>,
    /// 输出针脚定义
    pub outputs: Vec<PinDefinition>,
    /// UI 样式
    pub ui_style: String,
    /// 节点描述
    pub description: Option<String>,
    /// 数据处理器
    pub data_processor: Option<DataProcessor>,
    /// 流程处理器
    pub flow_processor: Option<FlowProcessor>,
}

impl Serialize for NodeDefinition {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct NodeDefProxy<'a> {
            node_type: &'a String,
            category: &'a Vec<String>,
            title: &'a String,
            inputs: &'a Vec<PinDefinition>,
            outputs: &'a Vec<PinDefinition>,
            ui_style: &'a String,
            description: &'a Option<String>,
        }
        let proxy = NodeDefProxy {
            node_type: &self.node_type,
            category: &self.category,
            title: &self.title,
            inputs: &self.inputs,
            outputs: &self.outputs,
            ui_style: &self.ui_style,
            description: &self.description,
        };
        proxy.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NodeDefinition {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Err(serde::de::Error::custom(
            "NodeDefinition cannot be deserialized",
        ))
    }
}

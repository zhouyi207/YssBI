//! 节点注册中心
//!
//! 存放所有内置节点的定义。

use super::catalog;
use super::definition::NodeDefinition;

/// 获取所有节点定义
pub fn get_all_node_definitions() -> Vec<NodeDefinition> {
    catalog::get_all_builtin_nodes()
}

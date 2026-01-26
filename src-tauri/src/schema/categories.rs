//! 节点分类定义模块
//!
//! 定义所有节点分类及其显示属性。

use serde::{Deserialize, Serialize};

/// 节点分类定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryDefinition {
    /// 分类标识符 (如 "Internal", "Math", "Flow")
    pub name: String,
    /// 显示名称
    pub display_name: String,
    /// 分类描述
    pub description: Option<String>,
    /// 排序权重 (越小越靠前)
    pub sort_order: i32,
    /// 分类颜色 (用于 UI 高亮)
    pub color: Option<String>,
    /// 图标名称 (可选)
    pub icon: Option<String>,
    /// 是否在节点面板中显示
    pub visible_in_palette: bool,
}

/// 获取所有分类定义
pub fn get_category_definitions() -> Vec<CategoryDefinition> {
    vec![
        CategoryDefinition {
            name: "Internal".into(),
            display_name: "内部".into(),
            description: Some("系统内部节点，不在面板中显示".into()),
            sort_order: 0,
            color: Some("#666666".into()),
            icon: Some("lock".into()),
            visible_in_palette: false,
        },
        CategoryDefinition {
            name: "Event".into(),
            display_name: "事件".into(),
            description: Some("事件触发节点".into()),
            sort_order: 10,
            color: Some("#CC3333".into()),
            icon: Some("bolt".into()),
            visible_in_palette: true,
        },
        CategoryDefinition {
            name: "Flow".into(),
            display_name: "流程控制".into(),
            description: Some("控制程序执行流程".into()),
            sort_order: 20,
            color: Some("#FFFFFF".into()),
            icon: Some("git-branch".into()),
            visible_in_palette: true,
        },
        CategoryDefinition {
            name: "Branch".into(),
            display_name: "分支".into(),
            description: Some("条件分支节点".into()),
            sort_order: 25,
            color: Some("#FFFFFF".into()),
            icon: Some("git-branch".into()),
            visible_in_palette: true,
        },
        CategoryDefinition {
            name: "Math".into(),
            display_name: "数学".into(),
            description: Some("数学运算节点".into()),
            sort_order: 30,
            color: Some("#9ECD4D".into()),
            icon: Some("calculator".into()),
            visible_in_palette: true,
        },
        CategoryDefinition {
            name: "String".into(),
            display_name: "字符串".into(),
            description: Some("字符串操作节点".into()),
            sort_order: 40,
            color: Some("#FF00FF".into()),
            icon: Some("text".into()),
            visible_in_palette: true,
        },
        CategoryDefinition {
            name: "Variable".into(),
            display_name: "变量".into(),
            description: Some("变量读写节点".into()),
            sort_order: 50,
            color: Some("#0D7EA6".into()),
            icon: Some("database".into()),
            visible_in_palette: true,
        },
        CategoryDefinition {
            name: "Function".into(),
            display_name: "函数".into(),
            description: Some("函数调用节点".into()),
            sort_order: 60,
            color: Some("#0055FF".into()),
            icon: Some("function".into()),
            visible_in_palette: true,
        },
        CategoryDefinition {
            name: "Macro".into(),
            display_name: "宏".into(),
            description: Some("宏调用节点".into()),
            sort_order: 70,
            color: Some("#AAAAAA".into()),
            icon: Some("package".into()),
            visible_in_palette: true,
        },
        CategoryDefinition {
            name: "Debug".into(),
            display_name: "调试".into(),
            description: Some("调试工具节点".into()),
            sort_order: 100,
            color: Some("#FFCC00".into()),
            icon: Some("bug".into()),
            visible_in_palette: true,
        },
    ]
}

/// 根据名称获取分类定义
pub fn get_category_by_name(name: &str) -> Option<CategoryDefinition> {
    get_category_definitions().into_iter().find(|c| c.name == name)
}

/// 获取所有在面板中可见的分类
pub fn get_visible_categories() -> Vec<CategoryDefinition> {
    get_category_definitions()
        .into_iter()
        .filter(|c| c.visible_in_palette)
        .collect()
}

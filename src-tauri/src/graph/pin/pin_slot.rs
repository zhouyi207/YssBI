//! Pin Slot - 节点 pin 接口的声明式描述
//!
//! PinSlot 取代了原先基于闭包的 `pin_generator`，
//! 将节点的 pin 布局完全表达为可序列化的数据结构。

use super::{PinDataTypeDefinition, PinDefinition, PinDirection, PinKind, PinRole};
use serde::{Deserialize, Serialize};

/// 节点 pin 槽位的声明式描述
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "slotKind", rename_all = "camelCase")]
pub enum PinSlot {
    /// 固定 pin，始终存在于节点上
    Fixed { pin: PinDefinition },

    /// 可重复 pin 槽位，用户可以增删同类型的 pin 实例
    ///
    /// 例如 Add 节点的可变操作数、Sequence 的可变步骤数
    Repeatable {
        /// pin 模板，role 必须是可索引的家族（Operands, Inputs, Outputs, Steps）
        template: PinDefinition,
        /// 名称前缀，为空时使用字母命名 (A, B, C...)，否则使用 "{prefix} {index+1}"
        name_prefix: String,
        /// 初始/最小 pin 数量
        min_count: usize,
        /// 最大 pin 数量，None 表示无限制
        max_count: Option<usize>,
    },

    /// 从上游输入数据 schema 派生的 pin（运行时创建）
    ///
    /// 例如 Decompose DataFrame 根据连接的 DataFrame 列结构动态生成输出 pin
    DerivedFromInput {
        /// 驱动派生的源输入 pin 的角色
        source_role: PinRole,
        /// 派生 pin 的方向（通常为 Output）
        direction: PinDirection,
        /// 派生 pin 的基础类型模式（用于兼容性过滤）
        base_type: PinDataTypeDefinition,
    },
}

impl PinSlot {
    // ======================== 构造器 ========================

    pub fn fixed(pin: PinDefinition) -> Self {
        PinSlot::Fixed { pin }
    }

    pub fn repeatable(
        template: PinDefinition,
        name_prefix: impl Into<String>,
        min_count: usize,
        max_count: Option<usize>,
    ) -> Self {
        PinSlot::Repeatable {
            template,
            name_prefix: name_prefix.into(),
            min_count,
            max_count,
        }
    }

    pub fn derived_from_input(
        source_role: PinRole,
        direction: PinDirection,
        base_type: PinDataTypeDefinition,
    ) -> Self {
        PinSlot::DerivedFromInput {
            source_role,
            direction,
            base_type,
        }
    }

    // ======================== 初始 pin 生成 ========================

    /// 生成该槽位的初始 pin 定义列表
    ///
    /// - Fixed: 返回单个 pin
    /// - Repeatable: 返回 min_count 个 pin（角色索引递增）
    /// - DerivedFromInput: 返回空（运行时由 resolver 创建）
    pub fn generate_initial_pins(&self) -> Vec<PinDefinition> {
        match self {
            PinSlot::Fixed { pin } => vec![pin.clone()],

            PinSlot::Repeatable {
                template,
                name_prefix,
                min_count,
                ..
            } => (0..*min_count)
                .map(|i| {
                    let mut pin = template.clone();
                    if let Some(new_role) = template.role.with_index(i) {
                        pin.role = new_role;
                    }
                    pin.name = generate_slot_name(name_prefix, i);
                    pin
                })
                .collect(),

            PinSlot::DerivedFromInput { .. } => vec![],
        }
    }

    // ======================== 类型能力查询 ========================

    /// 获取该槽位能接受或产出的 pin 类型信息（用于兼容性过滤）
    pub fn type_capabilities(&self) -> Vec<PinTypeCapability> {
        match self {
            PinSlot::Fixed { pin } => {
                if let Some(dt) = &pin.data_type {
                    vec![PinTypeCapability {
                        direction: pin.direction,
                        kind: pin.kind,
                        data_type: dt.clone(),
                    }]
                } else {
                    vec![PinTypeCapability {
                        direction: pin.direction,
                        kind: pin.kind,
                        data_type: PinDataTypeDefinition::Unknown,
                    }]
                }
            }

            PinSlot::Repeatable { template, .. } => {
                if let Some(dt) = &template.data_type {
                    vec![PinTypeCapability {
                        direction: template.direction,
                        kind: template.kind,
                        data_type: dt.clone(),
                    }]
                } else {
                    vec![PinTypeCapability {
                        direction: template.direction,
                        kind: template.kind,
                        data_type: PinDataTypeDefinition::Unknown,
                    }]
                }
            }

            PinSlot::DerivedFromInput {
                direction,
                base_type,
                ..
            } => vec![PinTypeCapability {
                direction: *direction,
                kind: PinKind::Data,
                data_type: base_type.clone(),
            }],
        }
    }

    /// 该槽位是否支持动态 pin（Repeatable 或 DerivedFromInput）
    pub fn is_dynamic(&self) -> bool {
        !matches!(self, PinSlot::Fixed { .. })
    }

    /// 为 Repeatable 槽位生成指定索引的单个 pin 定义
    ///
    /// 仅对 `PinSlot::Repeatable` 有效，其他变体返回 None。
    pub fn generate_pin_at_index(&self, index: usize) -> Option<PinDefinition> {
        match self {
            PinSlot::Repeatable {
                template,
                name_prefix,
                max_count,
                ..
            } => {
                if let Some(max) = max_count {
                    if index >= *max {
                        return None;
                    }
                }
                let mut pin = template.clone();
                if let Some(new_role) = template.role.with_index(index) {
                    pin.role = new_role;
                }
                pin.name = generate_slot_name(name_prefix, index);
                Some(pin)
            }
            _ => None,
        }
    }

    /// 获取 Repeatable 槽位的模板角色（用于 family 匹配）
    pub fn repeatable_template_role(&self) -> Option<&PinRole> {
        match self {
            PinSlot::Repeatable { template, .. } => Some(&template.role),
            _ => None,
        }
    }

    /// 获取 Repeatable 槽位的最小 pin 数量
    pub fn repeatable_min_count(&self) -> Option<usize> {
        match self {
            PinSlot::Repeatable { min_count, .. } => Some(*min_count),
            _ => None,
        }
    }
}

/// pin 类型能力描述（用于前端过滤兼容节点）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinTypeCapability {
    pub direction: PinDirection,
    pub kind: PinKind,
    pub data_type: PinDataTypeDefinition,
}

/// 根据命名前缀和索引生成 pin 名称
fn generate_slot_name(prefix: &str, index: usize) -> String {
    if prefix.is_empty() {
        if index < 26 {
            String::from((b'A' + index as u8) as char)
        } else {
            format!("Pin {}", index)
        }
    } else {
        format!("{} {}", prefix, index + 1)
    }
}

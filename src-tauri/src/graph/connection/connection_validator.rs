//! 连接验证器
//!
//! 前端只发送两个 Pin ID，后端按验证链逐步检查，
//! 全部通过后返回已确定方向的 `ValidatedConnection`。

use crate::graph::core::GraphDataState;
use crate::graph::pin::{PinDirection, PinKind};
use crate::graph::value::DataType;
use crate::graph::PinId;
use std::fmt;

/// 验证通过后的连接描述（方向已确定）
#[derive(Debug, Clone)]
pub struct ValidatedConnection {
    /// Output pin（数据/执行 流出端）
    pub from_pin: PinId,
    /// Input pin（数据/执行 流入端）
    pub to_pin: PinId,
}

/// 连接错误（精确描述拒绝原因）
#[derive(Debug, Clone)]
pub enum ConnectionError {
    PinNotFound(PinId),
    SamePin,
    SameNode,
    SameDirection(PinDirection),
    KindMismatch { a: PinKind, b: PinKind },
    AlreadyConnected,
    TypeIncompatible { from: DataType, to: DataType },
    CycleDetected,
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PinNotFound(id) => write!(f, "Pin {:?} not found", id),
            Self::SamePin => write!(f, "Cannot connect a pin to itself"),
            Self::SameNode => write!(f, "Cannot connect pins on the same node"),
            Self::SameDirection(d) => write!(f, "Cannot connect two {:?} pins", d),
            Self::KindMismatch { a, b } => {
                write!(f, "Cannot connect {:?} pin to {:?} pin", a, b)
            }
            Self::AlreadyConnected => write!(f, "Connection already exists"),
            Self::TypeIncompatible { from, to } => {
                write!(f, "Type {} is not compatible with {}", from, to)
            }
            Self::CycleDetected => write!(f, "Connection would create a cycle"),
        }
    }
}

impl From<ConnectionError> for String {
    fn from(e: ConnectionError) -> Self {
        e.to_string()
    }
}

/// 对 `GraphDataState` 执行完整的连接验证链
///
/// 调用方持有 `data_state` 的读锁，将引用传入。
/// `pin_types` 提供推断后的 DataType 映射（来自 `GraphInstance`）。
pub fn validate_connection(
    data_state: &GraphDataState,
    pin_a: PinId,
    pin_b: PinId,
) -> Result<ValidatedConnection, ConnectionError> {
    // 1. Pin 存在性
    let inst_a = data_state
        .pins
        .get(&pin_a)
        .ok_or(ConnectionError::PinNotFound(pin_a))?;
    let inst_b = data_state
        .pins
        .get(&pin_b)
        .ok_or(ConnectionError::PinNotFound(pin_b))?;

    // 2. 同 Pin 自连
    if pin_a == pin_b {
        return Err(ConnectionError::SamePin);
    }

    // 3. 同节点
    if inst_a.node_id == inst_b.node_id {
        return Err(ConnectionError::SameNode);
    }

    // 4. 方向推断：确定 (output, input)
    let (from_pin, to_pin) = match (inst_a.definition.direction, inst_b.definition.direction) {
        (PinDirection::Output, PinDirection::Input) => (pin_a, pin_b),
        (PinDirection::Input, PinDirection::Output) => (pin_b, pin_a),
        (PinDirection::Input, PinDirection::Input) => {
            return Err(ConnectionError::SameDirection(PinDirection::Input));
        }
        (PinDirection::Output, PinDirection::Output) => {
            return Err(ConnectionError::SameDirection(PinDirection::Output));
        }
    };

    // 重新获取按方向确定后的实例引用
    let out_inst = data_state.pins.get(&from_pin).unwrap();
    let in_inst = data_state.pins.get(&to_pin).unwrap();

    // 5. Kind 兼容性（Data ↔ Data, Exec ↔ Exec）
    if out_inst.definition.kind != in_inst.definition.kind {
        return Err(ConnectionError::KindMismatch {
            a: out_inst.definition.kind,
            b: in_inst.definition.kind,
        });
    }

    // 6. 重复连接
    let downstream = data_state.connections.get_downstream(from_pin);
    if downstream.contains(&to_pin) {
        return Err(ConnectionError::AlreadyConnected);
    }

    // 7. 数据类型兼容性（仅 Data pin）
    if out_inst.definition.kind == PinKind::Data {
        let from_type = data_state
            .pin_types
            .get(&from_pin)
            .cloned()
            .unwrap_or(DataType::Any);
        let to_type = data_state
            .pin_types
            .get(&to_pin)
            .cloned()
            .unwrap_or(DataType::Any);

        if !to_type.can_accept(&from_type) && from_type != DataType::Any {
            return Err(ConnectionError::TypeIncompatible {
                from: from_type,
                to: to_type,
            });
        }
    }

    // 8. 环路检测
    if data_state.connections.would_create_cycle(from_pin, to_pin) {
        return Err(ConnectionError::CycleDetected);
    }

    Ok(ValidatedConnection { from_pin, to_pin })
}

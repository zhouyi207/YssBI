//! Pin 行为抽象

use std::any::Any;
use std::fmt::Debug;
use crate::executor::error::{ExecutionResult, NodeResult};
use crate::executor::types::DataValue;
use crate::executor::node::NodeId;
use super::types::{DataPinEvent, DataPinState, ExecPinState, PinId};

/// 所有 Pin 的基础行为
pub trait BasePin: Debug + Send + Sync + 'static {
    /// 获取 Pin 的唯一 ID
    fn id(&self) -> PinId;

    /// 获取所属节点的 ID
    fn node_id(&self) -> NodeId;

    /// 获取 Pin 的名称
    fn name(&self) -> &str;

    /// 向下转型（用于动态类型判断）
    fn as_any(&self) -> &dyn Any;

    /// 可变向下转型 (由于使用内部互斥，通常不需要 &mut self)
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// 数据 Pin：负责数据的输入输出
pub trait DataPin: BasePin {
    /// 获取当前数据值
    fn value(&self) -> DataValue;

    /// 设置数据值
    fn set_value(&self, value: DataValue) -> NodeResult<()>;

    /// 获取 Pin 状态
    fn state(&self) -> DataPinState;

    /// 设置 Pin 状态
    fn set_state(&self, state: DataPinState);

    /// 获取数据类型名称
    fn data_type(&self) -> &str;

    /// 注册事件监听器
    fn subscribe(&self, callback: Box<dyn Fn(DataPinEvent) + Send + Sync + 'static>);
}

/// 输入数据 Pin：只能接收数据
pub trait InDataPin: DataPin {
    /// 连接到上游输出 Pin
    fn link_to(&self, out_pin_id: PinId) -> NodeResult<()>;

    /// 断开上游连接
    fn unlink(&self) -> NodeResult<()>;

    /// 获取上游 Pin ID
    fn upstream(&self) -> Option<PinId>;

    /// 从上游读取数据（如果已连接）
    fn read_from_upstream(&self) -> NodeResult<DataValue>;
}

/// 输出数据 Pin：只能输出数据
pub trait OutDataPin: DataPin {
    /// 写入数据并更新状态为 Ready
    fn write(&self, data: DataValue) -> NodeResult<()>;

    /// 置为 Error 状态
    fn set_error(&self, message: String);

    /// 重置数据和状态
    fn reset(&self);

    /// 获取所有下游 Pin ID
    fn downstream(&self) -> Vec<PinId>;

    /// 添加下游连接
    fn add_downstream(&self, in_pin_id: PinId) -> NodeResult<()>;

    /// 移除下游连接
    fn remove_downstream(&self, in_pin_id: PinId) -> NodeResult<()>;
}

/// 执行 Pin：负责流程控制
pub trait ExecPin: BasePin {
    /// 触发执行
    fn trigger(&self) -> ExecutionResult<()>;

    /// 获取执行状态
    fn state(&self) -> ExecPinState;

    /// 设置执行状态
    fn set_state(&self, state: ExecPinState);

    /// 添加依赖的数据 Pin
    fn add_dependency(&self, pin_id: PinId) -> NodeResult<()>;

    /// 移除依赖
    fn remove_dependency(&self, pin_id: PinId) -> NodeResult<()>;

    /// 检查所有依赖是否就绪
    fn check_dependencies_ready(&self) -> bool;

    /// 连接到下游执行 Pin
    fn connect_to(&self, next_pin_id: PinId) -> NodeResult<()>;

    /// 获取下游执行 Pin
    fn next(&self) -> Option<PinId>;
}

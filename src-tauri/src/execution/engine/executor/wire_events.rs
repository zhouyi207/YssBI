//! 连线视觉事件的唯一发射点。
//!
//! | 函数 | 事件 | 语义 |
//! |------|------|------|
//! | `emit_data_pull` | `ConnectionActive` | data 消费者声明依赖（取数） |
//! | `emit_data_flow` | `ConnectionFlow` | data 值沿 output→input 就绪（流动） |
//! | `emit_exec_spawn` | `ConnectionActive` | exec 控制流传递到下游（前端当作流动） |

use super::super::event_emitter::EventEmitter;
use super::super::execution_event::ExecutionEvent;
use crate::graph::PinId;

fn emit_connection_active<E: EventEmitter>(emitter: &E, from: PinId, to: PinId) {
    emitter.emit(ExecutionEvent::ConnectionActive {
        from_pin_id: from.to_string(),
        to_pin_id: to.to_string(),
    });
}

pub(crate) fn emit_data_pull<E: EventEmitter>(emitter: &E, from: PinId, to: PinId) {
    emit_connection_active(emitter, from, to);
}

pub(crate) fn emit_data_flow<E: EventEmitter>(emitter: &E, from: PinId, to: PinId) {
    emitter.emit(ExecutionEvent::ConnectionFlow {
        from_pin_id: from.to_string(),
        to_pin_id: to.to_string(),
    });
}

pub(crate) fn emit_exec_spawn<E: EventEmitter>(emitter: &E, from: PinId, to: PinId) {
    emit_connection_active(emitter, from, to);
}

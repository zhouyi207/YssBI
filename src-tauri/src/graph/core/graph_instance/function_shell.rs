//! 函数签名 → 壳节点 / Call 节点 pin 投影
//!
//! - Function Entry / Function Return 是函数图内的系统托管壳节点，其 pin 是函数签名的投影：
//!   Entry 把每个 `function_inputs` 项投影为「输出」pin，Return 把每个 `function_outputs` 项
//!   投影为「输入」pin。
//! - Call Function 节点（在调用方图里）则相反：目标 inputs → Call 输入 pin，目标 outputs →
//!   Call 输出 pin。
//! - 签名同时承载 data 与 exec：`DataRole::Custom(sig.id)` / `ExecRole::Custom(sig.id)`。
//!   无 exec 入参时按数据拉取求值；有 exec 入参时走控制流子程序。

use super::*;
use crate::graph::pin::{ExecRole, PinOrder};
use crate::graph::ShellRole;

/// 单个投影 pin 的目标规格。
struct DesiredShellPin {
    role: PinRole,
    name: String,
    direction: PinDirection,
    /// `None` 表示 exec pin。
    data_type: Option<DataType>,
}

impl DesiredShellPin {
    fn control(role: ExecRole, name: &str, direction: PinDirection) -> Self {
        Self {
            role: PinRole::Exec(role),
            name: name.to_string(),
            direction,
            data_type: None,
        }
    }

    fn data(sig_id: &str, name: &str, direction: PinDirection, data_type: DataType) -> Self {
        Self {
            role: PinRole::Data(DataRole::Custom(sig_id.to_string())),
            name: name.to_string(),
            direction,
            data_type: Some(data_type),
        }
    }

    fn to_pin_definition(&self) -> PinDefinition {
        let def = match (&self.role, &self.data_type, self.direction) {
            (PinRole::Exec(role), None, PinDirection::Output) => {
                PinDefinition::exec_output(&self.name, role.clone())
            }
            (PinRole::Exec(role), None, PinDirection::Input) => {
                PinDefinition::exec_input(&self.name, role.clone())
            }
            (PinRole::Data(role), Some(dt), PinDirection::Output) => PinDefinition::data_output(
                &self.name,
                role.clone(),
                PinDataTypeDefinition::Concrete(dt.clone()),
            ),
            (PinRole::Data(role), Some(dt), PinDirection::Input) => PinDefinition::data_input(
                &self.name,
                role.clone(),
                PinDataTypeDefinition::Concrete(dt.clone()),
            ),
            (role, _, direction) => PinDefinition {
                name: self.name.clone(),
                direction,
                kind: PinKind::Data,
                role: role.clone(),
                data_type: Some(PinDataTypeDefinition::Concrete(DataType::Any)),
                optional: false,
                default_value: None,
                meta_data: Default::default(),
            },
        };
        def.with_dynamic(true)
    }
}

fn desired_pin_from_signature(
    sig: &FunctionSignaturePin,
    direction: PinDirection,
) -> Option<DesiredShellPin> {
    if sig.is_exec() {
        Some(DesiredShellPin::control(
            ExecRole::Custom(sig.id.clone()),
            &sig.name,
            direction,
        ))
    } else {
        sig.data_type
            .clone()
            .map(|dt| DesiredShellPin::data(&sig.id, &sig.name, direction, dt))
    }
}

/// 把一组签名项投影为壳 / Call pin（data + exec，保持签名顺序）。
fn pins_from_signature(
    sigs: &[FunctionSignaturePin],
    direction: PinDirection,
) -> Vec<DesiredShellPin> {
    sigs.iter()
        .filter_map(|sig| desired_pin_from_signature(sig, direction))
        .collect()
}

impl GraphInstance {
    /// 函数签名是否含 exec 入参（决定 Call 走控制流还是数据拉取）。
    pub fn signature_has_exec_input(&self) -> bool {
        self.function_inputs.iter().any(|p| p.is_exec())
    }

    /// 找到函数图的 Entry / Return 壳节点（若存在）。
    pub(crate) fn find_function_shell_nodes(&self) -> (Option<NodeId>, Option<NodeId>) {
        let data_state = self.data_state.read().unwrap();
        let mut entry = None;
        let mut ret = None;
        for (id, node) in data_state.nodes.iter() {
            match node.definition.metadata.shell_role {
                Some(ShellRole::FunctionEntry) => entry = Some(*id),
                Some(ShellRole::FunctionReturn) => ret = Some(*id),
                _ => {}
            }
        }
        (entry, ret)
    }

    /// 依据当前函数签名重建 Entry / Return 壳节点 pin，返回变更集用于发事件。
    pub fn sync_function_shell_pins(&self) -> Vec<PinChangeSet> {
        if self.kind != GraphKind::Function {
            return Vec::new();
        }

        let (entry_id, return_id) = self.find_function_shell_nodes();
        let mut sets = Vec::new();

        if let Some(node_id) = entry_id {
            let desired = pins_from_signature(&self.function_inputs, PinDirection::Output);
            sets.push(self.reconcile_shell_pins(node_id, desired, None));
        }

        if let Some(node_id) = return_id {
            let desired = pins_from_signature(&self.function_outputs, PinDirection::Input);
            sets.push(self.reconcile_shell_pins(node_id, desired, None));
        }

        sets
    }

    /// 依据目标函数签名重建本图内某个 Call Function 节点的 pin。
    pub fn sync_call_function_pins_from_signature(
        &self,
        call_node_id: NodeId,
        inputs: &[FunctionSignaturePin],
        outputs: &[FunctionSignaturePin],
        predetermined_new_pin_ids: Option<&[PinId]>,
    ) -> PinChangeSet {
        let mut desired = pins_from_signature(inputs, PinDirection::Input);
        desired.extend(pins_from_signature(outputs, PinDirection::Output));
        self.reconcile_shell_pins(call_node_id, desired, predetermined_new_pin_ids)
    }

    /// 将某壳 / Call 节点的 pin 调整为 `desired`，按 role 匹配复用已有 pin 以保留连接。
    fn reconcile_shell_pins(
        &self,
        node_id: NodeId,
        desired: Vec<DesiredShellPin>,
        predetermined_new_pin_ids: Option<&[PinId]>,
    ) -> PinChangeSet {
        let mut data_state = self.data_state.write().unwrap();

        let current_ids: Vec<PinId> = data_state
            .nodes
            .get(&node_id)
            .map(|n| n.pin_ids.clone())
            .unwrap_or_default();

        let mut added_pins = Vec::new();
        let mut updated_pins = Vec::new();
        let mut new_order = Vec::new();
        let mut used: std::collections::HashSet<PinId> = std::collections::HashSet::new();
        let mut new_pin_slot = 0usize;

        for (i, d) in desired.iter().enumerate() {
            let existing = current_ids.iter().copied().find(|pid| {
                data_state
                    .pins
                    .get(pid)
                    .map(|p| p.definition.role == d.role)
                    .unwrap_or(false)
            });

            if let Some(pid) = existing {
                used.insert(pid);
                if let Some(pin) = data_state.pins.get_mut(&pid) {
                    pin.definition = d.to_pin_definition();
                    pin.order = PinOrder(i as i32);
                    updated_pins.push(pin.clone());
                }
                new_order.push(pid);
            } else {
                let mut new_pin =
                    PinInstance::from_definition(&d.to_pin_definition(), node_id, i as i32);
                if let Some(ids) = predetermined_new_pin_ids {
                    if let Some(&predetermined_id) = ids.get(new_pin_slot) {
                        new_pin.id = predetermined_id;
                    }
                    new_pin_slot += 1;
                }
                let new_id = new_pin.id;
                data_state.connections.register_pin(new_id, node_id);
                data_state.pins.insert(new_id, new_pin.clone());
                added_pins.push(new_pin);
                new_order.push(new_id);
            }
        }

        let mut removed_pin_ids = Vec::new();
        let mut removed_connections = Vec::new();
        for pid in &current_ids {
            if used.contains(pid) {
                continue;
            }
            for to_pin in data_state.connections.get_downstream(*pid) {
                removed_connections.push((*pid, to_pin));
            }
            if let Some(from_pin) = data_state.connections.get_upstream(*pid) {
                removed_connections.push((from_pin, *pid));
            }
            data_state.connections.disconnect_all(*pid);
            data_state.pins.remove(pid);
            data_state.pin_types.remove(pid);
            removed_pin_ids.push(*pid);
        }

        if let Some(node) = data_state.nodes.get_mut(&node_id) {
            node.pin_ids = new_order;
        }

        PinChangeSet {
            node_id,
            removed_pin_ids,
            added_pins,
            updated_pins,
            removed_connections,
        }
    }
}

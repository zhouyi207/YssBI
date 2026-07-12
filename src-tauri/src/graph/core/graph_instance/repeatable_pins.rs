use super::*;

fn pin_repeatable_insert_index(
    node: &NodeInstance,
    data_state: &GraphDataState,
    definition: &NodeDefinition,
    slot_index: usize,
    template_role: &PinRole,
) -> usize {
    if let Some(pos) = node.pin_ids.iter().rposition(|pid| {
        data_state
            .pins
            .get(pid)
            .map(|p| p.definition.role.matches_family(template_role))
            .unwrap_or(false)
    }) {
        return pos + 1;
    }

    let mut idx = 0;
    for (si, slot) in definition.pin_slots.iter().enumerate() {
        if si == slot_index {
            return idx;
        }
        match slot {
            PinSlot::Fixed { .. } => {
                if idx < node.pin_ids.len() {
                    idx += 1;
                }
            }
            PinSlot::Repeatable { .. } => {
                let Some(tmpl) = slot.repeatable_template_role() else {
                    continue;
                };
                while idx < node.pin_ids.len() {
                    let pid = node.pin_ids[idx];
                    let m = data_state
                        .pins
                        .get(&pid)
                        .map(|p| p.definition.role.matches_family(tmpl))
                        .unwrap_or(false);
                    if m {
                        idx += 1;
                    } else {
                        break;
                    }
                }
            }
            PinSlot::DerivedFromInput { .. } => {}
        }
    }
    idx
}

/// Repeatable Pin 增删
impl GraphInstance {
    /// 向节点的某个 Repeatable 槽位追加一个新 pin
    ///
    /// `slot_index` 是节点定义 `pin_slots` 数组中的索引，必须指向一个 Repeatable 槽位。
    /// 返回 (新增 pin 的 PinChangeSet, 下游 resolve 产生的 PinChangeSet 列表)
    pub fn add_repeatable_pin(
        &self,
        node_id: NodeId,
        slot_index: usize,
    ) -> Result<(PinChangeSet, Vec<PinChangeSet>), String> {
        let definition;
        {
            let data_state = self.data_state.read().unwrap();
            let node = data_state
                .nodes
                .get(&node_id)
                .ok_or_else(|| format!("Node {:?} not found", node_id))?;
            definition = node.definition.clone();
        }

        let slot = definition
            .pin_slots
            .get(slot_index)
            .ok_or_else(|| format!("Slot index {} out of range", slot_index))?;

        let template_role = match slot {
            PinSlot::Repeatable { template, .. } => &template.role,
            _ => {
                return Err(format!(
                    "Slot index {} is not a Repeatable slot",
                    slot_index
                ));
            }
        };

        let family_pins = self.get_pin_instances_by_pin_role_family(node_id, template_role);
        let current_count = family_pins.len();

        if let PinSlot::Repeatable { max_count, .. } = slot {
            if let Some(max) = max_count {
                if current_count >= *max {
                    return Err(format!("Repeatable slot already at max count ({})", max));
                }
            }
        }

        let new_index = current_count;
        let pin_def = slot
            .generate_pin_at_index(new_index)
            .ok_or_else(|| "Failed to generate pin definition".to_string())?;

        let order = {
            let data_state = self.data_state.read().unwrap();
            let node = data_state.nodes.get(&node_id).unwrap();
            node.pin_ids.len() as i32
        };

        let new_pin = PinInstance::from_definition(&pin_def, node_id, order);
        let new_pin_id = new_pin.id;

        {
            let mut data_state = self.data_state.write().unwrap();
            data_state.connections.register_pin(new_pin_id, node_id);
            data_state.pins.insert(new_pin_id, new_pin.clone());

            let insert_pos = data_state
                .nodes
                .get(&node_id)
                .map(|node| {
                    pin_repeatable_insert_index(
                        node,
                        &data_state,
                        definition.as_ref(),
                        slot_index,
                        template_role,
                    )
                })
                .unwrap_or(0);

            if let Some(node) = data_state.nodes.get_mut(&node_id) {
                let pos = insert_pos.min(node.pin_ids.len());
                node.pin_ids.insert(pos, new_pin_id);
            }
        }

        let resolve_sets = self
            .recompile(GraphRecompileScope::FromSeeds(vec![node_id]))
            .change_sets;

        let main_set = PinChangeSet {
            node_id,
            removed_pin_ids: vec![],
            added_pins: vec![new_pin],
            updated_pins: vec![],
            removed_connections: vec![],
        };
        Ok((main_set, resolve_sets))
    }

    /// 从节点移除一个 Repeatable 槽位的 pin
    ///
    /// 验证 pin 属于某个 Repeatable 槽位且当前数量 > min_count，
    /// 然后断开连接、移除 pin，并重新索引剩余的同族 pin。
    /// 返回 (移除信息的 PinChangeSet, 被移除 pin 在槽位中的索引, 下游 resolve 产生的 PinChangeSet 列表)
    pub fn remove_repeatable_pin(
        &self,
        node_id: NodeId,
        pin_id: PinId,
    ) -> Result<(PinChangeSet, usize, Vec<PinChangeSet>), String> {
        let definition;
        let pin_role;
        {
            let data_state = self.data_state.read().unwrap();
            let node = data_state
                .nodes
                .get(&node_id)
                .ok_or_else(|| format!("Node {:?} not found", node_id))?;
            definition = node.definition.clone();

            let pin = data_state
                .pins
                .get(&pin_id)
                .ok_or_else(|| format!("Pin {:?} not found", pin_id))?;
            if pin.node_id != node_id {
                return Err("Pin does not belong to the specified node".to_string());
            }
            pin_role = pin.definition.role.clone();
        }

        let (slot_index, slot) = definition
            .pin_slots
            .iter()
            .enumerate()
            .find(|(_, s)| {
                s.repeatable_template_role()
                    .map(|tmpl_role| pin_role.matches_family(tmpl_role))
                    .unwrap_or(false)
            })
            .ok_or_else(|| "Pin does not belong to any Repeatable slot".to_string())?;

        let template_role = slot.repeatable_template_role().unwrap();
        let min_count = slot.repeatable_min_count().unwrap_or(0);

        let family_pins = self.get_pin_instances_by_pin_role_family(node_id, template_role);
        let current_count = family_pins.len();

        if current_count <= min_count {
            return Err(format!(
                "Cannot remove pin: already at minimum count ({})",
                min_count
            ));
        }

        let pin_index_in_family = family_pins
            .iter()
            .position(|p| p.id == pin_id)
            .ok_or_else(|| "Pin not found in family".to_string())?;

        // Collect connections that will be removed
        let mut removed_connections = Vec::new();
        {
            let data_state = self.data_state.read().unwrap();
            let downstream = data_state.connections.get_downstream(pin_id);
            for to_pin in &downstream {
                removed_connections.push((pin_id, *to_pin));
            }
            if let Some(from_pin) = data_state.connections.get_upstream(pin_id) {
                removed_connections.push((from_pin, pin_id));
            }
        }

        // Remove the pin
        {
            let mut data_state = self.data_state.write().unwrap();
            data_state.connections.disconnect_all(pin_id);
            data_state.pins.remove(&pin_id);
            data_state.pin_types.remove(&pin_id);
            if let Some(node) = data_state.nodes.get_mut(&node_id) {
                node.pin_ids.retain(|id| *id != pin_id);
            }
        }

        // Re-index remaining pins in the same family
        let updated_pins = self.reindex_repeatable_pins(node_id, slot_index)?;

        let resolve_sets = self
            .recompile(GraphRecompileScope::FromSeeds(vec![node_id]))
            .change_sets;

        let main_set = PinChangeSet {
            node_id,
            removed_pin_ids: vec![pin_id],
            added_pins: vec![],
            updated_pins,
            removed_connections,
        };
        Ok((main_set, pin_index_in_family, resolve_sets))
    }

    /// Re-index all pins belonging to a Repeatable slot so their roles and names
    /// are contiguous (Operands(0), Operands(1), ...; A, B, C, ...).
    /// Returns updated pin instances for frontend sync.
    fn reindex_repeatable_pins(
        &self,
        node_id: NodeId,
        slot_index: usize,
    ) -> Result<Vec<PinInstance>, String> {
        let definition;
        {
            let data_state = self.data_state.read().unwrap();
            let node = data_state
                .nodes
                .get(&node_id)
                .ok_or_else(|| format!("Node {:?} not found", node_id))?;
            definition = node.definition.clone();
        }

        let slot = definition
            .pin_slots
            .get(slot_index)
            .ok_or_else(|| format!("Slot index {} out of range", slot_index))?;

        let template_role = match slot.repeatable_template_role() {
            Some(r) => r.clone(),
            None => return Ok(Vec::new()),
        };

        let family_pins = self.get_pin_instances_by_pin_role_family(node_id, &template_role);

        let mut data_state = self.data_state.write().unwrap();
        for (i, fpin) in family_pins.iter().enumerate() {
            if let Some(pin) = data_state.pins.get_mut(&fpin.id) {
                if let Some(pin_def) = slot.generate_pin_at_index(i) {
                    pin.definition.role = pin_def.role;
                    pin.definition.name = pin_def.name;
                }
            }
        }

        Ok(family_pins
            .iter()
            .filter_map(|fpin| data_state.pins.get(&fpin.id).cloned())
            .collect())
    }
}

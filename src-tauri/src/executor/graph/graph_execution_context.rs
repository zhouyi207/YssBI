use super::Graph;
use crate::executor::node::{NodeExecutionContext, NodeId};
use crate::executor::pin::{PinDirection, PinId, PinRole};
use crate::executor::value::DataValue;

/// 具体的执行上下文实现
pub struct GraphExecutionContext<'a> {
    pub graph: &'a mut Graph,
    pub node_id: NodeId,
    pub logs: Vec<String>,
}

impl<'a> GraphExecutionContext<'a> {
    pub fn new(graph: &'a mut Graph, node_id: NodeId) -> Self {
        Self {
            graph,
            node_id,
            logs: Vec::new(),
        }
    }
}

impl<'a> NodeExecutionContext for GraphExecutionContext<'a> {
    fn get_input_by_role(&self, role: &PinRole) -> Result<DataValue, String> {
        let pin = self
            .graph
            .get_pin_by_role(self.node_id, role)
            .ok_or_else(|| format!("Input pin with role {:?} not found", role))?;

        if pin.definition.direction != PinDirection::Input {
            return Err(format!("Pin {:?} is not an input", role));
        }

        self.graph
            .resolve_pin_value(pin.id)
            .ok_or_else(|| format!("Pin {:?} has no resolved value", role))
    }

    fn get_inputs_by_role(&self, role: &PinRole) -> Result<Vec<DataValue>, String> {
        let pins = self.graph.get_pins_by_role(self.node_id, role);

        if pins.is_empty() {
            return Err(format!("No input pins with role {:?} found", role));
        }

        let mut values = Vec::new();

        for pin in pins {
            if pin.definition.direction != PinDirection::Input {
                continue;
            }

            if let Some(value) = self.graph.resolve_pin_value(pin.id) {
                values.push(value);
            }
        }

        Ok(values)
    }

    fn emit_output_by_role(&mut self, role: &PinRole, value: DataValue) -> Result<(), String> {
        let pin = self
            .graph
            .get_pin_by_role(self.node_id, role)
            .ok_or_else(|| format!("Output pin with role {:?} not found", role))?;

        if pin.definition.direction != PinDirection::Output {
            return Err(format!("Pin {:?} is not an output", role));
        }

        self.graph.set_pin_current_value(pin.id, value)
    }

    fn emit_outputs_by_role(
        &mut self,
        role: &PinRole,
        values: Vec<DataValue>,
    ) -> Result<(), String> {
        let pins = self.graph.get_pins_by_role(self.node_id, role);

        let output_pins: Vec<PinId> = pins
            .iter()
            .filter(|p| p.definition.direction == PinDirection::Output)
            .map(|p| p.id)
            .collect();

        if output_pins.len() != values.len() {
            return Err(format!(
                "Value count mismatch: {} pins, {} values",
                output_pins.len(),
                values.len()
            ));
        }

        for (pin_id, value) in output_pins.into_iter().zip(values) {
            self.graph.set_pin_current_value(pin_id, value)?;
        }

        Ok(())
    }

    fn is_input_connected(&self, role: &PinRole) -> bool {
        self.graph
            .get_pin_by_role(self.node_id, role)
            .and_then(|pin| self.graph.connections().get_upstream(pin.id))
            .is_some()
    }

    fn node_id(&self) -> NodeId {
        self.node_id
    }

    fn log(&mut self, message: String) {
        self.logs.push(message);
    }

    fn error(&mut self, message: String) {
        self.logs.push(format!("ERROR: {}", message));
    }
}

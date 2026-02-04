//! 新架构节点示例
//!
//! 展示如何使用新架构创建节点

use std::sync::Arc;
use crate::executor::value::{PinTypeDesc, ValueType, TypeConstraint};
use crate::executor::types::DataValue;
use super::new_architecture::*;

// ==================== 示例 1: Add 节点（支持动态 Operands） ====================

/// 创建 Add 节点定义
/// 
/// 支持任意数量的操作数，通过 Operands 角色访问
pub fn create_add_node_definition() -> NodeDefinition {
    NodeDefinition::new("add", "Add (+)")
        // 动态输入组：Operands
        .add_pin(PinDefinition::dynamic_group(
            PinRole::Operands,
            PinDirection::Input,
            PinKind::Data,
            Some(PinTypeDesc::concrete(ValueType::Float64)),
            "operands",
        ))
        // 输出：Result
        .add_pin(PinDefinition::data_output(
            PinRole::Result,
            "Result",
            PinTypeDesc::concrete(ValueType::Float64),
        ))
        .with_processor(Arc::new(|ctx| {
            // 通过角色获取所有操作数
            let operands = ctx.get_inputs_by_role(&PinRole::Operands)?;
            
            // 计算总和
            let mut sum = 0.0;
            for operand in operands {
                match operand {
                    DataValue::Float64(v) => sum += v,
                    DataValue::Int64(v) => sum += v as f64,
                    _ => {
                        ctx.error(format!("Invalid operand type: {:?}", operand));
                        return Err("Invalid operand type".to_string());
                    }
                }
            }
            
            // 输出结果
            ctx.emit_output_by_role(&PinRole::Result, DataValue::Float64(sum))?;
            
            ctx.log(format!("Add: {} operands, result = {}", operands.len(), sum));
            
            // 返回下一个执行 Pin 的角色（如果有）
            Ok(PinRole::ExecOut)
        }))
        .with_metadata(
            vec!["Math".into(), "Operators".into()],
            "math".into(),
            Some("Add multiple numbers together".into()),
        )
        .with_dynamic_pins(DynamicPinConfig {
            min_count: 2,
            max_count: Some(10),
            name_template: "Operand {}".to_string(),
            can_reorder: true,
        })
}

// ==================== 示例 2: If-Else 节点 ====================

/// 创建 If-Else 节点定义
/// 
/// 使用语义角色：Condition, ExecTrue, ExecFalse
pub fn create_if_else_node_definition() -> NodeDefinition {
    NodeDefinition::new("if_else", "If-Else")
        // 执行输入
        .add_pin(PinDefinition::exec_input(PinRole::ExecIn, "In"))
        // 条件输入
        .add_pin(PinDefinition::data_input(
            PinRole::Condition,
            "Condition",
            PinTypeDesc::concrete(ValueType::Boolean),
        ))
        // True 分支
        .add_pin(PinDefinition::exec_output(PinRole::ExecTrue, "True"))
        // False 分支
        .add_pin(PinDefinition::exec_output(PinRole::ExecFalse, "False"))
        .with_processor(Arc::new(|ctx| {
            // 通过角色获取条件值
            let condition = ctx.get_input_by_role(&PinRole::Condition)?;
            
            // 根据条件选择执行路径
            let next_role = match condition {
                DataValue::Boolean(true) => {
                    ctx.log("Condition is true, taking True branch".to_string());
                    PinRole::ExecTrue
                }
                DataValue::Boolean(false) => {
                    ctx.log("Condition is false, taking False branch".to_string());
                    PinRole::ExecFalse
                }
                _ => {
                    ctx.error(format!("Invalid condition type: {:?}", condition));
                    return Err("Condition must be boolean".to_string());
                }
            };
            
            // 返回要触发的执行 Pin 角色
            Ok(next_role)
        }))
        .with_metadata(
            vec!["Control Flow".into()],
            "control".into(),
            Some("Branch execution based on condition".into()),
        )
}

// ==================== 示例 3: Sequence 节点（支持动态 Steps） ====================

/// 创建 Sequence 节点定义
/// 
/// 支持任意数量的执行步骤，通过 Steps 角色访问
pub fn create_sequence_node_definition() -> NodeDefinition {
    NodeDefinition::new("sequence", "Sequence")
        // 执行输入
        .add_pin(PinDefinition::exec_input(PinRole::ExecIn, "In"))
        // 动态输出组：Steps
        .add_pin(PinDefinition::dynamic_group(
            PinRole::Steps,
            PinDirection::Output,
            PinKind::Exec,
            None,
            "steps",
        ))
        .with_processor(Arc::new(|ctx| {
            ctx.log("Executing sequence steps".to_string());
            
            // Sequence 节点会依次触发所有 Steps
            // 这里返回第一个 Step 的角色
            // 实际执行器会处理多个执行输出的情况
            Ok(PinRole::Steps)
        }))
        .with_metadata(
            vec!["Control Flow".into()],
            "control".into(),
            Some("Execute multiple steps in sequence".into()),
        )
        .with_dynamic_pins(DynamicPinConfig {
            min_count: 2,
            max_count: Some(10),
            name_template: "Then {}".to_string(),
            can_reorder: true,
        })
}

// ==================== 示例 4: 泛型 Add 节点（类型推断） ====================

/// 创建泛型 Add 节点定义
/// 
/// 使用类型变量和约束，支持多种数值类型
pub fn create_generic_add_node_definition() -> NodeDefinition {
    use crate::executor::value::{TypeVarId, DataType};
    
    // 创建类型变量（所有操作数和结果共享同一类型）
    let type_var = TypeVarId::new();
    
    NodeDefinition::new("add_generic", "Add (Generic)")
        // 动态输入组：Operands（带类型约束）
        .add_pin(PinDefinition {
            role: PinRole::Operands,
            direction: PinDirection::Input,
            kind: PinKind::Data,
            type_desc: Some(PinTypeDesc {
                data_type: DataType::TypeVar(type_var),
                constraints: vec![TypeConstraint::Numeric],
                is_optional: false,
                is_array: false,
            }),
            display_name: "Operand".to_string(),
            is_dynamic: true,
            group_id: Some("operands".to_string()),
        })
        // 输出：Result（使用相同的类型变量）
        .add_pin(PinDefinition {
            role: PinRole::Result,
            direction: PinDirection::Output,
            kind: PinKind::Data,
            type_desc: Some(PinTypeDesc {
                data_type: DataType::TypeVar(type_var),
                constraints: vec![TypeConstraint::Numeric],
                is_optional: false,
                is_array: false,
            }),
            display_name: "Result".to_string(),
            is_dynamic: false,
            group_id: None,
        })
        .with_processor(Arc::new(|ctx| {
            let operands = ctx.get_inputs_by_role(&PinRole::Operands)?;
            
            if operands.is_empty() {
                return Err("No operands provided".to_string());
            }
            
            // 根据第一个操作数的类型决定计算方式
            let result = match &operands[0] {
                DataValue::Int64(_) => {
                    let mut sum: i64 = 0;
                    for operand in operands {
                        if let DataValue::Int64(v) = operand {
                            sum += v;
                        } else {
                            return Err("Type mismatch in operands".to_string());
                        }
                    }
                    DataValue::Int64(sum)
                }
                DataValue::Float64(_) => {
                    let mut sum: f64 = 0.0;
                    for operand in operands {
                        if let DataValue::Float64(v) = operand {
                            sum += v;
                        } else {
                            return Err("Type mismatch in operands".to_string());
                        }
                    }
                    DataValue::Float64(sum)
                }
                _ => {
                    return Err("Unsupported operand type".to_string());
                }
            };
            
            ctx.emit_output_by_role(&PinRole::Result, result)?;
            Ok(PinRole::ExecOut)
        }))
        .with_metadata(
            vec!["Math".into(), "Operators".into()],
            "math".into(),
            Some("Add multiple numbers (type-safe)".into()),
        )
        .with_dynamic_pins(DynamicPinConfig {
            min_count: 2,
            max_count: Some(10),
            name_template: "Operand {}".to_string(),
            can_reorder: true,
        })
}

// ==================== 示例 5: Switch 节点（多分支） ====================

/// 创建 Switch 节点定义
/// 
/// 支持多个 Case 分支，通过 Cases 角色访问
pub fn create_switch_node_definition() -> NodeDefinition {
    NodeDefinition::new("switch", "Switch")
        // 执行输入
        .add_pin(PinDefinition::exec_input(PinRole::ExecIn, "In"))
        // 选择值输入
        .add_pin(PinDefinition::data_input(
            PinRole::Input,
            "Value",
            PinTypeDesc::concrete(ValueType::Int64),
        ))
        // 动态输出组：Cases
        .add_pin(PinDefinition::dynamic_group(
            PinRole::Cases,
            PinDirection::Output,
            PinKind::Exec,
            None,
            "cases",
        ))
        // 默认分支
        .add_pin(PinDefinition::exec_output(
            PinRole::Custom("Default".to_string()),
            "Default",
        ))
        .with_processor(Arc::new(|ctx| {
            let value = ctx.get_input_by_role(&PinRole::Input)?;
            
            let index = match value {
                DataValue::Int64(v) => v as usize,
                _ => {
                    ctx.error("Switch value must be integer".to_string());
                    return Ok(PinRole::Custom("Default".to_string()));
                }
            };
            
            ctx.log(format!("Switch: selecting case {}", index));
            
            // 这里简化处理，实际应该根据 index 选择对应的 Case
            // 执行器会处理 Cases 角色的多个 Pin
            Ok(PinRole::Cases)
        }))
        .with_metadata(
            vec!["Control Flow".into()],
            "control".into(),
            Some("Multi-way branch based on integer value".into()),
        )
        .with_dynamic_pins(DynamicPinConfig {
            min_count: 2,
            max_count: Some(10),
            name_template: "Case {}".to_string(),
            can_reorder: false,
        })
}

// ==================== 示例 6: 简单数学运算节点 ====================

/// 创建 Multiply 节点定义
pub fn create_multiply_node_definition() -> NodeDefinition {
    NodeDefinition::new("multiply", "Multiply (*)")
        .add_pin(PinDefinition::data_input(
            PinRole::Custom("A".to_string()),
            "A",
            PinTypeDesc::concrete(ValueType::Float64),
        ))
        .add_pin(PinDefinition::data_input(
            PinRole::Custom("B".to_string()),
            "B",
            PinTypeDesc::concrete(ValueType::Float64),
        ))
        .add_pin(PinDefinition::data_output(
            PinRole::Result,
            "Result",
            PinTypeDesc::concrete(ValueType::Float64),
        ))
        .with_processor(Arc::new(|ctx| {
            let a = ctx.get_input_by_role(&PinRole::Custom("A".to_string()))?;
            let b = ctx.get_input_by_role(&PinRole::Custom("B".to_string()))?;
            
            let result = match (a, b) {
                (DataValue::Float64(va), DataValue::Float64(vb)) => va * vb,
                (DataValue::Int64(va), DataValue::Int64(vb)) => (va * vb) as f64,
                _ => {
                    ctx.error("Invalid operand types".to_string());
                    return Err("Operands must be numeric".to_string());
                }
            };
            
            ctx.emit_output_by_role(&PinRole::Result, DataValue::Float64(result))?;
            ctx.log(format!("Multiply: result = {}", result));
            
            Ok(PinRole::ExecOut)
        }))
        .with_metadata(
            vec!["Math".into(), "Operators".into()],
            "math".into(),
            Some("Multiply two numbers".into()),
        )
}

// ==================== 节点注册表 ====================

/// 节点定义注册表
pub struct NodeDefinitionRegistry {
    definitions: std::collections::HashMap<NodeDefinitionId, Arc<NodeDefinition>>,
}

impl NodeDefinitionRegistry {
    pub fn new() -> Self {
        Self {
            definitions: std::collections::HashMap::new(),
        }
    }
    
    /// 注册节点定义
    pub fn register(&mut self, definition: NodeDefinition) {
        let id = definition.node_type.clone();
        self.definitions.insert(id, Arc::new(definition));
    }
    
    /// 获取节点定义
    pub fn get(&self, node_type: &str) -> Option<Arc<NodeDefinition>> {
        self.definitions.get(node_type).cloned()
    }
    
    /// 注册所有内置节点
    pub fn register_builtin_nodes(&mut self) {
        self.register(create_add_node_definition());
        self.register(create_if_else_node_definition());
        self.register(create_sequence_node_definition());
        self.register(create_generic_add_node_definition());
        self.register(create_switch_node_definition());
        self.register(create_multiply_node_definition());
    }
}

impl Default for NodeDefinitionRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register_builtin_nodes();
        registry
    }
}

// ==================== 使用示例 ====================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_add_node_with_role_based_access() {
        // 创建 Graph
        let mut graph = Graph::new();
        
        // 获取节点定义
        let registry = NodeDefinitionRegistry::default();
        let add_def = registry.get("add").unwrap();
        
        // 创建节点实例
        let node = NodeInstance::new("add", "My Add Node");
        let node_id = graph.add_node(node, &add_def);
        
        // 创建动态 Pin（模拟添加操作数）
        // 实际应该通过 API 添加
        
        // 设置输入值（通过角色）
        let operand_pins = graph.get_pins_by_role(node_id, &PinRole::Operands);
        for (i, pin) in operand_pins.iter().enumerate() {
            graph.set_pin_value(pin.id, DataValue::Float64((i + 1) as f64 * 10.0)).unwrap();
        }
        
        // 创建执行上下文
        let mut ctx = GraphExecutionContext::new(&mut graph, node_id);
        
        // 执行节点处理器
        if let Some(processor) = &add_def.processor {
            let result = processor(&mut ctx);
            assert!(result.is_ok());
        }
        
        // 验证结果（通过角色）
        let result_pin = graph.get_pin_by_role(node_id, &PinRole::Result).unwrap();
        let result_value = graph.get_pin_value(result_pin.id).unwrap();
        
        // 结果应该是所有操作数的和
        println!("Result: {:?}", result_value);
        println!("Logs: {:?}", ctx.logs);
    }
    
    #[test]
    fn test_if_else_node_with_role_based_access() {
        let mut graph = Graph::new();
        let registry = NodeDefinitionRegistry::default();
        let if_else_def = registry.get("if_else").unwrap();
        
        let node = NodeInstance::new("if_else", "My If-Else");
        let node_id = graph.add_node(node, &if_else_def);
        
        // 设置条件值（通过角色）
        let condition_pin = graph.get_pin_by_role(node_id, &PinRole::Condition).unwrap();
        graph.set_pin_value(condition_pin.id, DataValue::Boolean(true)).unwrap();
        
        // 执行
        let mut ctx = GraphExecutionContext::new(&mut graph, node_id);
        if let Some(processor) = &if_else_def.processor {
            let next_role = processor(&mut ctx).unwrap();
            assert_eq!(next_role, PinRole::ExecTrue);
        }
        
        println!("Logs: {:?}", ctx.logs);
    }
}

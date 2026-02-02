//! 多输出字符串节点
//! 
//! 包含返回多个结果的字符串处理节点

use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericOutDataPin, GenericInDataPin};
use serde_json::json;
use crate::executor::value::{ValueType, PinTypeDesc};

pub fn register(registry: &NodeRegistry) {
    let string_cat = vec!["String".into(), "Multi-Output".into()];

    // ============================================================================
    // SplitString 节点 - 分割字符串为多个部分
    // ============================================================================
    {
        let node = GenericNode::new_prototype("split_string", "Split String");
        node.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Input", PinTypeDesc::concrete(ValueType::String)));
        node.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Delimiter", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "First", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Second", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Rest", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Array", PinTypeDesc::concrete(ValueType::List(Box::new(ValueType::String)))));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let input = ctx.get_pin_value(&node.inputs[0].id)
                .as_str()
                .unwrap_or("")
                .to_string();
            let delimiter = ctx.get_pin_value(&node.inputs[1].id)
                .as_str()
                .unwrap_or(",")
                .to_string();
            
            let parts: Vec<&str> = input.split(&delimiter).collect();
            
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            match output_name {
                "First" => json!(parts.get(0).unwrap_or(&"")),
                "Second" => json!(parts.get(1).unwrap_or(&"")),
                "Rest" => {
                    if parts.len() > 2 {
                        json!(parts[2..].join(&delimiter))
                    } else {
                        json!("")
                    }
                }
                "Array" => json!(parts),
                _ => json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            string_cat.clone(),
            "string".into(),
            Some("Splits a string into multiple parts".into())
        );
        registry.register("split_string".into(), Arc::new(node));
    }

    // ============================================================================
    // ParseURL 节点 - 解析 URL 为多个组件
    // ============================================================================
    {
        let node = GenericNode::new_prototype("parse_url", "Parse URL");
        node.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "URL", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Protocol", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Host", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Port", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Path", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Query", PinTypeDesc::concrete(ValueType::String)));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let url_str = ctx.get_pin_value(&node.inputs[0].id)
                .as_str()
                .unwrap_or("")
                .to_string();
            
            // 简单的 URL 解析（实际应该使用 url crate）
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            // 简化的解析逻辑
            let protocol = if let Some(pos) = url_str.find("://") {
                &url_str[..pos]
            } else {
                ""
            };
            
            let after_protocol = if let Some(pos) = url_str.find("://") {
                &url_str[pos + 3..]
            } else {
                &url_str
            };
            
            let (host_port, path_query) = if let Some(pos) = after_protocol.find('/') {
                (&after_protocol[..pos], &after_protocol[pos..])
            } else {
                (after_protocol, "")
            };
            
            let (host, port) = if let Some(pos) = host_port.find(':') {
                (&host_port[..pos], &host_port[pos + 1..])
            } else {
                (host_port, "")
            };
            
            let (path, query) = if let Some(pos) = path_query.find('?') {
                (&path_query[..pos], &path_query[pos + 1..])
            } else {
                (path_query, "")
            };
            
            match output_name {
                "Protocol" => json!(protocol),
                "Host" => json!(host),
                "Port" => json!(port),
                "Path" => json!(path),
                "Query" => json!(query),
                _ => json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            string_cat.clone(),
            "string".into(),
            Some("Parses a URL into its components".into())
        );
        registry.register("parse_url".into(), Arc::new(node));
    }

    // ============================================================================
    // StringInfo 节点 - 返回字符串的多个属性
    // ============================================================================
    {
        let node = GenericNode::new_prototype("string_info", "String Info");
        node.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Input", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Length", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "IsEmpty", PinTypeDesc::concrete(ValueType::Boolean)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "WordCount", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "LineCount", PinTypeDesc::concrete(ValueType::Float64)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "FirstChar", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "LastChar", PinTypeDesc::concrete(ValueType::String)));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let input = ctx.get_pin_value(&node.inputs[0].id)
                .as_str()
                .unwrap_or("")
                .to_string();
            
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            match output_name {
                "Length" => json!(input.len()),
                "IsEmpty" => json!(input.is_empty()),
                "WordCount" => json!(input.split_whitespace().count()),
                "LineCount" => json!(input.lines().count()),
                "FirstChar" => json!(input.chars().next().map(|c| c.to_string()).unwrap_or_default()),
                "LastChar" => json!(input.chars().last().map(|c| c.to_string()).unwrap_or_default()),
                _ => json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            string_cat.clone(),
            "string".into(),
            Some("Returns multiple properties of a string".into())
        );
        registry.register("string_info".into(), Arc::new(node));
    }

    // ============================================================================
    // ParseName 节点 - 解析姓名为多个部分
    // ============================================================================
    {
        let node = GenericNode::new_prototype("parse_name", "Parse Name");
        node.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "FullName", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "FirstName", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "MiddleName", PinTypeDesc::concrete(ValueType::String)));
        node.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "LastName", PinTypeDesc::concrete(ValueType::String)));
        
        node.set_data_processor(Box::new(|ctx, node, pin_id| {
            let full_name = ctx.get_pin_value(&node.inputs[0].id)
                .as_str()
                .unwrap_or("")
                .to_string();
            
            let parts: Vec<&str> = full_name.split_whitespace().collect();
            
            let output_name = node.outputs.iter()
                .find(|p| p.id == *pin_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            
            match output_name {
                "FirstName" => json!(parts.get(0).unwrap_or(&"")),
                "MiddleName" => {
                    if parts.len() > 2 {
                        json!(parts[1..parts.len()-1].join(" "))
                    } else {
                        json!("")
                    }
                }
                "LastName" => {
                    if parts.len() > 1 {
                        json!(parts.last().unwrap_or(&""))
                    } else {
                        json!("")
                    }
                }
                _ => json!(null)
            }
        }));
        
        let mut node = node;
        node.set_metadata(
            string_cat.clone(),
            "string".into(),
            Some("Parses a full name into first, middle, and last names".into())
        );
        registry.register("parse_name".into(), Arc::new(node));
    }
}

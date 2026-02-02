use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericInDataPin, GenericOutDataPin, GenericOutExecPin, GenericInExecPin};
use crate::executor::value::{PinTypeDesc, ValueType};
use serde_json::Value;

/// 计算样本均值
fn mean(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    data.iter().sum::<f64>() / data.len() as f64
}

/// 计算样本标准差
fn std_dev(data: &[f64], mean: f64) -> f64 {
    if data.len() <= 1 {
        return 0.0;
    }
    let variance = data.iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>() / (data.len() - 1) as f64;
    variance.sqrt()
}

/// 计算 t 统计量（独立样本 t 检验）
fn calculate_t_statistic(sample1: &[f64], sample2: &[f64]) -> (f64, f64) {
    let n1 = sample1.len() as f64;
    let n2 = sample2.len() as f64;
    
    if n1 < 2.0 || n2 < 2.0 {
        return (0.0, 1.0); // 样本太小，返回无意义的值
    }
    
    let mean1 = mean(sample1);
    let mean2 = mean(sample2);
    
    let std1 = std_dev(sample1, mean1);
    let std2 = std_dev(sample2, mean2);
    
    // 合并标准差（假设方差齐性）
    let pooled_std = ((std1.powi(2) * (n1 - 1.0) + std2.powi(2) * (n2 - 1.0)) / (n1 + n2 - 2.0)).sqrt();
    
    // t 统计量
    let t = (mean1 - mean2) / (pooled_std * ((1.0 / n1) + (1.0 / n2)).sqrt());
    
    // 自由度
    let df = n1 + n2 - 2.0;
    
    // 计算 p 值（双尾检验）
    let p_value = calculate_p_value(t.abs(), df);
    
    (t, p_value)
}

/// 计算 p 值（使用 t 分布的近似）
fn calculate_p_value(t: f64, df: f64) -> f64 {
    // 对于大样本（df > 30），t 分布接近正态分布
    if df > 30.0 {
        let z = t;
        let p = 2.0 * (1.0 - normal_cdf(z.abs()));
        return p.max(0.0).min(1.0);
    }
    
    // 对于小样本，使用 t 分布近似
    let x = df / (df + t * t);
    let p = incomplete_beta(df / 2.0, 0.5, x);
    p.max(0.0).min(1.0)
}

/// 标准正态分布的累积分布函数（CDF）
fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

/// 误差函数的近似实现
fn erf(x: f64) -> f64 {
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;
    
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    
    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    
    sign * y
}

/// 不完全 Beta 函数的近似实现
fn incomplete_beta(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    
    let bt = (a * x.ln() + b * (1.0 - x).ln()).exp() / beta_function(a, b);
    
    if x < (a + 1.0) / (a + b + 2.0) {
        bt * continued_fraction(a, b, x) / a
    } else {
        1.0 - bt * continued_fraction(b, a, 1.0 - x) / b
    }
}

/// Beta 函数
fn beta_function(a: f64, b: f64) -> f64 {
    (gamma(a) * gamma(b)) / gamma(a + b)
}

/// Gamma 函数的近似（Stirling 近似）
fn gamma(x: f64) -> f64 {
    if x < 0.5 {
        std::f64::consts::PI / ((std::f64::consts::PI * x).sin() * gamma(1.0 - x))
    } else {
        let x = x - 1.0;
        let tmp = x + 5.5;
        let tmp = (x + 0.5) * tmp.ln() - tmp;
        let ser = 1.000000000190015
            + 76.18009172947146 / (x + 1.0)
            - 86.50532032941677 / (x + 2.0)
            + 24.01409824083091 / (x + 3.0)
            - 1.231739572450155 / (x + 4.0)
            + 0.1208650973866179e-2 / (x + 5.0)
            - 0.5395239384953e-5 / (x + 6.0);
        (tmp + ser.ln()).exp()
    }
}

/// 连分数展开
fn continued_fraction(a: f64, b: f64, x: f64) -> f64 {
    const MAX_ITER: usize = 100;
    const EPSILON: f64 = 1e-10;
    
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    
    if d.abs() < EPSILON {
        d = EPSILON;
    }
    d = 1.0 / d;
    let mut h = d;
    
    for m in 1..=MAX_ITER {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < EPSILON {
            d = EPSILON;
        }
        c = 1.0 + aa / c;
        if c.abs() < EPSILON {
            c = EPSILON;
        }
        d = 1.0 / d;
        h *= d * c;
        
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < EPSILON {
            d = EPSILON;
        }
        c = 1.0 + aa / c;
        if c.abs() < EPSILON {
            c = EPSILON;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        
        if (del - 1.0).abs() < EPSILON {
            break;
        }
    }
    
    h
}

/// 从 JSON Value 中提取浮点数数组（支持 Series 格式）
fn extract_float_array(value: &Value) -> Result<Vec<f64>, String> {
    match value {
        Value::Array(arr) => {
            let mut result = Vec::new();
            for item in arr {
                match item {
                    Value::Number(n) => {
                        if let Some(f) = n.as_f64() {
                            result.push(f);
                        } else {
                            return Err("Array contains non-numeric values".to_string());
                        }
                    }
                    Value::Null => {
                        // 跳过 null 值
                        continue;
                    }
                    _ => return Err("Array contains non-numeric values".to_string()),
                }
            }
            if result.is_empty() {
                return Err("Array contains no valid numeric values".to_string());
            }
            Ok(result)
        }
        _ => Err("Input is not an array".to_string()),
    }
}

pub fn register(registry: &NodeRegistry) {
    let t_test_node = GenericNode::new_prototype("t_test", "T-Test");
    
    // 输入 pins
    t_test_node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    t_test_node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Sample1",
        PinTypeDesc::concrete(ValueType::Series)
    ));
    t_test_node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Sample2",
        PinTypeDesc::concrete(ValueType::Series)
    ));
    
    // 输出 pins
    t_test_node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));
    t_test_node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "T",
        PinTypeDesc::concrete(ValueType::Float64)
    ));
    t_test_node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "P",
        PinTypeDesc::concrete(ValueType::Float64)
    ));
    
    t_test_node.set_flow_processor(Box::new(|ctx, node| {
        ctx.log(format!("[T-Test] Node has {} inputs, {} outputs", node.inputs.len(), node.outputs.len()));
        
        // 打印所有输入 pin 的信息
        for (i, input) in node.inputs.iter().enumerate() {
            ctx.log(format!("[T-Test] Input[{}]: id={}, name={}, type={}", i, input.id, input.name, input.pin_type));
        }
        
        // 查找数据输入 pins（跳过 exec pin）
        let data_inputs: Vec<_> = node.inputs.iter()
            .filter(|pin| pin.pin_type != "exec")
            .collect();
        
        if data_inputs.len() < 2 {
            ctx.log(format!("[T-Test] Error: Expected 2 data inputs (Sample1, Sample2), got {}", data_inputs.len()));
            return Err("Missing input samples".to_string());
        }
        
        ctx.log(format!("[T-Test] Getting Sample1 from pin: {}", data_inputs[0].id));
        let sample1_value = ctx.get_pin_value(&data_inputs[0].id);
        ctx.log(format!("[T-Test] Sample1 value: {}", serde_json::to_string(&sample1_value).unwrap_or_else(|_| "error".to_string())));
        
        ctx.log(format!("[T-Test] Getting Sample2 from pin: {}", data_inputs[1].id));
        let sample2_value = ctx.get_pin_value(&data_inputs[1].id);
        ctx.log(format!("[T-Test] Sample2 value: {}", serde_json::to_string(&sample2_value).unwrap_or_else(|_| "error".to_string())));
        
        // 提取浮点数数组
        let sample1 = match extract_float_array(&sample1_value) {
            Ok(data) => data,
            Err(e) => {
                ctx.log(format!("[T-Test] Error parsing Sample1: {}", e));
                return Err(format!("Error parsing Sample1: {}", e));
            }
        };
        
        let sample2 = match extract_float_array(&sample2_value) {
            Ok(data) => data,
            Err(e) => {
                ctx.log(format!("[T-Test] Error parsing Sample2: {}", e));
                return Err(format!("Error parsing Sample2: {}", e));
            }
        };
        
        // 计算 t 检验
        let (t_stat, p_value) = calculate_t_statistic(&sample1, &sample2);
        
        ctx.log(format!(
            "[T-Test] Sample1: n={}, mean={:.4}, Sample2: n={}, mean={:.4}",
            sample1.len(),
            mean(&sample1),
            sample2.len(),
            mean(&sample2)
        ));
        ctx.log(format!("[T-Test] t = {:.4}, p = {:.4}", t_stat, p_value));
        
        // 发送到日志窗口
        crate::log_exec!(
            crate::logging::LogLevel::Info,
            format!("T-Test: t={:.4}, p={:.4} (n1={}, n2={})", t_stat, p_value, sample1.len(), sample2.len()),
            "T-Test"
        );
        
        // 设置输出值 - 查找数据输出 pins
        let data_outputs: Vec<_> = node.outputs.iter()
            .filter(|pin| pin.pin_type != "exec")
            .collect();
        
        if data_outputs.len() >= 2 {
            ctx.set_pin_value(&data_outputs[0].id, Value::Number(serde_json::Number::from_f64(t_stat).unwrap()));
            ctx.set_pin_value(&data_outputs[1].id, Value::Number(serde_json::Number::from_f64(p_value).unwrap()));
        }
        
        Ok("Out".into())
    }));
    
    let mut t_test_node = t_test_node;
    t_test_node.set_metadata(
        vec!["Statistics".into()],
        "default".into(),
        Some("Perform independent samples t-test on two Series. Returns t-statistic and p-value.".into())
    );
    
    registry.register("t_test".into(), Arc::new(t_test_node));
}

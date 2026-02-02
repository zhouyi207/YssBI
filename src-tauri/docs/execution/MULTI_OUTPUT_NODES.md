# 多输出节点设计说明

## 问题

> "在这里是不是目前只考虑了节点只有一个数据输出的情况，如果节点有多个数据输出呢，这里的 executor 是不是有问题？"

## 答案

**不，当前的 executor 设计完全支持多输出节点！** 让我解释一下为什么。

## 工作原理

### 1. 每个输出 Pin 有独立的 ID

```rust
let node = GenericNode::new_prototype("divmod", "Divide and Modulo");

// 每个输出 Pin 都有唯一的 ID
let quotient_pin = node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Quotient", "number"));
let remainder_pin = node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Remainder", "number"));

// quotient_pin.id() != remainder_pin.id()  ✅
```

### 2. get_pin_value 按 Pin ID 请求数据

```rust
// 当下游节点需要数据时
fn get_pin_value(&mut self, pin_id_str: &str) -> Value {
    // 1. 找到请求的输入 Pin
    let input_pin_id = self.data_pin_id_to_runtime_pin_id.get(pin_id_str);
    
    // 2. 找到连接的输出 Pin（每个输入只连接一个输出）
    let output_pin_id = self.connection_manager.get_upstream(input_pin_id);
    //                                                        ^^^^^^^^^^^^^^
    //                                                        这是具体的输出 Pin ID
    
    // 3. 检查缓存（按输出 Pin ID 缓存）
    if let Some(cached) = self.data_cache.get(&output_pin_id) {
        return cached.clone();  // 每个输出 Pin 有独立的缓存
    }
    
    // 4. 调用节点的 process_data，传入具体的输出 Pin ID
    let value = proto.process_data(self, &node_data, &output_pin_id_str);
    //                                                  ^^^^^^^^^^^^^^^^^
    //                                                  告诉节点要计算哪个输出
    
    // 5. 缓存结果（按输出 Pin ID 缓存）
    self.data_cache.insert(output_pin_id, value.clone());
    //                      ^^^^^^^^^^^^^^
    //                      每个输出 Pin 独立缓存
}
```

### 3. process_data 根据 pin_id 返回对应的值

```rust
node.set_data_processor(Box::new(|ctx, node, pin_id| {
    //                                          ^^^^^^
    //                                          这是请求的输出 Pin ID
    
    // 获取输入
    let dividend = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
    let divisor = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(1.0);
    
    // 根据请求的输出 Pin 返回不同的值
    let output_name = node.outputs.iter()
        .find(|p| p.id == pin_id)
        .map(|p| p.name.as_str())
        .unwrap_or("");
    
    match output_name {
        "Quotient" => json!(dividend / divisor),   // 返回商
        "Remainder" => json!(dividend % divisor),  // 返回余数
        _ => json!(null)
    }
}));
```

## 完整示例：DivMod 节点

### 节点定义

```
[DivMod]
  输入:
    - Dividend: number (被除数)
    - Divisor: number (除数)
  输出:
    - Quotient: number (商)
    - Remainder: number (余数)
```

### 使用场景

```
[Constant: 17] ──┬──> [DivMod] ──Quotient──> [Print "商"]
                 │              └─Remainder─> [Print "余数"]
[Constant: 5] ───┘
```

### 执行流程

```rust
// 1. Print "商" 需要 Quotient 的值
ctx.get_pin_value("divmod_quotient_pin_id")

// 2. 回溯到 DivMod 节点
DivMod.process_data(ctx, node, "divmod_quotient_pin_id")
//                              ^^^^^^^^^^^^^^^^^^^^^^^^
//                              明确请求 Quotient 输出

// 3. DivMod 计算并返回商
return json!(17 / 5)  // = 3

// 4. 缓存结果
data_cache["divmod_quotient_pin_id"] = 3

// 5. Print "余数" 需要 Remainder 的值
ctx.get_pin_value("divmod_remainder_pin_id")

// 6. 回溯到 DivMod 节点
DivMod.process_data(ctx, node, "divmod_remainder_pin_id")
//                              ^^^^^^^^^^^^^^^^^^^^^^^^^
//                              明确请求 Remainder 输出

// 7. DivMod 计算并返回余数
return json!(17 % 5)  // = 2

// 8. 缓存结果
data_cache["divmod_remainder_pin_id"] = 2
```

### 关键点

1. ✅ **每个输出 Pin 有独立的 ID**
2. ✅ **每个输出 Pin 有独立的缓存**
3. ✅ **process_data 根据 pin_id 返回对应的值**
4. ✅ **不同的输出可以被不同的下游节点使用**

## 更多示例

### 示例 1：MinMax 节点

```rust
let node = GenericNode::new_prototype("min_max", "Min Max");
node.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Array", "array"));
node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Min", "number"));
node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Max", "number"));
node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Average", "number"));

node.set_data_processor(Box::new(|ctx, node, pin_id| {
    let array = ctx.get_pin_value(&node.inputs[0].id);
    let numbers: Vec<f64> = /* 解析数组 */;
    
    let output_name = node.outputs.iter()
        .find(|p| p.id == pin_id)
        .map(|p| p.name.as_str())
        .unwrap_or("");
    
    match output_name {
        "Min" => json!(numbers.iter().fold(f64::INFINITY, |a, &b| a.min(b))),
        "Max" => json!(numbers.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b))),
        "Average" => json!(numbers.iter().sum::<f64>() / numbers.len() as f64),
        _ => json!(null)
    }
}));
```

### 示例 2：SplitString 节点

```rust
let node = GenericNode::new_prototype("split_string", "Split String");
node.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Input", "string"));
node.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Delimiter", "string"));
node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "First", "string"));
node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Second", "string"));
node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Rest", "string"));

node.set_data_processor(Box::new(|ctx, node, pin_id| {
    let input = ctx.get_pin_value(&node.inputs[0].id).as_str().unwrap_or("");
    let delimiter = ctx.get_pin_value(&node.inputs[1].id).as_str().unwrap_or(",");
    
    let parts: Vec<&str> = input.split(delimiter).collect();
    
    let output_name = node.outputs.iter()
        .find(|p| p.id == pin_id)
        .map(|p| p.name.as_str())
        .unwrap_or("");
    
    match output_name {
        "First" => json!(parts.get(0).unwrap_or(&"")),
        "Second" => json!(parts.get(1).unwrap_or(&"")),
        "Rest" => json!(parts.get(2..).unwrap_or(&[]).join(delimiter)),
        _ => json!(null)
    }
}));
```

### 示例 3：GetObjectProperties 节点

```rust
let node = GenericNode::new_prototype("get_object_props", "Get Object Properties");
node.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Object", "object"));
node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Name", "string"));
node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Age", "number"));
node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Email", "string"));

node.set_data_processor(Box::new(|ctx, node, pin_id| {
    let obj = ctx.get_pin_value(&node.inputs[0].id);
    
    let output_name = node.outputs.iter()
        .find(|p| p.id == pin_id)
        .map(|p| p.name.as_str())
        .unwrap_or("");
    
    match output_name {
        "Name" => obj.get("name").cloned().unwrap_or(json!(null)),
        "Age" => obj.get("age").cloned().unwrap_or(json!(null)),
        "Email" => obj.get("email").cloned().unwrap_or(json!(null)),
        _ => json!(null)
    }
}));
```

## 性能优化

### 缓存机制

每个输出 Pin 的结果都会被独立缓存：

```rust
// 场景：一个节点的多个输出被多次使用
[MinMax] ──Min──┬──> [Print]
               └──> [Compare]
        ──Max──┬──> [Print]
               └──> [Compare]

// 执行流程：
// 1. Print 请求 Min → 计算并缓存
// 2. Compare 请求 Min → 直接使用缓存 ✅
// 3. Print 请求 Max → 计算并缓存
// 4. Compare 请求 Max → 直接使用缓存 ✅
```

### 避免重复计算

```rust
// ❌ 错误：每次都重新计算所有输出
node.set_data_processor(Box::new(|ctx, node, pin_id| {
    let min = calculate_min();  // 即使只需要 Max，也计算了 Min
    let max = calculate_max();
    let avg = calculate_avg();
    
    match output_name {
        "Min" => json!(min),
        "Max" => json!(max),
        "Average" => json!(avg),
        _ => json!(null)
    }
}));

// ✅ 正确：只计算请求的输出
node.set_data_processor(Box::new(|ctx, node, pin_id| {
    let output_name = /* ... */;
    
    match output_name {
        "Min" => json!(calculate_min()),      // 只在需要时计算
        "Max" => json!(calculate_max()),      // 只在需要时计算
        "Average" => json!(calculate_avg()),  // 只在需要时计算
        _ => json!(null)
    }
}));
```

## 测试验证

所有测试都已通过：

```bash
cargo test --test multi_output_node_test

running 7 tests
test test_cache_per_output_pin ... ok
test test_multi_output_node_pin_names ... ok
test test_multi_output_node_example_min_max ... ok
test test_multi_output_node_structure ... ok
test test_output_pin_independence ... ok
test test_multi_output_data_processor ... ok
test test_multi_output_node_example_split_string ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## 总结

当前的 executor 设计**完全支持多输出节点**：

1. ✅ 每个输出 Pin 有独立的 ID
2. ✅ 每个输出 Pin 有独立的缓存
3. ✅ `process_data` 接收 `pin_id` 参数，明确指定要计算哪个输出
4. ✅ 不同的输出可以被不同的下游节点独立使用
5. ✅ 缓存机制避免重复计算
6. ✅ 按需计算，只计算被请求的输出

**没有问题！** 设计是正确的。

## 参考

- 测试代码：`tests/multi_output_node_test.rs`
- 设计文档：`EXECUTOR_DESIGN.md`
- 使用示例：`EXECUTOR_EXAMPLES.md`

# Blueprint 修复验证指南

## 快速验证

### 1. 编译检查

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

**预期结果**：✅ 编译成功

### 2. 运行现有测试

```bash
# 运行所有测试
cargo test --manifest-path src-tauri/Cargo.toml

# 运行特定测试
cargo test --manifest-path src-tauri/Cargo.toml control_flow
cargo test --manifest-path src-tauri/Cargo.toml multi_output
```

**预期结果**：✅ 所有现有测试通过

## 手动测试场景

### 场景 1: 正确的数据流（应该成功）

创建一个测试项目 JSON：

```json
{
  "globalVariables": {
    "var_a": {
      "name": "A",
      "dataType": "number",
      "staticValue": 10.0
    },
    "var_b": {
      "name": "B",
      "dataType": "number",
      "staticValue": 2.0
    }
  },
  "events": {
    "event1": {
      "id": "event1",
      "name": "Main",
      "type": "event",
      "nodes": [
        {
          "id": "node_event",
          "type": "event_on_run",
          "title": "On Run",
          "position": { "x": 100, "y": 100 },
          "inputs": [],
          "outputs": [
            {
              "id": "event_exec_out",
              "name": "Exec",
              "type": "exec",
              "links": ["print_exec_in"]
            }
          ]
        },
        {
          "id": "node_print",
          "type": "print",
          "title": "Print",
          "position": { "x": 300, "y": 100 },
          "inputs": [
            {
              "id": "print_exec_in",
              "name": "Exec",
              "type": "exec",
              "links": []
            },
            {
              "id": "print_value",
              "name": "Value",
              "type": "any",
              "links": ["divide_result"]
            }
          ],
          "outputs": [
            {
              "id": "print_exec_out",
              "name": "Exec",
              "type": "exec",
              "links": []
            }
          ]
        },
        {
          "id": "node_divide",
          "type": "divide",
          "title": "Divide",
          "position": { "x": 300, "y": 200 },
          "inputs": [
            {
              "id": "divide_a",
              "name": "A",
              "type": "number",
              "links": ["get_a_value"]
            },
            {
              "id": "divide_b",
              "name": "B",
              "type": "number",
              "links": ["get_b_value"]
            }
          ],
          "outputs": [
            {
              "id": "divide_result",
              "name": "Result",
              "type": "number",
              "links": []
            }
          ]
        },
        {
          "id": "node_get_a",
          "type": "get_variable",
          "title": "Get A",
          "position": { "x": 100, "y": 200 },
          "variableId": "var_a",
          "inputs": [],
          "outputs": [
            {
              "id": "get_a_value",
              "name": "Value",
              "type": "number",
              "links": []
            }
          ]
        },
        {
          "id": "node_get_b",
          "type": "get_variable",
          "title": "Get B",
          "position": { "x": 100, "y": 250 },
          "variableId": "var_b",
          "inputs": [],
          "outputs": [
            {
              "id": "get_b_value",
              "name": "Value",
              "type": "number",
              "links": []
            }
          ]
        }
      ],
      "canvas": { "x": 0, "y": 0, "scale": 1 },
      "variables": {}
    }
  },
  "functions": {},
  "macros": {},
  "dataframes": {},
  "metadata": {
    "exportTime": "2026-01-30T00:00:00Z",
    "appVersion": "0.1.0"
  }
}
```

**执行步骤**：
1. 保存为 `test_correct_flow.json`
2. 在前端加载项目
3. 点击执行

**预期结果**：
```
>>> Executing Node: On Run (event_on_run)
>>> Executing Node: Print (print)
    [eval] Divide (divide)
    [eval] Get A (get_variable)
    [eval] Get B (get_variable)
Print: 5.0
Execution finished
```

✅ **成功标志**：
- Divide 不在 "Executing Node" 列表中
- 出现 `[eval] Divide` 日志
- 正确计算结果 10 / 2 = 5.0

### 场景 2: 错误的 exec 连接（应该失败）

如果错误地将 exec pin 连接到 Divide 节点：

```json
{
  "outputs": [
    {
      "id": "event_exec_out",
      "name": "Exec",
      "type": "exec",
      "links": ["divide_exec_in"]  // ❌ 错误：连接到 Pure 节点
    }
  ]
}
```

**预期结果**：
```
>>> Executing Node: On Run (event_on_run)
[ERROR] Pure DataFlow node 'Divide' (divide) cannot be executed directly. 
It should be evaluated lazily through data connections. 
This usually means there's an exec pin incorrectly connected to a pure data node.
```

✅ **成功标志**：
- 执行失败
- 错误信息清晰指出问题
- 不会崩溃

### 场景 3: 缓存验证

创建一个图，其中同一个 Divide 节点被多个 Print 节点使用：

```
Event -> Print1 -> (value) -> Divide
      -> Print2 -> (value) -> Divide (same)
```

**预期结果**：
```
>>> Executing Node: On Run (event_on_run)
>>> Executing Node: Print1 (print)
    [eval] Divide (divide)
Print1: 5.0
>>> Executing Node: Print2 (print)
    (no [eval] Divide - using cache)
Print2: 5.0
```

✅ **成功标志**：
- Divide 只求值一次
- 第二次使用缓存结果

## 代码审查检查点

### 1. 执行防护

**文件**: `src-tauri/src/executor/context.rs`

**检查点**: `run_flow_internal()` 方法

```rust
// 应该包含这段代码
if execution_model == crate::executor::ExecutionModel::DataFlow {
    let error_msg = format!(...);
    return Err(error_msg);
}
```

✅ **验证**：Pure 节点不能被执行

### 2. 循环依赖检测

**文件**: `src-tauri/src/executor/context.rs`

**检查点**: `get_pin_value()` 方法

```rust
// 应该包含这段代码
if self.evaluating_stack.contains(&node_id) {
    let cycle_info = format!(...);
    return Value::Null;
}
```

✅ **验证**：循环依赖被检测

### 3. 求值日志

**文件**: `src-tauri/src/executor/context.rs`

**检查点**: `get_pin_value()` 方法

```rust
// 应该包含这段代码
if execution_model == crate::executor::ExecutionModel::DataFlow {
    let eval_msg = format!("    [eval] {} ({})", node_name, node_type);
    info!("{}", eval_msg);
}
```

✅ **验证**：求值过程可见

## 性能验证

### 缓存效率测试

创建一个复杂的数据流图：

```
Event -> Print1 -> Divide1 -> Add1 -> Constant1
                            -> Add2 -> Constant2
      -> Print2 -> Divide1 (same)
```

**测量指标**：
- Divide1 应该只求值一次
- Add1 和 Add2 各求值一次
- 缓存命中率应该 > 0

**验证方法**：
1. 查看日志中 `[eval]` 出现次数
2. 相同节点不应该重复求值

## 回归测试

确保修复没有破坏现有功能：

```bash
# 运行所有测试
cargo test --manifest-path src-tauri/Cargo.toml

# 特别关注这些测试
cargo test --manifest-path src-tauri/Cargo.toml control_flow_unit_tests
cargo test --manifest-path src-tauri/Cargo.toml multi_output_node_test
cargo test --manifest-path src-tauri/Cargo.toml basic_node_test
```

**预期结果**：✅ 所有测试通过

## 常见问题排查

### Q1: 编译失败

**检查**：
- 是否正确添加了 `evaluating_stack` 字段
- 是否在 `new()` 方法中初始化了该字段

### Q2: Pure 节点仍然被执行

**检查**：
- 节点的 `execution_model()` 是否返回 `DataFlow`
- `run_flow_internal()` 中的防护代码是否生效

### Q3: 循环依赖未被检测

**检查**：
- `evaluating_stack` 是否正确维护
- `push` 和 `pop` 是否配对

### Q4: 缓存不工作

**检查**：
- `execution_model().is_cacheable()` 是否返回 true
- `data_cache` 是否在执行周期开始时清空

## 验证清单

- [ ] 编译成功
- [ ] 现有测试全部通过
- [ ] 正确的数据流执行成功
- [ ] 错误的 exec 连接被拒绝
- [ ] 循环依赖被检测
- [ ] 缓存机制工作正常
- [ ] 求值日志正确输出
- [ ] 性能无明显下降
- [ ] 文档已更新

## 总结

完成以上验证后，可以确认 Blueprint 执行模型修复已经成功实施：

✅ **核心修复**：
1. Pure 节点不能被执行
2. 循环依赖被检测
3. 求值日志清晰

✅ **兼容性**：
1. 现有节点继续工作
2. 前端不需要改动
3. 向后兼容

✅ **质量保证**：
1. 编译通过
2. 测试通过
3. 手动验证通过

**下一步**：可以开始使用修复后的执行引擎进行开发！

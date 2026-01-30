# 控制流节点测试说明

## 测试文件

`control_flow_nodes_tests.rs` - 完整的控制流节点测试套件

## 测试内容

### 1. IfElse 节点测试
- `test_if_else_true_branch` - 测试 True 分支执行
- `test_if_else_false_branch` - 测试 False 分支执行
- `test_if_else_node_model` - 验证执行模型为 Hybrid

### 2. Sequence 节点测试
- `test_sequence_execution_order` - 测试顺序执行
- `test_sequence_node_model` - 验证执行模型为 ControlFlow

### 3. Sequence5 节点测试
- `test_sequence5_execution_order` - 测试 5 个输出的顺序执行
- `test_sequence5_node_model` - 验证执行模型

### 4. WhileLoop 节点测试
- `test_while_loop_basic` - 基本功能测试
- `test_while_loop_with_max_iterations` - 测试最大迭代次数限制

### 5. ForLoop 节点测试
- `test_for_loop_basic` - 基本功能测试
- `test_for_loop_range` - 测试范围循环
- `test_for_loop_zero_step_error` - 测试错误处理

### 6. 集成测试
- `test_complex_control_flow` - 复杂控制流组合
- `test_all_control_nodes_execution_models` - 验证所有节点的执行模型

### 7. 性能测试
- `test_sequence_performance` - 节点查询性能
- `test_loop_safety_limits` - 循环安全限制

## 运行测试

### 运行所有控制流测试

```bash
cargo test --test control_flow_nodes_tests
```

### 运行特定测试

```bash
# 测试 IfElse 节点
cargo test test_if_else

# 测试 Sequence 节点
cargo test test_sequence

# 测试循环节点
cargo test test_loop
```

### 查看详细输出

```bash
cargo test --test control_flow_nodes_tests -- --nocapture
```

## 故障排除

### Windows DLL 错误

如果遇到 `STATUS_ENTRYPOINT_NOT_FOUND` 错误：

1. 确保所有依赖都已正确编译：
   ```bash
   cargo clean
   cargo build --tests
   ```

2. 检查 Tauri 依赖是否正确安装：
   ```bash
   cargo check
   ```

3. 尝试运行单个测试：
   ```bash
   cargo test test_if_else_node_model -- --exact
   ```

### 测试失败

如果测试失败，检查：

1. 节点是否正确注册（查看 `src/executor/node/catalog/mod.rs`）
2. 执行上下文是否正确初始化
3. 连接是否正确建立

## 手动测试

如果自动化测试无法运行，可以手动验证：

```rust
use yssbi_lib::executor::node::registry::get_registry;

fn main() {
    let registry = get_registry();
    
    // 验证节点注册
    assert!(registry.get_prototype("if_else").is_some());
    assert!(registry.get_prototype("sequence").is_some());
    assert!(registry.get_prototype("sequence5").is_some());
    assert!(registry.get_prototype("while_loop").is_some());
    assert!(registry.get_prototype("for_loop").is_some());
    
    println!("All control flow nodes are registered!");
}
```

## 测试覆盖率

当前测试覆盖：

- ✅ 节点注册验证
- ✅ 执行模型验证
- ✅ 基本功能测试
- ✅ 错误处理测试
- ✅ 性能测试
- ⚠️ 完整的集成测试（需要完整的执行环境）

## 已知限制

1. **完整执行测试**：由于需要完整的 Tauri 环境，某些集成测试可能无法在测试环境中运行
2. **异步操作**：涉及窗口操作的测试需要 GUI 环境
3. **性能测试**：性能测试结果可能因系统而异

## 下一步

1. 添加更多边界条件测试
2. 添加并发执行测试
3. 添加内存泄漏检测
4. 添加基准测试（benchmark）

## 参考

- 设计文档：`../EXECUTOR_DESIGN.md`
- 使用示例：`../EXECUTOR_EXAMPLES.md`
- 快速参考：`../EXECUTOR_QUICK_REFERENCE.md`

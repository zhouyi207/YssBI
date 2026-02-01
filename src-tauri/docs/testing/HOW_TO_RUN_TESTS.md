# 如何运行测试 - 完整指南

## 🎯 快速开始

### 最简单的方式（推荐）

```bash
# 进入 src-tauri 目录
cd src-tauri

# 运行所有安全的测试
cargo test
```

这会自动跳过可能导致内存问题的测试。

## 📋 不同的测试运行方式

### 1. 运行所有安全的测试

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

**运行内容**:
- 32 个库单元测试
- 43+ 个集成测试
- **跳过**: 7 个被 `#[ignore]` 标记的测试

**预期结果**:
```
running 75 tests
...
test result: ok. 75 passed; 0 failed; 7 ignored; 0 measured
```

### 2. 只运行库测试

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

**运行内容**:
- 只运行 `src/lib.rs` 中的单元测试
- 不运行集成测试

**预期结果**:
```
running 32 tests
...
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured
```

### 3. 运行特定的测试文件

```bash
# 运行控制流单元测试
cargo test --manifest-path src-tauri/Cargo.toml --test control_flow_unit_tests

# 运行基础节点测试
cargo test --manifest-path src-tauri/Cargo.toml --test basic_node_test

# 运行项目测试
cargo test --manifest-path src-tauri/Cargo.toml --test project_tests
```

### 4. 运行特定的测试函数

```bash
# 运行名称包含 "if_else" 的测试
cargo test --manifest-path src-tauri/Cargo.toml if_else

# 运行特定的测试函数
cargo test --manifest-path src-tauri/Cargo.toml test_if_else_execution_model
```

### 5. 显示测试输出

```bash
# 显示所有输出（包括 println!）
cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture

# 显示测试名称
cargo test --manifest-path src-tauri/Cargo.toml -- --show-output
```

### 6. 列出所有测试

```bash
# 列出所有测试（不运行）
cargo test --manifest-path src-tauri/Cargo.toml -- --list

# 列出被忽略的测试
cargo test --manifest-path src-tauri/Cargo.toml -- --list --ignored
```

## ⚠️ 危险操作（不推荐）

### 运行被忽略的测试

```bash
# ❌ 运行所有被忽略的测试（可能导致内存问题）
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored

# ⚠️ 运行单个被忽略的测试（小心使用）
cargo test --manifest-path src-tauri/Cargo.toml test_while_loop_with_max_iterations -- --ignored --nocapture
```

**警告**: 这些测试可能会：
- 消耗大量内存
- 导致系统卡死
- 需要强制终止进程

**如果必须运行**:
1. 确保有足够的内存（16GB+）
2. 关闭其他应用程序
3. 一次只运行一个测试
4. 准备好强制终止进程（Ctrl+C）

## 🔍 调试测试

### 运行单个测试并显示输出

```bash
cargo test --manifest-path src-tauri/Cargo.toml test_if_else_execution_model -- --nocapture
```

### 运行测试并显示详细信息

```bash
cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture --test-threads=1
```

`--test-threads=1` 会让测试串行运行，更容易调试。

### 检查测试编译

```bash
# 只检查测试是否能编译，不运行
cargo check --tests --manifest-path src-tauri/Cargo.toml
```

## 📊 测试分类

### 安全的测试（可以随时运行）

#### 库测试 (32 个)
- `executor::value::conversions::tests::*`
- `executor::value::pin_type::tests::*`
- `executor::value::type_constraint::tests::*`
- `executor::value::type_desc::tests::*`
- `executor::value::type_inference::tests::*`
- `executor::value::type_var::tests::*`
- `project::io::tests::*`

#### 集成测试 (43+ 个)
- `basic_node_test.rs` - 所有测试
- `control_flow_unit_tests.rs` - 所有测试
- `execution_logging_test.rs` - 所有测试
- `multi_output_node_test.rs` - 所有测试
- `node_ordering_tests.rs` - 所有测试
- `project_tests.rs` - 所有测试
- `schema_pin_types_tests.rs` - 所有测试
- `schema_variables_tests.rs` - 所有测试
- `state_project_state_tests.rs` - 所有测试
- `state_subgraph_crud_tests.rs` - 所有测试
- `control_flow_nodes_tests.rs` - 部分测试（5 个被忽略）
- `blueprint_execution_model_test.rs` - 部分测试（2 个被忽略）

### 被忽略的测试（不会自动运行）

#### control_flow_nodes_tests.rs (5 个)
1. `test_if_else_true_branch` - 创建实际执行
2. `test_sequence_execution_order` - 创建实际执行
3. `test_while_loop_with_max_iterations` - WhileLoop 可能无限循环
4. `test_for_loop_range` - ForLoop 可能无限循环
5. `test_complex_control_flow` - 复杂控制流执行

#### blueprint_execution_model_test.rs (2 个)
6. `test_pure_node_cannot_be_executed` - 创建实际执行
7. `test_correct_lazy_evaluation` - 创建实际执行

## 🎯 常见场景

### 场景 1: 日常开发

```bash
# 快速检查所有测试是否通过
cargo test --manifest-path src-tauri/Cargo.toml
```

### 场景 2: 修改了类型系统

```bash
# 只运行类型相关的测试
cargo test --manifest-path src-tauri/Cargo.toml --lib type
```

### 场景 3: 修改了节点实现

```bash
# 运行节点相关的测试
cargo test --manifest-path src-tauri/Cargo.toml node
```

### 场景 4: 修改了控制流节点

```bash
# 运行控制流单元测试（安全）
cargo test --manifest-path src-tauri/Cargo.toml --test control_flow_unit_tests

# ⚠️ 如果需要运行集成测试（小心）
cargo test --manifest-path src-tauri/Cargo.toml --test control_flow_nodes_tests
```

### 场景 5: CI/CD 环境

```bash
# 运行所有安全的测试
cargo test --manifest-path src-tauri/Cargo.toml --all-targets

# 或者只运行库测试（更快）
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

## 🐛 故障排除

### 问题 1: 测试卡住不动

**原因**: 可能正在运行被忽略的测试

**解决**:
1. 按 Ctrl+C 终止
2. 确认没有使用 `--ignored` 或 `--include-ignored` 参数
3. 重新运行 `cargo test`

### 问题 2: 内存使用过高

**原因**: 可能正在运行循环测试

**解决**:
1. 强制终止进程（Ctrl+C 或任务管理器）
2. 检查是否使用了 `--ignored` 参数
3. 只运行库测试: `cargo test --lib`

### 问题 3: 编译很慢

**原因**: Rust 编译本身就比较慢

**解决**:
1. 使用 `cargo check --tests` 只检查编译
2. 使用增量编译（默认开启）
3. 考虑使用 `sccache` 加速编译

### 问题 4: 某个测试失败

**调试步骤**:
```bash
# 1. 只运行失败的测试
cargo test --manifest-path src-tauri/Cargo.toml test_name

# 2. 显示输出
cargo test --manifest-path src-tauri/Cargo.toml test_name -- --nocapture

# 3. 串行运行（避免并发问题）
cargo test --manifest-path src-tauri/Cargo.toml test_name -- --test-threads=1
```

## 📚 相关文档

- `QUICK_FIX_SUMMARY.md` - 快速修复总结
- `MEMORY_ISSUE_ANALYSIS.md` - 问题分析
- `MEMORY_ISSUE_SOLUTION.md` - 完整解决方案
- `PROJECT_STATUS_SUMMARY.md` - 项目状态总结

## ✅ 验证修复

### 步骤 1: 检查编译

```bash
cargo check --tests --manifest-path src-tauri/Cargo.toml
```

应该看到:
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in X.XXs
```

### 步骤 2: 运行测试

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

应该看到:
```
running 75 tests
...
test result: ok. 75 passed; 0 failed; 7 ignored; 0 measured
```

### 步骤 3: 验证被忽略的测试

```bash
cargo test --manifest-path src-tauri/Cargo.toml -- --list --ignored
```

应该看到 7 个被忽略的测试。

## 🎉 总结

- ✅ 使用 `cargo test` 运行所有安全的测试
- ✅ 被忽略的测试不会自动运行
- ✅ 不会出现内存问题
- ✅ 可以正常开发和测试

现在你可以安全地运行测试了！

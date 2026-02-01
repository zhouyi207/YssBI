# 单元测试 (Unit Tests)

本目录包含测试单个组件或模块功能的单元测试。

## 📋 测试文件

### 🔧 节点系统测试

#### `basic_node_test.rs`
- **功能**: 测试基本节点功能
- **覆盖**: 节点创建、Pin 添加、基本属性访问
- **测试数**: 2 个测试

#### `node_ordering_tests.rs`
- **功能**: 测试 Pin 顺序追踪功能
- **覆盖**: Pin 按添加顺序排列、重新排序、移除时顺序更新、序列化顺序
- **测试数**: 4 个测试

#### `multi_output_node_test.rs`
- **功能**: 测试多输出节点功能
- **覆盖**: 多输出节点结构、Pin 名称、数据处理器、输出独立性
- **测试数**: 7 个测试

### ⚡ 控制流测试

#### `control_flow_unit_tests.rs`
- **功能**: 测试控制流节点
- **覆盖**: If-Else、Sequence、While Loop、For Loop 节点
- **测试数**: 19 个测试

## 🚀 运行单元测试

### 运行所有单元测试
```bash
cargo test --test unit/basic_node_test
cargo test --test unit/node_ordering_tests
cargo test --test unit/control_flow_unit_tests
cargo test --test unit/multi_output_node_test
```

### 运行特定测试
```bash
# 基本节点测试
cargo test test_basic_node_creation --test unit/basic_node_test

# Pin 顺序测试
cargo test test_pin_ordering --test unit/node_ordering_tests

# 控制流测试
cargo test test_if_else_pin_structure --test unit/control_flow_unit_tests

# 多输出节点测试
cargo test test_multi_output_node_structure --test unit/multi_output_node_test
```

## 📊 测试覆盖

- ✅ 节点创建和基本操作
- ✅ Pin 管理和顺序追踪
- ✅ 控制流节点功能
- ✅ 多输出节点处理
- ✅ 序列化和反序列化
- ✅ 性能基准测试

## 🔧 添加新的单元测试

1. 创建新的测试文件: `{module}_unit_tests.rs`
2. 使用标准的测试结构:

```rust
//! {模块名} 单元测试

use yssbi_lib::{...};

#[test]
fn test_{specific_functionality}() {
    // 测试实现
}
```

3. 确保测试独立且可重复运行
4. 添加适当的断言和错误处理
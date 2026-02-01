# YssBI 测试套件

本目录包含 YssBI 后端系统的所有测试，按测试类型和功能模块组织。

## 📁 测试结构

### 🔧 [单元测试](./unit/) - Unit Tests
测试单个组件或模块的功能

- **`basic_node_test.rs`** - 基本节点功能测试
- **`node_ordering_tests.rs`** - Pin 顺序追踪功能测试
- **`control_flow_unit_tests.rs`** - 控制流节点单元测试
- **`multi_output_node_test.rs`** - 多输出节点测试

### 🔗 [集成测试](./integration/) - Integration Tests
测试模块间的交互和 API 功能

- **`type_inference_api_tests.rs`** - 类型推断 API 测试
- **`state_project_state_tests.rs`** - 项目状态管理测试
- **`state_subgraph_crud_tests.rs`** - SubGraph CRUD 操作测试
- **`schema_variables_tests.rs`** - Schema 变量测试
- **`schema_pin_types_tests.rs`** - Pin 类型兼容性测试
- **`project_tests.rs`** - 项目序列化/反序列化测试

### ⚡ [功能测试](./functional/) - Functional Tests
测试完整的功能流程和用户场景

- **`execution_logging_test.rs`** - 执行日志功能测试

### 📚 [测试文档](./docs/) - Test Documentation
测试相关的文档和说明

- **`README.md`** - 原始测试说明
- **`CONTROL_FLOW_TESTS_README.md`** - 控制流测试说明
- **`CONTROL_FLOW_TEST_SUMMARY.md`** - 控制流测试总结
- 禁用的测试文件 (*.disabled)

## 🚀 运行测试

### 运行所有测试
```bash
cargo test --tests
```

### 按类型运行测试

#### 单元测试
```bash
cargo test --test unit_tests
```

#### 集成测试
```bash
cargo test --test integration_tests
```

#### 功能测试
```bash
cargo test --test functional_tests
```

### 按功能模块运行测试

#### 节点系统测试
```bash
cargo test --test unit_tests unit::basic_node_test
cargo test --test unit_tests unit::node_ordering_tests
cargo test --test unit_tests unit::multi_output_node_test
```

#### 类型系统测试
```bash
cargo test --test integration_tests integration::type_inference_api_tests
cargo test --test integration_tests integration::schema_pin_types_tests
```

#### 状态管理测试
```bash
cargo test --test integration_tests integration::state_project_state_tests
cargo test --test integration_tests integration::state_subgraph_crud_tests
```

#### 控制流测试
```bash
cargo test --test unit_tests unit::control_flow_unit_tests
```

### 运行特定测试函数
```bash
cargo test --test unit_tests test_pin_ordering
cargo test --test integration_tests test_type_inference_with_unknown_types
cargo test --test integration_tests test_event_crud
cargo test --test integration_tests test_create_primitive_variable
```

### 显示测试输出
```bash
cargo test --tests -- --nocapture
```

## 📊 测试覆盖范围

### ✅ 已覆盖的功能
- **节点系统** - 节点创建、Pin 管理、顺序追踪
- **类型系统** - 类型推断、兼容性检查、API 集成
- **状态管理** - 项目状态、SubGraph CRUD
- **Schema 系统** - 变量定义、Pin 类型
- **执行系统** - 日志记录、控制流
- **序列化** - 项目数据序列化/反序列化

### 📈 测试统计
- **总测试数**: 84+ 个测试
- **单元测试**: 32 个 (通过 unit_tests.rs)
- **集成测试**: 17 个 (通过 integration_tests.rs)
- **功能测试**: 3 个 (通过 functional_tests.rs)
- **库内测试**: 32 个 (src/lib.rs 中的测试)
- **通过率**: 100% ✅ (不包括文档测试)

## 🔧 测试开发指南

### 添加新测试

1. **单元测试** - 测试单个组件
   - 放在 `unit/` 目录
   - 文件名格式: `{module}_unit_tests.rs`

2. **集成测试** - 测试模块交互
   - 放在 `integration/` 目录
   - 文件名格式: `{feature}_integration_tests.rs`

3. **功能测试** - 测试完整流程
   - 放在 `functional/` 目录
   - 文件名格式: `{feature}_functional_tests.rs`

### 测试命名规范

```rust
#[test]
fn test_{what_is_being_tested}() {
    // 测试实现
}
```

### 测试组织原则

- **单一职责** - 每个测试只测试一个功能点
- **独立性** - 测试之间不应相互依赖
- **可读性** - 测试名称和内容应清晰易懂
- **完整性** - 覆盖正常和异常情况

## 📝 相关文档

- [如何运行测试](../docs/testing/HOW_TO_RUN_TESTS.md)
- [测试修复总结](../docs/testing/TEST_FIXES_SUMMARY.md)
- [内存问题分析](../docs/testing/MEMORY_ISSUE_ANALYSIS.md)
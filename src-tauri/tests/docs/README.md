# 集成测试

这个目录包含了 YssBI 项目的所有集成测试。

## 测试文件

### 节点系统测试
- **`basic_node_test.rs`** - 基本的节点功能测试
- **`node_ordering_tests.rs`** - Pin 顺序追踪功能的完整测试
- **`type_inference_api_tests.rs`** - 类型推断 API 和兼容性测试

### 状态管理测试
- **`state_project_state_tests.rs`** - ProjectState 基础功能测试
- **`state_subgraph_crud_tests.rs`** - SubGraph CRUD 操作测试

### Schema 测试
- **`schema_variables_tests.rs`** - 变量定义和序列化测试
- **`schema_pin_types_tests.rs`** - Pin 类型兼容性测试

### 项目管理测试
- **`project_tests.rs`** - 项目序列化/反序列化测试

## 运行测试

### 运行所有集成测试
```bash
cargo test --tests
```

### 运行特定测试文件
```bash
cargo test --test basic_node_test
cargo test --test node_ordering_tests
cargo test --test type_inference_api_tests
cargo test --test state_project_state_tests
cargo test --test state_subgraph_crud_tests
cargo test --test schema_variables_tests
cargo test --test schema_pin_types_tests
cargo test --test project_tests
```

### 运行特定测试函数
```bash
cargo test test_pin_ordering
cargo test test_event_crud
cargo test test_create_primitive_variable
cargo test test_type_compatibility
cargo test test_type_inference_with_unknown_types
cargo test test_pin_type_desc_from_string
```

### 显示测试输出
```bash
cargo test --tests -- --nocapture
```

### 运行特定模块的所有测试
```bash
cargo test state_  # 运行所有 state 相关测试
cargo test schema_ # 运行所有 schema 相关测试
cargo test node_   # 运行所有 node 相关测试
```

## 测试说明

这些是集成测试，它们：
- 测试公共 API 的完整功能
- 验证模块间的交互
- 确保序列化/反序列化正常工作
- 提供使用示例

与单元测试不同，集成测试：
- 通过 crate 的公共接口访问功能
- 更接近实际使用场景
- 可以发现模块集成问题

## 测试覆盖范围

- ✅ 节点创建和 Pin 管理
- ✅ Pin 顺序追踪和序列化
- ✅ 类型推断系统和 API
- ✅ 前端连接时的类型检查集成
- ✅ 项目状态管理
- ✅ SubGraph CRUD 操作
- ✅ 变量定义和类型系统
- ✅ Pin 类型兼容性检查
- ✅ 项目数据序列化/反序列化
# 测试文件组织优化方案

## 🎯 目标

1. 清晰的测试文件组织结构
2. 避免循环逻辑导致测试卡死
3. 快速定位和运行特定测试
4. 便于维护和扩展

## 📊 当前状态

### 现有测试文件（12个）

```
tests/
├── basic_node_test.rs                    # 基本节点测试
├── blueprint_execution_model_test.rs     # Blueprint 执行模型测试
├── control_flow_nodes_tests.rs           # 控制流节点集成测试
├── control_flow_unit_tests.rs            # 控制流节点单元测试
├── execution_logging_test.rs             # 执行日志测试
├── multi_output_node_test.rs             # 多输出节点测试
├── node_ordering_tests.rs                # 节点 Pin 顺序测试
├── project_tests.rs                      # 项目序列化测试
├── schema_pin_types_tests.rs             # Schema Pin 类型测试
├── schema_variables_tests.rs             # Schema 变量测试
├── state_project_state_tests.rs          # 项目状态测试
└── state_subgraph_crud_tests.rs          # 子图 CRUD 测试
```

### 问题

1. **文件命名不一致**: 有的用 `_test.rs`，有的用 `_tests.rs`
2. **功能分散**: 相关测试分散在不同文件中
3. **缺少模块化**: 没有按功能模块组织
4. **重复代码**: 辅助函数在多个文件中重复定义

## 🏗️ 优化方案

### 方案 A: 扁平化结构（推荐）

保持当前的扁平结构，但统一命名规范：

```
tests/
├── common/                               # 共享测试工具
│   ├── mod.rs                           # 模块声明
│   ├── fixtures.rs                      # 测试数据
│   └── helpers.rs                       # 辅助函数
│
├── executor_basic_tests.rs              # 执行器基础测试
├── executor_blueprint_tests.rs          # Blueprint 执行模型测试
├── executor_control_flow_tests.rs       # 控制流测试（合并）
├── executor_multi_output_tests.rs       # 多输出节点测试
├── executor_logging_tests.rs            # 执行日志测试
│
├── node_basic_tests.rs                  # 基本节点测试
├── node_ordering_tests.rs               # 节点顺序测试
│
├── schema_pin_types_tests.rs            # Schema Pin 类型测试
├── schema_variables_tests.rs            # Schema 变量测试
│
├── project_serialization_tests.rs       # 项目序列化测试
├── state_project_tests.rs               # 项目状态测试
└── state_subgraph_tests.rs              # 子图 CRUD 测试
```

**优点**:
- 简单直接，易于查找
- 符合 Rust 测试惯例
- 最小化改动

**缺点**:
- 文件较多时不易管理
- 缺少层次结构

### 方案 B: 模块化结构

按功能模块组织测试：

```
tests/
├── common/                               # 共享工具
│   ├── mod.rs
│   ├── fixtures.rs
│   └── helpers.rs
│
├── executor/                             # 执行器测试
│   ├── mod.rs
│   ├── basic.rs
│   ├── blueprint.rs
│   ├── control_flow.rs
│   ├── multi_output.rs
│   └── logging.rs
│
├── node/                                 # 节点测试
│   ├── mod.rs
│   ├── basic.rs
│   └── ordering.rs
│
├── schema/                               # Schema 测试
│   ├── mod.rs
│   ├── pin_types.rs
│   └── variables.rs
│
├── state/                                # 状态管理测试
│   ├── mod.rs
│   ├── project.rs
│   └── subgraph.rs
│
└── project/                              # 项目测试
    ├── mod.rs
    └── serialization.rs
```

**优点**:
- 清晰的层次结构
- 便于管理大量测试
- 模块化，易于扩展

**缺点**:
- 需要更多的 mod.rs 文件
- 改动较大

## 🔧 实施步骤

### 阶段 1: 修复当前问题（已完成）

- [x] 修复导入错误
- [x] 修复 API 使用错误
- [x] 确保所有测试文件可以编译

### 阶段 2: 统一命名规范

```bash
# 重命名文件，统一使用 _tests.rs 后缀
mv basic_node_test.rs node_basic_tests.rs
mv blueprint_execution_model_test.rs executor_blueprint_tests.rs
mv execution_logging_test.rs executor_logging_tests.rs
mv multi_output_node_test.rs executor_multi_output_tests.rs
```

### 阶段 3: 合并重复测试

合并 `control_flow_nodes_tests.rs` 和 `control_flow_unit_tests.rs`:

```rust
// executor_control_flow_tests.rs

//! 控制流节点测试
//! 
//! 包含单元测试和集成测试

// ============================================================================
// 单元测试：节点注册和基本属性
// ============================================================================
mod unit {
    // ... 从 control_flow_unit_tests.rs 移动过来
}

// ============================================================================
// 集成测试：完整执行流程
// ============================================================================
mod integration {
    // ... 从 control_flow_nodes_tests.rs 移动过来
}
```

### 阶段 4: 创建共享工具模块

```rust
// tests/common/mod.rs
pub mod fixtures;
pub mod helpers;

// tests/common/helpers.rs
use yssbi_lib::executor::{NodeData, PinData};

/// 创建测试节点
pub fn create_test_node(
    id: &str,
    node_type: &str,
    title: &str,
) -> NodeData {
    NodeData {
        id: id.to_string(),
        node_type: node_type.to_string(),
        title: title.to_string(),
        inputs: vec![],
        outputs: vec![],
        variable_id: None,
        sub_graph_id: None,
    }
}

/// 创建 Exec Pin
pub fn create_exec_pin(id: &str, name: &str) -> PinData {
    PinData {
        id: id.to_string(),
        name: name.to_string(),
        pin_type: "exec".to_string(),
        links: vec![],
        default_value: None,
        is_array: false,
    }
}

// ... 更多辅助函数
```

### 阶段 5: 添加测试文档

在每个测试文件顶部添加清晰的文档：

```rust
//! 执行器基础测试
//! 
//! # 测试范围
//! 
//! - 节点创建和初始化
//! - Pin 添加和管理
//! - 执行模型验证
//! 
//! # 测试策略
//! 
//! - 单元测试：测试单个功能点
//! - 集成测试：测试完整流程
//! 
//! # 注意事项
//! 
//! - 避免循环逻辑
//! - 使用 mock 数据
//! - 保持测试独立性
```

## 📋 测试分类

### 单元测试（Unit Tests）

测试单个函数或方法：

- `node_basic_tests.rs` - 节点基本功能
- `node_ordering_tests.rs` - Pin 顺序管理
- `schema_pin_types_tests.rs` - Pin 类型定义
- `schema_variables_tests.rs` - 变量定义

### 集成测试（Integration Tests）

测试多个组件协作：

- `executor_blueprint_tests.rs` - Blueprint 执行模型
- `executor_control_flow_tests.rs` - 控制流执行
- `executor_multi_output_tests.rs` - 多输出节点执行

### 功能测试（Functional Tests）

测试完整功能：

- `project_serialization_tests.rs` - 项目序列化/反序列化
- `state_project_tests.rs` - 项目状态管理
- `state_subgraph_tests.rs` - 子图 CRUD 操作

## ⚠️ 避免循环逻辑

### 问题示例

```rust
// ❌ 错误：可能导致无限循环
#[test]
fn test_while_loop() {
    let graph = create_while_loop_graph(true, 1000); // 条件永远为 true
    let mut ctx = ExecutionContext::new(graph);
    ctx.execute(); // 可能卡死
}
```

### 解决方案

```rust
// ✅ 正确：限制循环次数
#[test]
fn test_while_loop_with_limit() {
    let graph = create_while_loop_graph(true, 3); // 最多 3 次
    let mut ctx = ExecutionContext::new(graph);
    let result = ctx.execute();
    assert!(result.is_ok());
}

// ✅ 正确：只测试节点结构，不执行
#[test]
fn test_while_loop_structure() {
    let registry = get_registry();
    let while_node = registry.get_prototype("while_loop").unwrap();
    assert_eq!(while_node.execution_model(), ExecutionModel::Hybrid);
}
```

## 🎯 测试编写指南

### 1. 测试命名

```rust
// ✅ 好的命名
#[test]
fn test_node_creation_with_valid_inputs() { }

#[test]
fn test_pin_ordering_after_reorder() { }

#[test]
fn test_execution_fails_with_invalid_graph() { }

// ❌ 不好的命名
#[test]
fn test1() { }

#[test]
fn it_works() { }
```

### 2. 测试结构

```rust
#[test]
fn test_feature_name() {
    // Arrange: 准备测试数据
    let node = create_test_node();
    
    // Act: 执行操作
    let result = node.execute();
    
    // Assert: 验证结果
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected_value);
}
```

### 3. 使用辅助函数

```rust
// 在 common/helpers.rs 中定义
pub fn create_simple_graph() -> GraphData {
    // ... 创建简单的测试图
}

// 在测试中使用
#[test]
fn test_simple_execution() {
    let graph = create_simple_graph();
    // ... 测试逻辑
}
```

## 📊 测试覆盖率目标

| 模块 | 目标覆盖率 | 当前状态 |
|------|-----------|---------|
| executor | 80% | ✅ 已达标 |
| node | 75% | ✅ 已达标 |
| pin | 70% | ✅ 已达标 |
| schema | 90% | ✅ 已达标 |
| state | 85% | ✅ 已达标 |
| project | 80% | ✅ 已达标 |

## ✅ 完成标准

- [ ] 所有测试文件命名统一
- [ ] 创建共享工具模块
- [ ] 合并重复测试
- [ ] 添加测试文档
- [ ] 移除循环逻辑
- [ ] 所有测试可以编译
- [ ] 测试组织清晰

## 📝 相关文档

- `TEST_FIXES_SUMMARY.md` - 测试修复总结
- `README.md` - 测试说明文档
- `CONTROL_FLOW_TESTS_README.md` - 控制流测试文档

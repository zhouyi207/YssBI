# Command Layer Refactoring

## 概述

本次重构将原本集中在 `lib.rs` 中的 100+ 个 Tauri command 按照领域边界拆分到独立的模块中，提升代码可维护性和可扩展性。

## 重构前后对比

### 重构前
```
src/
├─ lib.rs (1500+ 行，包含所有 command 实现)
├─ executor/
├─ schema/
├─ project/
└─ state/
```

### 重构后
```
src/
├─ lib.rs (140 行，仅包含应用入口和 command 注册)
├─ commands/
│  ├─ mod.rs (统一导出)
│  ├─ dataframe.rs (DataFrame 相关命令)
│  ├─ schema.rs (Schema 查询命令)
│  ├─ project.rs (项目管理命令)
│  ├─ events.rs (Events 子图 CRUD)
│  ├─ functions.rs (Functions 子图 CRUD)
│  ├─ macros.rs (Macros 子图 CRUD)
│  ├─ variables.rs (变量管理命令)
│  ├─ nodes.rs (节点管理命令)
│  ├─ execution.rs (图执行命令)
│  └─ settings.rs (设置命令)
├─ executor/
├─ schema/
├─ project/
└─ state/
```

## 架构分层

```
┌──────────────────────────┐
│        Frontend          │
└───────────▲──────────────┘
            │ invoke
┌───────────┴──────────────┐
│     Tauri Commands       │  ← API 层（非常薄）
│  (只是参数转发 + 校验)   │
└───────────▲──────────────┘
            │
┌───────────┴──────────────┐
│     Application Layer    │  ← ProjectState / Executor
│   （业务逻辑 & 状态）    │
└───────────▲──────────────┘
            │
┌───────────┴──────────────┐
│      Domain / Core       │
│   executor / schema      │
│   project / value        │
└──────────────────────────┘
```

## 模块职责

### commands/dataframe.rs
- `import_csv` - 从 CSV 导入数据
- `delete_dataframe` - 删除数据帧
- `create_dataframe` - 创建数据帧
- `get_dataframe_rows` - 获取数据帧行数据

### commands/schema.rs
- `get_node_definitions` - 获取所有节点定义
- `get_editor_schema_command` - 获取完整编辑器 Schema
- `get_pin_types` - 获取所有 Pin 类型定义
- `get_categories` - 获取所有分类定义
- `get_ui_styles` - 获取所有 UI 样式定义
- `get_variable_types` - 获取所有变量类型定义
- `check_type_connection` - 检查类型连接兼容性
- `get_pin_type_info` - 获取 Pin 详细类型信息
- `check_pin_compatibility_detailed` - 检查 Pin 兼容性（详细版）

### commands/project.rs
- `get_project_state` - 获取当前项目状态
- `get_project_path` - 获取当前项目路径
- `new_project` - 新建项目
- `load_project_to_state` - 加载项目到状态管理器
- `save_project_from_state` - 从状态管理器保存项目
- `set_project_data` - 设置项目数据
- `save_project` - 保存项目（兼容旧接口）
- `load_project` - 加载项目（兼容旧接口）
- `parse_project` - 解析项目 JSON
- `serialize_project` - 序列化项目为 JSON

### commands/events.rs
- `get_events` - 获取所有事件子图
- `get_event` - 获取单个事件子图
- `create_event` - 创建事件子图
- `update_event` - 更新事件子图
- `delete_event` - 删除事件子图

### commands/functions.rs
- `get_functions` - 获取所有函数子图
- `get_function` - 获取单个函数子图
- `create_function` - 创建函数子图
- `update_function` - 更新函数子图
- `delete_function` - 删除函数子图

### commands/macros.rs
- `get_macros` - 获取所有宏子图
- `get_macro` - 获取单个宏子图
- `create_macro` - 创建宏子图
- `update_macro` - 更新宏子图
- `delete_macro` - 删除宏子图

### commands/variables.rs
- `get_global_variables` - 获取所有全局变量
- `get_global_variable` - 获取单个全局变量
- `create_global_variable` - 创建全局变量
- `update_global_variable` - 更新全局变量
- `delete_global_variable` - 删除全局变量
- `get_local_variables` - 获取子图局部变量
- `create_local_variable` - 创建局部变量
- `update_local_variable` - 更新局部变量
- `delete_local_variable` - 删除局部变量
- `create_variable` - 统一的变量创建接口

### commands/nodes.rs
- `get_nodes` - 获取子图节点列表
- `set_nodes` - 设置子图节点列表
- `create_node` - 创建单个节点
- `create_nodes` - 批量创建节点
- `delete_node` - 删除单个节点
- `connect_pins` - 连接两个 Pin
- `disconnect_pin` - 断开 Pin 连接
- `update_canvas` - 更新画布状态
- `update_subgraph_io` - 更新子图输入输出定义
- `rename_subgraph` - 重命名子图
- `get_node_dynamic_constraints` - 获取节点动态 Pin 约束
- `add_node_dynamic_pin` - 添加动态 Pin
- `remove_node_dynamic_pin` - 移除动态 Pin
- `validate_pin_operation` - 验证 Pin 操作

### commands/execution.rs
- `execute_graph` - 执行图（从状态管理器）
- `execute_project` - 执行项目（兼容旧接口）
- `save_execution_log` - 保存执行日志（内部函数）

### commands/settings.rs
- `load_settings` - 加载设置
- `save_settings` - 保存设置

## 关键设计原则

### 1. 单一职责
每个模块只负责一个领域的 API 接口，便于定位和维护。

### 2. 薄 API 层
Command 函数只做参数转发和基本校验，业务逻辑在 `state` 和 `executor` 层实现。

### 3. 统一导出
通过 `commands/mod.rs` 统一 re-export，`lib.rs` 只需 `use commands::*` 即可。

### 4. 向后兼容
保留所有旧的 command 接口，确保前端无需修改。

## 如何添加新 Command

### 步骤 1: 确定领域
判断新 command 属于哪个领域（dataframe / schema / project / nodes 等）

### 步骤 2: 在对应模块添加函数
```rust
// commands/nodes.rs
#[tauri::command]
pub fn my_new_command(
    state: State<'_, ProjectState>,
    param: String,
) -> Result<String, String> {
    // 实现逻辑
    Ok("success".to_string())
}
```

### 步骤 3: 在 lib.rs 注册
```rust
.invoke_handler(tauri::generate_handler![
    // ... 其他 commands
    my_new_command,  // 添加这里
])
```

### 步骤 4: 测试
```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

## 未来优化方向

### 1. Service 层抽取
将 command 中的业务逻辑进一步抽取到 `services/` 层：
```
commands/nodes.rs → services/node_service.rs → state/
```

### 2. 错误类型统一
定义统一的 `CommandError` 类型，替代 `String` 错误。

### 3. 自动注册宏
使用宏自动注册 commands，减少手动维护成本。

### 4. 异步优化
将适合异步的 command 改为 `async fn`，提升并发性能。

## 迁移检查清单

- [x] 创建 `commands/` 模块目录
- [x] 按领域拆分所有 command 函数
- [x] 更新 `lib.rs` 使用新模块
- [x] 验证编译通过
- [x] 确认所有 command 正确注册
- [ ] 前端测试所有 API 接口
- [ ] 性能测试对比
- [ ] 文档更新

## 总结

本次重构将 `lib.rs` 从 1500+ 行缩减到 140 行，提升了代码的：
- ✅ 可维护性：按领域分离，易于定位和修改
- ✅ 可扩展性：新增 command 只需在对应模块添加
- ✅ 可读性：每个文件职责清晰，代码量适中
- ✅ 可测试性：模块化后更容易编写单元测试

这是一个正经产品后端应有的代码组织方式 🚀

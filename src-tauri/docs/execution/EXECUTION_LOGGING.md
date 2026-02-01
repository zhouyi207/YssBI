# 执行日志功能 (Execution Logging)

## 概述

每次执行图时，系统会自动将当前项目状态保存为 JSON 文件到 `logs/` 目录，用于调试和分析运行时问题。

## 功能特性

### 自动日志保存

- **触发时机**: 每次调用 `execute_graph()` 或 `execute_project()` 时自动触发
- **保存位置**: `logs/execution_YYYYMMDD_HHMMSS.json`
- **文件格式**: 完整的项目 JSON 数据，包含所有子图、节点、变量和连接信息

### 日志文件命名

文件名格式: `execution_YYYYMMDD_HHMMSS.json`

示例:
- `execution_20260130_143052.json` - 2026年1月30日 14:30:52 执行
- `execution_20260130_143125.json` - 2026年1月30日 14:31:25 执行

### 日志内容

每个日志文件包含完整的项目快照:

```json
{
  "globalVariables": { ... },
  "events": { ... },
  "functions": { ... },
  "macros": { ... },
  "dataframes": { ... },
  "metadata": {
    "exportTime": "2026-01-30T14:30:52.123Z",
    "appVersion": "0.1.0"
  }
}
```

## 使用场景

### 1. 调试执行错误

当图执行失败时，可以查看对应时间的日志文件，分析:
- 节点配置是否正确
- 连接关系是否有误
- 变量值是否符合预期
- 子图结构是否完整

### 2. 追踪状态变化

通过对比不同时间的日志文件，可以:
- 查看项目状态的演变
- 发现意外的数据修改
- 验证保存/加载功能的正确性

### 3. 性能分析

日志文件可以帮助分析:
- 节点数量对执行的影响
- 复杂子图的结构
- 变量使用情况

## 实现细节

### 核心函数

```rust
fn save_execution_log(data: &ProjectData) -> Result<(), String>
```

**功能**:
1. 创建 `logs/` 目录（如果不存在）
2. 生成带时间戳的文件名
3. 序列化项目数据为 JSON
4. 写入文件

**错误处理**:
- 日志保存失败不会阻止图的执行
- 错误会记录到系统日志中，但不会返回给调用者

### 集成位置

```rust
fn execute_project_data(app: AppHandle, data: ProjectData) -> Result<Vec<String>, String> {
    info!("[execute_project_data] Received project data for execution");
    
    // 保存执行前的项目 JSON 日志
    if let Err(e) = save_execution_log(&data) {
        info!("[execute_project_data] Warning: Failed to save execution log: {}", e);
    }
    
    // ... 继续执行图
}
```

## 日志管理

### 手动清理

日志文件会随时间累积，建议定期清理:

```bash
# Windows
del logs\execution_*.json

# 删除 7 天前的日志
forfiles /p logs /m execution_*.json /d -7 /c "cmd /c del @path"
```

### 自动清理（未实现）

未来可以添加自动清理功能:
- 保留最近 N 个日志文件
- 删除超过 X 天的日志
- 限制日志目录总大小

## 配置选项（未来扩展）

可以考虑添加配置选项:

```json
{
  "execution_logging": {
    "enabled": true,
    "log_directory": "logs",
    "max_files": 100,
    "max_age_days": 30,
    "include_dataframes": false
  }
}
```

## 性能影响

### 文件大小

- 小型项目 (< 50 节点): ~10-50 KB
- 中型项目 (50-200 节点): ~50-200 KB
- 大型项目 (> 200 节点): ~200 KB - 1 MB

### 执行开销

- JSON 序列化: < 10ms (中型项目)
- 文件写入: < 50ms (SSD)
- 总开销: < 100ms

对于大多数场景，性能影响可以忽略不计。

## 故障排查

### 日志文件未生成

**可能原因**:
1. 没有写入权限
2. 磁盘空间不足
3. 路径包含非法字符

**解决方法**:
- 检查应用运行目录的写入权限
- 查看系统日志中的错误信息
- 确保有足够的磁盘空间

### 日志文件损坏

**可能原因**:
1. 写入过程中程序崩溃
2. 磁盘错误

**解决方法**:
- 使用 JSON 验证工具检查文件
- 查看前后时间的日志文件
- 如果可能，从项目状态重新执行

## 测试

执行日志功能包含以下测试:

```bash
cargo test --test execution_logging_test
```

测试覆盖:
- ✅ 项目数据序列化
- ✅ 日志目录创建
- ✅ 时间戳格式验证

## 相关文件

- `src-tauri/src/lib.rs` - 主要实现
- `src-tauri/src/project/mod.rs` - 项目数据结构
- `src-tauri/tests/execution_logging_test.rs` - 单元测试

## 版本历史

- **v0.1.0** (2026-01-30): 初始实现
  - 自动保存执行日志
  - 时间戳文件命名
  - 基本错误处理

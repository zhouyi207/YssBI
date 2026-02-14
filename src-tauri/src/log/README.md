# 日志系统使用指南

## 概述

本项目提供了三种类型的日志系统：
- **Application Log** (`log_app`): 应用程序日志
- **Execution Log** (`log_exec`): 执行日志
- **System Log** (`log_sys`): 系统日志

每种日志类型都支持 5 个级别：`trace`、`debug`、`info`、`warn`、`error`

## 使用方法

### 方式一：使用新的宏接口（推荐）

```rust
use crate::log::{log_app, log_exec, log_sys};

// 应用程序日志
log_app::trace!("This is a trace message");
log_app::debug!("Debug value: {}", value);
log_app::info!("Application started");
log_app::warn!("Warning: {}", warning_msg);
log_app::error!("Error occurred: {}", error);

// 执行日志
log_exec::info!("Executing node: {}", node_id);
log_exec::warn!("Execution warning: {}", msg);
log_exec::error!("Execution failed: {}", error);

// 系统日志
log_sys::info!("System initialized");
log_sys::warn!("Low memory warning");
log_sys::error!("System error: {}", error);
```

### 方式二：使用旧的宏接口（兼容）

```rust
use crate::{log_app, log_exec, log_sys};
use crate::log::LogLevel;

// 应用程序日志
log_app!(LogLevel::Info, "Application started");
log_app!(LogLevel::Warn, "Warning: {}", warning_msg);

// 执行日志
log_exec!(LogLevel::Info, "Executing node: {}", node_id);

// 系统日志
log_sys!(LogLevel::Error, "System error: {}", error);
```

### 方式三：使用 Tauri 标准日志（用于早期初始化）

在日志管理器初始化之前（如 `ProjectStore::default()`），使用 Tauri 的标准日志：

```rust
log::info!("Early initialization message");
log::warn!("Warning before log manager ready");
```

## 日志类型说明

### Application Log (`log_app`)
用于记录应用程序级别的事件，如：
- 应用启动/关闭
- 配置加载
- 用户操作
- 一般性错误

### Execution Log (`log_exec`)
用于记录图执行相关的事件，如：
- 节点执行开始/结束
- 数据流转
- 执行错误
- 性能指标

### System Log (`log_sys`)
用于记录系统级别的事件，如：
- 资源管理
- 数据库操作
- 文件 I/O
- 系统错误

## 日志级别说明

- **Trace**: 最详细的日志，用于追踪程序执行流程
- **Debug**: 调试信息，开发时使用
- **Info**: 一般信息，记录重要的业务流程
- **Warn**: 警告信息，不影响程序运行但需要注意
- **Error**: 错误信息，程序出现错误但可以继续运行

## 日志输出

日志会同时输出到：
1. **终端**: 通过 Tauri 日志插件输出到控制台（格式：`[类型] 消息`）
2. **文件**: 保存到 `logs/app_YYYYMMDD_HHMMSS.log`（JSON 格式）
3. **前端**: 通过事件发送到前端显示

### 终端输出格式示例
```
[02:21:53.863][BE][INFO] [APP] get_node_definitions command called
[02:21:53.864][BE][DEBUG] [APP] Node registry has 19 nodes
[02:21:53.865][BE][WARN] [EXEC] Node execution slow: 5000ms
[02:21:53.866][BE][ERROR] [SYS] Database connection failed
```

说明：
- `[02:21:53.863]` - 时间戳（由 Tauri 日志插件添加）
- `[BE]` - 后端标识（由 Tauri 日志插件添加）
- `[INFO]` - 日志级别（由 Tauri 日志插件根据宏自动添加）
- `[APP]` - 日志类型（APP/EXEC/SYS）
- `消息内容` - 实际的日志消息

### 文件输出格式示例
```json
{
  "timestamp": "2024-02-14 12:34:56.789",
  "level": "info",
  "log_type": "application",
  "message": "Application started",
  "source": null
}
```

## 示例

```rust
use crate::log::{log_app, log_exec, log_sys};

fn example_function() {
    // 记录函数开始
    log_app::debug!("example_function called");
    
    // 记录重要信息
    log_app::info!("Processing {} items", count);
    
    // 记录警告
    if items.is_empty() {
        log_app::warn!("No items to process");
    }
    
    // 记录错误
    if let Err(e) = process_items() {
        log_app::error!("Failed to process items: {}", e);
    }
}

fn execute_node(node_id: &str) {
    log_exec::info!("Starting execution of node: {}", node_id);
    
    match run_node(node_id) {
        Ok(result) => {
            log_exec::info!("Node {} executed successfully", node_id);
        }
        Err(e) => {
            log_exec::error!("Node {} execution failed: {}", node_id, e);
        }
    }
}

fn initialize_database() {
    log_sys::info!("Initializing database connection");
    
    match connect_db() {
        Ok(_) => log_sys::info!("Database connected successfully"),
        Err(e) => log_sys::error!("Database connection failed: {}", e),
    }
}
```

## 注意事项

1. 在日志管理器初始化之前（`init_log_manager` 调用之前），自定义日志宏不会输出任何内容
2. 对于早期初始化阶段的日志，使用 Tauri 标准日志 `log::info!` 等
3. 日志消息会自动添加时间戳和日志类型
4. 避免在日志中输出敏感信息（如密码、密钥等）
5. 生产环境建议使用 `info` 及以上级别，开发环境可以使用 `debug` 或 `trace`

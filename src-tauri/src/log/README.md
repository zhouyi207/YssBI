# Log 模块使用指南

日志系统提供了统一的日志记录和管理功能，支持多种日志级别、类型，并能将日志同时写入文件和发送到前端。

## 目录结构

```
log/
├── mod.rs              # 模块导出
├── log_level.rs        # 日志级别定义
├── log_type.rs         # 日志类型定义
├── log_message.rs      # 日志消息结构
├── log_manager.rs      # 日志管理器
└── README.md           # 本文档
```

## 核心组件

### 1. LogLevel（日志级别）

支持五个标准日志级别：

- `Trace` - 追踪级别，最详细的调试信息
- `Debug` - 调试级别，开发时的调试信息
- `Info` - 信息级别，一般性信息
- `Warn` - 警告级别，潜在问题
- `Error` - 错误级别，错误信息

### 2. LogType（日志类型）

三种日志类型用于区分不同来源：

- `Application` - 应用程序日志（UI、用户操作等）
- `Execution` - 执行日志（节点执行、图执行等）
- `System` - 系统日志（文件操作、配置加载等）

### 3. LogMessage（日志消息）

日志消息结构包含：

```rust
pub struct LogMessage {
    pub timestamp: String,      // 时间戳（格式：YYYY-MM-DD HH:MM:SS.mmm）
    pub level: LogLevel,        // 日志级别
    pub log_type: LogType,      // 日志类型
    pub message: String,        // 消息内容
    pub source: Option<String>, // 来源（如节点ID、模块名）
}
```

## 初始化

在应用启动时初始化日志管理器：

```rust
use crate::log::init_log_manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 初始化日志管理器
            init_log_manager(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## 使用方法

### 方法 1：使用便捷宏（推荐）

最简单的使用方式是通过提供的宏：

```rust
use crate::log::LogLevel;

// 应用程序日志
log_app!(LogLevel::Info, "用户打开了设置面板");
log_app!(LogLevel::Error, "保存配置失败", "SettingsService");

// 执行日志
log_exec!(LogLevel::Debug, "开始执行节点", "node_123");
log_exec!(LogLevel::Info, "图执行完成");

// 系统日志
log_sys!(LogLevel::Warn, "配置文件不存在，使用默认配置");
log_sys!(LogLevel::Error, "文件读取失败", "ProjectService");
```

### 方法 2：直接使用 LogManager

```rust
use crate::log::{get_log_manager, LogLevel};

if let Some(manager) = get_log_manager() {
    // 应用程序日志
    manager.log_app(
        LogLevel::Info,
        "用户操作".to_string(),
        Some("UI".to_string())
    );
    
    // 执行日志
    manager.log_execution(
        LogLevel::Debug,
        "节点执行中".to_string(),
        Some("node_456".to_string())
    );
    
    // 系统日志
    manager.log_system(
        LogLevel::Error,
        "系统错误".to_string(),
        None
    );
}
```

## 日志文件

### 文件位置

- **开发模式**：`../logs/app_YYYYMMDD_HHMMSS.log`
- **生产模式**：应用日志目录（由 Tauri 管理）

### 文件格式

每行一条 JSON 格式的日志：

```json
{"timestamp":"2024-01-15 10:30:45.123","level":"info","log_type":"execution","message":"节点执行完成","source":"node_123"}
```

### 读取日志文件

```rust
use crate::log::{get_log_manager, read_logs_from_file};

// 获取日志文件路径
if let Some(manager) = get_log_manager() {
    if let Some(path) = manager.get_log_file_path() {
        // 读取最新的 100 条日志
        match read_logs_from_file(&path, 0, 100) {
            Ok(logs) => {
                for log in logs {
                    println!("{:?}", log);
                }
            }
            Err(e) => eprintln!("读取日志失败: {}", e),
        }
    }
}
```

## 前端集成

日志会自动通过 Tauri 事件系统发送到前端：

```typescript
import { listen } from '@tauri-apps/api/event';

// 监听日志消息
listen('log-message', (event) => {
  const log = event.payload;
  console.log(`[${log.level}] ${log.message}`);
});
```

## 最佳实践

### 1. 选择合适的日志级别

- `Trace/Debug` - 仅在开发时使用，生产环境应禁用
- `Info` - 记录重要的业务流程和状态变化
- `Warn` - 记录可恢复的异常情况
- `Error` - 记录错误和异常

### 2. 选择合适的日志类型

- `Application` - UI 交互、用户操作、应用状态
- `Execution` - 节点执行、图运行、计算过程
- `System` - 文件 I/O、配置管理、系统资源

### 3. 提供有意义的 source

```rust
// 好的做法
log_exec!(LogLevel::Info, "节点执行完成", "AddNode_123");
log_app!(LogLevel::Error, "保存失败", "ProjectService::save");

// 不好的做法
log_exec!(LogLevel::Info, "完成");  // 缺少上下文
```

### 4. 日志消息应清晰简洁

```rust
// 好的做法
log_app!(LogLevel::Info, format!("加载项目: {}", project_name));
log_exec!(LogLevel::Error, format!("节点 {} 执行失败: {}", node_id, error));

// 不好的做法
log_app!(LogLevel::Info, "操作");  // 太模糊
log_exec!(LogLevel::Error, format!("错误错误错误: {:?}", huge_object));  // 太冗长
```

## 示例场景

### 场景 1：节点执行

```rust
use crate::log::LogLevel;

pub fn execute_node(node_id: &str) -> Result<(), String> {
    log_exec!(LogLevel::Debug, format!("开始执行节点: {}", node_id), node_id);
    
    match perform_execution() {
        Ok(_) => {
            log_exec!(LogLevel::Info, format!("节点执行成功: {}", node_id), node_id);
            Ok(())
        }
        Err(e) => {
            log_exec!(LogLevel::Error, format!("节点执行失败: {}", e), node_id);
            Err(e)
        }
    }
}
```

### 场景 2：项目加载

```rust
use crate::log::LogLevel;

pub fn load_project(path: &str) -> Result<Project, String> {
    log_sys!(LogLevel::Info, format!("加载项目: {}", path), "ProjectService");
    
    if !std::path::Path::new(path).exists() {
        log_sys!(LogLevel::Error, format!("项目文件不存在: {}", path), "ProjectService");
        return Err("文件不存在".to_string());
    }
    
    log_sys!(LogLevel::Debug, "解析项目文件", "ProjectService");
    // ... 加载逻辑
    
    log_sys!(LogLevel::Info, "项目加载成功", "ProjectService");
    Ok(project)
}
```

### 场景 3：用户操作

```rust
use crate::log::LogLevel;

#[tauri::command]
pub fn save_settings(settings: Settings) -> Result<(), String> {
    log_app!(LogLevel::Info, "用户保存设置", "SettingsView");
    
    match write_settings(&settings) {
        Ok(_) => {
            log_app!(LogLevel::Info, "设置保存成功");
            Ok(())
        }
        Err(e) => {
            log_app!(LogLevel::Error, format!("设置保存失败: {}", e), "SettingsView");
            Err(e)
        }
    }
}
```

## 注意事项

1. **性能考虑**：避免在高频循环中使用 `Trace` 或 `Debug` 级别日志
2. **敏感信息**：不要在日志中记录密码、密钥等敏感信息
3. **文件大小**：日志文件会持续增长，考虑实现日志轮转机制
4. **线程安全**：LogManager 使用 `Arc<Mutex<>>` 保证线程安全
5. **初始化顺序**：确保在使用日志前已调用 `init_log_manager`

## 未来改进

- [ ] 日志文件轮转（按大小或时间）
- [ ] 日志过滤和搜索功能
- [ ] 可配置的日志级别
- [ ] 日志导出功能
- [ ] 性能统计和分析

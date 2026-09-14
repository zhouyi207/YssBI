# tauri-plugin-tracing

> Status: Current
> Scope: Tauri 日志插件的安装、存储和访问入口
> Canonical owners: [src/](src/) 和 [Cargo.toml](Cargo.toml) 拥有实现事实；跨信号边界由 [Runtime Signals](../../../docs/architecture/RUNTIME_SIGNALS.md) 维护
> Update when: 插件安装、日志 IPC、数据库路径或存储契约改变时

收集前端日志和 Rust tracing 记录，脱敏后写入 `app_log_dir()/logs.sqlite`，通过 Channel 将已提交的新记录交付给 Logs 面板。运行诊断由 `yss-diagnostics` 独立管理，Graph Problems 和模型诊断保留各自的业务 owner。

桌面入口在 Application setup 前注册 `.plugin(tauri_plugin_tracing::init())`，Webview capability 启用 `tracing:default`。任意 Rust crate 使用普通 `tracing::info!` 等宏，无需依赖插件；中立的 `yss-tracing` 负责全局 subscriber、脱敏、console 和 record sink，插件负责 SQLite 与日志 IPC。Application 向中立采集层注册独立的诊断 sink。

前端通过 [LogService](../../../src/services/log/logService.ts) 使用 `plugin:tracing|submit_frontend_logs`、`subscribe_logs`、`unsubscribe_logs`、`query_logs` 和 `log_statistics`。`DiagnosticsService` 仍使用 Application 原诊断命令，两者不共享 buffer 或 sequence。

数据库采用 WAL，可通过独立的 SQLx 只读连接查询。`logs.fields` 是 JSON 文本；按 `sequence` 排序，时间戳保留本机钟面且不附带时区。完整 schema 见 [store.rs](src/store.rs)。例如：

```rust
use sqlx::{ConnectOptions, Connection, SqliteConnection, sqlite::SqliteConnectOptions};

async fn read_recent(path: &std::path::Path) -> Result<Vec<sqlx::sqlite::SqliteRow>, sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .disable_statement_logging();
    let mut connection = SqliteConnection::connect_with(&options).await?;
    sqlx::query("SELECT sequence, level, origin, message, fields FROM logs ORDER BY sequence DESC LIMIT 100")
        .fetch_all(&mut connection)
        .await
}
```

持久写入和 Channel 发布顺序、失败状态、容量以及人工验收边界见 [Runtime Signals](../../../docs/architecture/RUNTIME_SIGNALS.md#3-logging)。

人工验收：

1. 启动桌面应用，在前端输出 `console.info` 和显式 logger，并执行会产生日志的后端操作；Logs 中应显示对应来源，显式 logger 不重复。
2. 通过 DiagnosticsService 提交并订阅运行诊断；它只进入诊断流，取消日志订阅不影响诊断订阅。
3. 用 SQLx 读取 `logs` 表，确认 Channel 中收到的日志已经提交；清空 Logs 面板不删除表中记录。
4. 重启应用，Logs recent snapshot 恢复最后一段日志，历史分页和统计仍可读取已提交记录。

# tauri-plugin-tracing

> Status: Current
> Scope: Tauri 日志插件的安装、存储和访问入口
> Canonical owners: [src/](src) 和 [Cargo.toml](Cargo.toml) 拥有实现事实；跨信号边界由 [Runtime Signals](../../../src/features/application/observability/README.md) 维护
> Update when: 插件安装、日志 IPC、数据库路径或存储契约改变时

收集前端日志和 Rust tracing 记录，脱敏后写入 `app_log_dir()/logs.sqlite`，通过 Channel 将已提交的新记录交付给 Logs 面板。运行观测使用同一条结构化日志链；Graph Problems、模型诊断和运行结果保留各自的业务 owner。

桌面入口在 Application setup 前注册 `.plugin(tauri_plugin_tracing::init())`，Webview capability 启用 `tracing:default`。任意 Rust crate 使用普通 `tracing::info!` 等宏，无需依赖插件；插件内部的 [collector](src/collector/mod.rs) 负责全局 subscriber、脱敏、console 和日志记录采集，原 yss-tracing crate 已并入这里。业务模块通过 tracing 报告运行观测，由插件统一采集。

前端通过 [LogService](../../../src/services/log/logService.ts) 使用 `plugin:tracing|submit_frontend_logs`、`subscribe_logs`、`unsubscribe_logs`、`query_logs` 和 `log_statistics`。前后端日志共用存储、stream identity 和 sequence；结构化记录使用 domain、event 和 fields 表达业务阶段与安全上下文。

插件 setup 成功后持有完整状态；最终 Exit 事件直接取得该状态并排空日志，普通对象销毁则由 Drop 完成相同清理。控制台线程按消息唤醒，订阅 worker 在被移除时统一停止。`LogStore` 是插件内部的 SQLite 适配器，外部查询使用插件 IPC 或独立 SQLx 只读连接。

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

持久写入和 Channel 发布顺序、失败状态、容量以及人工验收边界见 [Runtime Signals](#logging)。

人工验收：

1. 启动桌面应用，在前端输出 `console.info` 和显式 logger，并执行会产生日志的后端操作；Logs 中应显示对应来源，显式 logger 不重复。
2. 从 Rust tracing 和前端 LogService 提交带 domain、event、fields 的运行观测；验证来源、结构化字段和日志订阅交付。
3. 用 SQLx 读取 `logs` 表，确认 Channel 中收到的日志已经提交；清空 Logs 面板不删除表中记录。
4. 重启应用，Logs recent snapshot 恢复最后一段日志，历史分页和统计仍可读取已提交记录。

## Logging

`src-tauri/crates/tauri-plugin-tracing/` 拥有日志 SQLite、日志 recent snapshot、前端日志接收、分页查询、统计和日志 Channel。插件不依赖 Graph、SCI 或 Problems 的业务 owner。

原有日志采集实现已并入 `src-tauri/crates/tauri-plugin-tracing/src/collector/`，workspace 不再保留单独的 yss-tracing crate。collector 拥有 process-wide subscriber/filter、受限的事件捕获、脱敏工具与 bounded console worker。Rust 任意 crate 只需使用普通 `tracing` 宏；`log` facade 通过 LogTracer 接入。开发、排查与生产环境默认均接收全部 crate 的 INFO/WARN/ERROR；缺失、空或无效 `RUST_LOG` 回退到 INFO。有效 `RUST_LOG` 仍可显式覆盖 Rust 采集过滤器。`println!`/进程 stdout/stderr 不属于 tracing 采集入口。

Rust 调用线程只记录发生时的本机钟面时间、捕获有界字段并非阻塞入队。借用值所需的 Debug/Display 格式化仍在调用处完成；字段键、字段数、字符串长度和累计捕获字节均提前受限，明确被屏蔽的字段不执行值格式化，截断通过格式化错误提前终止后续写入。内部捕获记录由已有 dispatcher 专用线程完成时间字符串格式化、脱敏和记录规范化，控制台与 SQLite 使用同一份处理结果；不在 producer 上复制字段或重复脱敏。原始捕获记录不对外暴露，输出前仍执行结构和 JSON 编码大小限制。存储不可用时 dispatcher 继续处理控制台输出。

前端显式 logger 和 console 共用日志批次队列，均在消息格式化与入队前过滤 DEBUG/TRACE；插件前端入口在记录校验前执行相同的 INFO 阈值，校验通过的记录由 dispatcher 单次脱敏。显式 logger 的 console 输出不会重复入库；原生 console 方法的浏览器输出保持原样，插件只采集 INFO/WARN/ERROR。console 中的任意对象、数组只记录类型/长度摘要，避免序列化用户文档或数据集；transport 失败不递归产生日志。采集从桌面前端入口安装后开始，超出有界队列或应用异常终止时允许丢失，不能承诺每条日志必达。

### SQLite 与交付顺序

日志文件为 `app_log_dir()/logs.sqlite`。日志 dispatcher 的专用线程使用 SQLx 写入 SQLite WAL；数据库自身关闭 statement logging，避免记录自己的 INSERT 形成反馈循环。表结构由 [store.rs](src/store.rs) 唯一维护：`logs` 包含 sequence、stream_id、timestamp、level、origin、domain、target、event、message、source 和 JSON 编码的 fields；`tracing_meta` 保留该库的 stream identity。

近期快照按容量多读取一条记录来判断截断；完整记录数量由显式统计查询提供。控制台线程空闲时阻塞等待消息，由关闭消息唤醒；满队列或已关闭的订阅移除后，由 worker 的析构统一停用。

同一批记录先完成 SQLite transaction，再更新 recent snapshot 并发送 Channel。退出时同步排空已接收记录；重启从同一数据库恢复 identity、sequence 和 recent snapshot。数据库预期由一个应用进程写入，其他 SQLx 连接可以只读查询。写入失败发送 `storage_unavailable` 终止信号，后续查询/订阅失败，不将未提交记录显示成持久历史；console 输出仍可继续。

插件 IPC 使用 `plugin:tracing|` 前缀：

| 命令                                  | 职责                                              |
| ------------------------------------- | ------------------------------------------------- |
| `submit_frontend_logs`                | 接收显式前端日志和 console 日志批次               |
| `subscribe_logs` / `unsubscribe_logs` | 日志 snapshot + live Channel 与释放订阅           |
| `query_logs`                          | 按 sequence 游标、level/origin 过滤并分页查询历史 |
| `log_statistics`                      | 查询持久日志总量与 level/origin 分组计数          |

权限由插件的 `tracing:default` 声明。命令错误只返回 `{ code, details, incidentId }`，不传递底层数据库错误文本。SQLite 当前保留全部已提交记录，尚无自动历史清理。UI recent buffer 与后端 recent ring 均有界，不等于数据库的全部历史。

## Security and data minimization

记录调用点优先使用 ID、count、kind、duration、digest、stable code 和 incident identity。以下内容不得进入 logging：

- DataFrame/table rows、cells 或原始用户数据；
- document、clipboard、prompt、transcript、model response 或 tool payload；
- SQL text、connection string、authorization/cookie header、token、API key 或 private key；
- provider、parser、database 或 infrastructure 的未清理 payload。

sanitizer 是最后防线，不是记录任意 `%error`、`?request` 或完整对象的许可。字段键和值、message、target/source/event、结构深度、集合长度和编码大小都必须受限。前端日志遵守同一数据最小化规则。

Graph 执行通道传递安全的运行与结果身份，不传递用户程序文本。Harness transcript 和 memory 属于持久业务数据，也不得写入运行日志；插件进程的 stdout/stderr 仍按插件通信协议处理。

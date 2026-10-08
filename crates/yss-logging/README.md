# yss-logging

> Status: Current
> Scope: 平台中立日志采集、SQLite 历史、近期快照与订阅
> Canonical owners: src/collection.rs、src/collector/、src/store.rs 与 src/stream/
> Update when: 日志采集、存储、交付、关闭或宿主接口改变时

GPUI 宿主直接使用 `LogCollection` 与 `LogRuntime`。本 crate 不依赖桌面框架，
不注册命令或权限，也不持有 Graph Problems、模型诊断、运行失败或结果事实。
Rust 生产者使用普通 `tracing` 宏；`log` facade 由 collector 的 LogTracer 接入。

## 组装与生命周期

`LogCollection::initialize(directory)` 安装 process-wide subscriber 和受限 console worker，
在指定目录打开 `logs.sqlite`。存储无法初始化时保留 console 输出，并向宿主表示历史不可用。
宿主持有 collection 至窗口循环结束；`shutdown` 排空已接收日志并结束 dispatcher，
Drop 随后停止 console 入队并排空队列。console 关闭等待约 250 ms；sink 拒收或 panic
会终止输出，持续阻塞的 sink 不会无限阻塞宿主关闭。
原生 `YSSBI_APP_DATA_DIR` 可隔离验收数据，具体路径由宿主决定。

`LogRuntime` 提供 typed query、statistics、recent snapshot 和 subscribe/unsubscribe。
订阅使用中立 sink 回调；关闭释放原 worker。Logs 清空只清视图，不删除持久记录。
保留的 `FrontendLogEntryDto` 是日志输入契约，不会加载浏览器或 Tauri runtime。

## 采集与数据最小化

collector 拥有 filter、有界事件捕获、脱敏与 console worker。缺失、空或无效的
`RUST_LOG` 回退到 INFO；有效配置可以覆盖 Rust 过滤器。`println!`、进程 stdout/stderr
不属于 tracing 入口。显式 UI 日志输入在校验前同样执行 INFO 阈值。

生产者只记录发生时的本机钟面时间、捕获有界字段并非阻塞入队。字段键、字段数、
字符串长度和累计字节提前受限，被屏蔽字段不执行值格式化。dispatcher 专用线程完成
时间格式化、脱敏和规范化，console 与 SQLite 共享同一处理结果。输出仍检查结构和编码大小。

调用点优先记录 ID、count、kind、duration、digest、stable code 和 incident identity。
禁止记录原始表格行/单元格、文档、clipboard、prompt、transcript、模型回复、工具 payload、
SQL、连接字符串、认证/密钥及未经清理的基础设施错误。sanitizer 是最后防线，
不是记录完整请求或任意底层错误的许可。插件 stdout/stderr 继续由原协议 owner 管理。

## SQLite 与交付顺序

[store.rs](src/store.rs) 唯一维护 schema：`logs` 包含 sequence、stream_id、timestamp、level、
origin、domain、target、event、message、source 和 JSON fields；`tracing_meta` 保存流身份。
时间戳保留本机钟面且不附带时区。SQLx statement logging 被关闭，避免 INSERT 反馈循环。

同一批记录先提交 SQLite WAL transaction，再更新 recent snapshot 并发送订阅通知。
重启从同一库恢复 identity、sequence 和 recent snapshot；数据库预计由一个应用进程写入，
诊断连接可只读查询。持久历史尚无自动清理；UI/recent buffer 有界，不等于完整历史。
近期查询多读取一条以判断截断，完整数量由 statistics 查询提供。

写入失败通过 `LogBatchDto.failure` 交付 `storage_unavailable`，之后查询和订阅失败，
未提交记录不成为可见持久历史；console 输出继续。慢订阅队列满时交付 `subscriber_lagged`。
终止通知使用普通队列之外的一个保留位置：当前回调返回后丢弃剩余批次，通知一次并释放 sink。
dispatcher 不等待慢消费者。sink 已关闭、拒收或永久阻塞时不能承诺送达。
原生宿主从已提交 recent snapshot 恢复；存储不可用时显示错误。

## 验证

从仓库根目录运行：

```sh
cargo test -p yss-logging --lib
cargo clippy -p yss-logging --lib --no-deps -- -D warnings
cargo check -p yss-desktop-gpui --bin yss-desktop-gpui
```

库测试覆盖提交先于交付、重启恢复、失败终止、脱敏与有界队列。
原生 Logs 的显示、清空和重启恢复使用人工验收。

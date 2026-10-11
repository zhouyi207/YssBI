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
在指定目录打开 `logs.sqlite`，默认保留全部历史。`initialize_with_retention(directory, policy)`
在暴露历史之前应用首轮保留策略；数据库初始化与维护失败返回宿主，不静默忽略。
目录为 `None` 时只保留 console 输出，并向宿主表示历史不可用。
宿主持有 collection 至窗口循环结束；`shutdown` 排空已接收日志并结束 dispatcher，
Drop 随后停止 console 入队并排空队列。console 关闭等待约 250 ms；sink 拒收或 panic
会终止输出，持续阻塞的 sink 不会无限阻塞宿主关闭。
原生 `YSSBI_APP_DATA_DIR` 可隔离验收数据，具体路径由宿主决定。

`LogRuntime` 提供 typed query、statistics、recent snapshot 和 subscribe/unsubscribe。
订阅使用中立 sink 回调；关闭释放原 worker。Logs 清空只清视图，不删除持久记录。
`FrontendLogEntryDto` 是显式 UI 日志输入的数据契约，不引入桌面框架依赖；原生日志呈现与操作由 [GPUI host](../yss-desktop-gpui/README.md) 拥有。

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
origin、domain、target、event、message、source 和 JSON fields；`tracing_meta` 保存流身份、
独立于剩余行数的 sequence 高水位与历史已清理标记。
时间戳保留本机钟面且不附带时区。SQLx statement logging 被关闭，避免 INSERT 反馈循环。

同一批记录先提交 SQLite WAL transaction，再更新 recent snapshot 并发送订阅通知。
重启从同一库恢复 identity、sequence 和 recent snapshot；数据库预计由一个应用进程写入，
诊断连接可只读查询。清理最后一行也不会重置 sequence、stream_id 或查询游标。
近期查询多读取一条以判断截断，完整数量由 statistics 查询提供。

写入失败通过 `LogBatchDto.failure` 交付 `storage_unavailable`，之后查询和订阅失败，
未提交记录不成为可见持久历史；console 输出继续。慢订阅队列满时交付 `subscriber_lagged`。
终止通知使用普通队列之外的一个保留位置：当前回调返回后丢弃剩余批次，通知一次并释放 sink。
dispatcher 不等待慢消费者。sink 已关闭、拒收或永久阻塞时不能承诺送达。
原生宿主从已提交 recent snapshot 恢复；存储不可用时显示错误。

## 可选历史保留策略

宿主显式传入 `LogRetentionPolicy::new(retention_days, max_storage_mib)`；两项均为
`None` 时不删除、不整理数据库。天数范围为 1–3650，存储目标为 1–10240 MiB。
`LogRuntime::open_with_retention` 和 collection 构造器在启动时应用策略；
运行时通过 `LogRuntime::set_retention` 更新。设置、启动都需在阻塞 executor 上执行。
显示级别只影响宿主过滤，不改变 `RUST_LOG` 或显式 UI 输入的 INFO 采集阈值。

天数按记录的本机钟面时间与当前本机时间比较；严格早于截止时间的记录删除，
恰好位于截止时间的记录保留。因历史时间戳不含时区，切换时区或系统时钟会相应影响年龄。
存储目标按 SQLite 已分配且未空闲的页计量，包含表、索引与数据库开销，按 sequence
从旧到新删除。它不是文件系统硬上限：WAL、碎片、最低数据库开销、读事务及待处理维护
均可能使实际文件暂时大于目标。

每轮最多删除 512 条；大小清理每 32 条重估页数，随后最多回收 256 个空闲页并尝试
非等待 WAL 截断。首次启用需要 SQLite 的单次 auto-vacuum 整理，此操作可能重写数据库；
持续 ingest 不执行全库 VACUUM。首轮应用后，dispatcher 每秒维护，有积压时每 100 ms
继续有界清理，空闲期间也会执行年龄清理。活动读事务阻止 WAL 截断时延后回收，不终止读者。

提交删除后才发布带排序 `evicted_sequences` 的批次，宿主移除对应显示行；
活动订阅、stream_id 和 sequence 高水位保持不变，新快照不再包含已删除记录。
维护失败由同步调用返回，并向活动订阅交付 `retention_failed` 终止通知；
之后持久查询失败，console 继续输出。异步维护失败同样由 Logs 显示错误。


## 验证

从仓库根目录运行：

```sh
cargo test -p yss-logging --lib
cargo clippy -p yss-logging --lib --no-deps -- -D warnings
cargo check -p yss-desktop-gpui --bin yss-desktop-gpui
```

库测试覆盖提交先于交付、重启恢复、失败终止、脱敏、有界队列、保留截止边界、
有界清理、SQLite 空间回收、空历史高水位和保留失败。原生 Logs 的显示、清空、
动态显示偏好与重启恢复使用人工验收。

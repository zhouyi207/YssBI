# yss-filesystem

> Status: Current
> Scope: 文件访问、目录身份、文件事务、变化契约与监听会话
> Canonical owners: [Cargo.toml](Cargo.toml) 和 [src/](src) 拥有 API 与依赖事实；项目规则由 [Project](../yss-project/README.md) 拥有
> Update when: FS API、路径安全、事务或监听生命周期改变时

本 crate 不依赖任何 YssBI 内部 crate 或桌面框架。它不知道项目、图、图表、数据库、资源版本或项目目录结构；调用方只传入文件系统路径、字节和可选的内容/路径过滤规则。

| 模块                          | 职责                                                             |
| ----------------------------- | ---------------------------------------------------------------- |
| `root` / `identity`           | 路径规范化、原生目录身份、RootBinding 重验、独立的 TransactionId |
| `coordinator`                 | 按根目录排序的访问 lease、关闭准入、排空和最终 lease             |
| `transaction` / `recovery`    | 暂存、提交、显式/Drop 回滚、恢复标记与故障注入                   |
| `lifecycle`                   | 安全文件清单枚举和目录操作策略                                   |
| `change`                      | 安全 RelativePath、FileChange 与 RescanRequired                  |
| `watcher` / `watcher::notify` | 监听接口、epoch 隔离、关闭和排空；notify 平台实现                |

RootBinding 接收明确的目录路径；它不会把某个文件名当作项目入口，也不会根据文件名自动选择父目录。RootIdentity 表示原生目录对象身份，TransactionId 只标识文件事务。项目操作 ID 与保存到注册库的身份投影由调用方显式转换。

目标根目录已存在时，`bind_existing` 保留首次捕获的原生身份，拒绝等待期间被替换的目录；
首次捕获时尚不存在的目标目录允许在创建后绑定身份。事务目标与复制源共用 `RelativePath`
的普通相对路径校验，拒绝前导当前目录、父目录和绝对路径组件；事务目标还按共享 portable path
规则排除暂存目录及其大小写别名。
`remove_directory_if_created` 接收捕获的 RootBinding 与创建事实，只在目录身份重验通过后清理；
重验失败时保留同路径的替换目录。

`read_file_inventory` 在枚举前绑定源目录，在返回前重验根身份；根目录及其祖先的链接或重解析点
由现有 RootBinding 拒绝。清单只保存相对路径，不加载整棵树的文件正文。事务复制通过暂存文件流式读取。

`FilesystemTransaction::prepare` 默认接受任意字节，不执行 JSON 或业务文档校验。需要校验时，调用方使用 `prepare_with_validator` 或 `prepare_with_file_validator` 提供规则。提交后必须调用 `finalize` 确认，或调用 `rollback` 撤销；未确认的提交沿用 Drop 回滚语义。失败恢复状态只描述文件系统结果，上层决定如何暂停或恢复业务。

`transaction.rs` 定义公开事务契约；内部 `prepare` 与 `commit` 编排准备、提交和回滚，
`paths` 负责路径校验与目录原语，`mutation` 负责文件替换和移动，`journal` 持有修改前状态及恢复逻辑。
`workspace` 同时拥有暂存目录、事务上下文和文件 lease，从准备阶段移交到已提交事务，不另建事务 registry。
调用方在准备后因版本重验失败而放弃候选，或准备过程提前退出时，工作区在释放 lease 前清理暂存文件；
清理失败写入调用方提供的 RecoveryMarker。已提交事务继续按原有 finalize/rollback 协议处理真实文件。
所有清理入口先从租约根目录重验到暂存目录的完整路径，再校验目录树，拒绝沿被替换的父目录重定向清理。

```rust
use yss_filesystem::{
    FilesystemCoordinator, FilesystemError, FilesystemTransaction, RootBinding,
    StagedFilesystemMutation, TransactionContext, TransactionId,
};

fn write_bytes(directory: &std::path::Path) -> Result<(), FilesystemError> {
    let binding = RootBinding::for_existing(directory)?;
    let root = binding.normalized().clone();
    let coordinator = FilesystemCoordinator::default();
    let lease = coordinator.acquire(root.clone())?;
    let transaction = FilesystemTransaction::prepare(
        TransactionContext { root, transaction_id: TransactionId::new(), recovery_marker: None },
        lease,
        vec![StagedFilesystemMutation::Write {
            relative_path: "sample.bin".into(), contents: vec![0, 255, 17],
        }],
    )?;
    transaction.commit()?.finalize();
    Ok(())
}
```

同一协调范围应复用同一个 FilesystemCoordinator，以共享 lease 和准入状态。文件树读取会跳过本库的事务暂存目录 `.yssbi-transaction`；该目录保留原有名称以维持已有暂存区排除行为。

RootLifecycleGuard 在关闭新准入后允许已准入操作排空，再取得最终 lease；这一取得过程只等待现有持有者，
不返回没有实际失败分支的 Result。lease 对外只提供根身份的包含性检查，不暴露内部根列表。

`NotifyFileWatcher::new()` 默认观察所有安全的根内文件变化。`with_filter` 接收调用方定义的相对路径过滤器；根目录变化和后端 rescan 请求仍交付重扫信号。原生事件按有界队列合并为 RescanRequired，满队列不会丢掉“仍需重扫”的事实。WatcherState 负责新旧会话 epoch 隔离及可重试排空，不更新任何业务状态。

`watcher/mod.rs` 定义公开监听契约；`lifecycle` 持有会话切换状态，`admission` 过滤已关闭或不匹配的 epoch，
`drain` 唯一持有待关闭的 source session、可重试排空句柄及真实终态。底层关闭和排空在锁外执行；
并发 finish 按各自的截止时间等待同一 owner，超时返回同一排空过程的重试句柄，不能把“正在排空”当作工作线程失败。
成功与 WorkerPanicked 终态都会保留，底层关闭/排空回调的 unwind 也会发布失败终态并唤醒等待者。
启动候选的守卫在错误或 unwind 时关闭其 epoch 并释放 Starting 状态，旧 sink 不能继续交付变化。
新候选安装前若已被另一个 watch 抢占空位，会重新处理当前会话；不会等待 Active 状态自行消失。
epoch 过滤只决定准入，已经进入的回调由 source session 的排空契约负责完成；不另建无人消费的在途计数。

Project 在自己的模块中解释项目入口、选择索引输入路径、校验文档，并将 FilesystemError 转成 ProjectOperationError；Application 决定项目切换时何时启动或关闭监听。CSV/Parquet 等格式与数据集存储继续由各自的业务 owner 管理。

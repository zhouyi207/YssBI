# yss-filesystem

> Status: Current
> Scope: 文件访问、目录身份、文件事务、变化契约与监听会话
> Canonical owners: [Cargo.toml](Cargo.toml) 和 [src/](src/) 拥有 API 与依赖事实；项目规则由 [Project](../yss-project/README.md) 拥有
> Update when: FS API、路径安全、事务或监听生命周期改变时

本 crate 不依赖任何 YssBI 内部 crate，也不依赖 Tauri。它不知道项目、图、图表、数据库、资源版本或项目目录结构；调用方只传入文件系统路径、字节和可选的内容/路径过滤规则。

| 模块                          | 职责                                                             |
| ----------------------------- | ---------------------------------------------------------------- |
| `root` / `identity`           | 路径规范化、原生目录身份、RootBinding 重验、独立的 TransactionId |
| `coordinator`                 | 按根目录排序的访问 lease、关闭准入、排空和最终 lease             |
| `transaction` / `recovery`    | 暂存、提交、显式/Drop 回滚、恢复标记与故障注入                   |
| `lifecycle`                   | 安全读取文件树、枚举和目录操作策略                               |
| `change`                      | 安全 RelativePath、FileChange 与 RescanRequired                  |
| `watcher` / `watcher::notify` | 监听接口、epoch 隔离、关闭和排空；notify 平台实现                |

RootBinding 接收明确的目录路径；它不会把某个文件名当作项目入口，也不会根据文件名自动选择父目录。RootIdentity 表示原生目录对象身份，TransactionId 只标识文件事务。项目操作 ID 与保存到注册库的身份投影由调用方显式转换。

`FilesystemTransaction::prepare` 默认接受任意字节，不执行 JSON 或业务文档校验。需要校验时，调用方使用 `prepare_with_validator` 或 `prepare_with_file_validator` 提供规则。提交后必须调用 `finalize` 确认，或调用 `rollback` 撤销；未确认的提交沿用 Drop 回滚语义。失败恢复状态只描述文件系统结果，上层决定如何暂停或恢复业务。

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

`NotifyFileWatcher::new()` 默认观察所有安全的根内文件变化。`with_filter` 接收调用方定义的相对路径过滤器；根目录变化和后端 rescan 请求仍交付重扫信号。原生事件按有界队列合并为 RescanRequired，满队列不会丢掉“仍需重扫”的事实。WatcherState 负责新旧会话 epoch 隔离及可重试排空，不更新任何业务状态。

Project 在自己的模块中解释项目入口、选择索引输入路径、校验文档，并将 FilesystemError 转成 ProjectOperationError；Application 决定项目切换时何时启动或关闭监听。CSV/Parquet 等格式与数据集存储继续由各自的业务 owner 管理。

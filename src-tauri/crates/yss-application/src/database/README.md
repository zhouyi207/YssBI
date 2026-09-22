# Database application

> Status: Current
> Scope: 数据导入导出、会话协调与发布结果
> Canonical owners: 本模块编排用例，Database Runtime 与 Dataset Store 拥有数据及存储
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## Database use cases

Database 用例由 Application 组合 Project declaration authority 和 session-scoped Database runtime：

- typed import source 在 transport 边界解析；
- 项目格式 5 使用 SQLite committed catalog 和不可变 Parquet 文件集；旧格式在读取入口拒绝，不自动改写；
- 大表 query/edit/profile/export 保持在 Rust，使用 DataFusion、Arrow 分页、列投影、聚合与批处理；
- 插件结果由独立 DataFusion 适配器查询，Arrow 负责交换文件；
- 宿主 IPC/CSV/Parquet 文件边界使用 Arrow batches；精确存储 Schema 与 Graph 语义 Schema 分开，见 [Database runtime](../../../yss-database-runtime/README.md)；
- mutation 在锁外执行 I/O，并在最终 Project gate 重新验证 session/revision 后提交。

当前数据库窗口只读展示分页行与元数据，并提供导出；前端 `DatabaseService` 仅保留导入、查询、资源管理及 Details 使用的类型转换和语义设置入口。后端数据库编辑、历史和 checkpoint 仍由 Database runtime 的当前契约拥有。

runtime 登记必须关联实际物理准备及其存储恢复记录，schema 变化由准备前后的快照决定。未提交的物理准备通过释放对象清理；已提交或提交结果不确定的变更保留给 SQLite operation record 恢复。Application 只在存储尚未提交时补偿 runtime 登记，会话关闭后由 `resolve_storage_recoveries` 统一核对持久化结果。

数据库导入准备和导出发布分别位于 Application 的 `database/import.rs`、`database/export.rs`，用例入口位于 `database/mod.rs`，会话装配位于 `session/database.rs`。编辑历史是 Database Runtime 的私有实现，供 IPC 共享的 EditState 归 `yss-database-contract`。数据库导出、插件 JSON/文件导出和 Julia worker assets 直接使用 `atomicwrites::replace_atomic`；临时文件、内容同步、会话重验和失败清理由各调用方负责。窗口状态由官方插件独立持久化。

文件替换要求临时文件与目标位于同一文件系统。`replace_atomic` 在 Unix 重命名后同步父目录，返回错误时目标可能已经更新；调用方统一保守地报告发布结果不确定，不自动重试、不回滚或删除目标，只清理临时路径。不能通过临时文件消失推断持久化成功。
数据库导出返回 `database_export_publication_uncertain`，插件 JSON/文件导出返回 `plugin_file_publication_uncertain`，Julia assets 返回 `julia_worker_asset_publication_uncertain` 并停止本次 worker 准备；写入前及内容写入失败继续使用原有失败码。替换失败不提交成功响应或后续内存更新，调用方需检查目标后决定后续操作。该协议不提供多文件事务或跨平台完整断电保证；`replace_atomic` 不同步文件内容，内容同步仍属于调用方。

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

当前工作台数据面板只读展示分页行，底栏提供刷新和导出，对应的 Details 提供元数据、选中内容预览与列设置；不再提供独立数据库窗口。前端 `DatabaseService` 仅保留导入、查询、资源管理及 Details 使用的类型转换和语义设置入口。后端数据库编辑、历史和 checkpoint 仍由 Database runtime 的当前契约拥有。

桌面 IPC 只注册这些实际使用的入口。数据概览由 Harness 的 profile capability 携带查询控制直接读取；行列编辑、撤销重做、checkpoint 和编辑状态通过统一资源工具调用 Application 用例。
Profile 查询保留 Runtime 已分类的取消和期限错误，分别返回 `cancelled` 和 `deadline_elapsed`；其他数据库查询错误继续返回 `database_unavailable`。

Harness 通过统一资源工具调用这些后端用例，覆盖导入、分页读取、行列编辑、物理类型转换、
列语义、撤销重做、checkpoint、重命名和删除。每次修改携带 Project 资源 revision；资源检查中的
元数据、分页和编辑状态沿用本次资源检查捕获的同一 revision。`duplicate_database_for_application` 复用流式 Parquet
导出和导入，保留数据及列元数据，由现有名称分配器生成副本名称并分配新 DatabaseId。
Harness 导出还携带预期资源 revision，创建临时文件前同时核对 Project 与 Runtime 声明版本，
写入目标文件前再次核对原读取基线和 Project 版本；普通 GUI 导出仍使用当前版本。

元数据、分页行、列分布、列取值和编辑状态要求调用方传入预期 Project 资源 revision，共用 `query.rs` 的私有读取入口：
校验原 Application session 和 Project revision，捕获 Database query basis 并确认其声明 observation revision
与请求相等，执行查询及返回值转换，再重验运行时基线、Project revision 与原会话。Project 校验复用已有
按数据库 ID 的授权入口，Runtime 沿用现有 observation，不保存第二份版本表。前端可以让元数据与分页
使用同一个期望版本；后端已前进而事件尚未抵达前端时，旧版本查询仍会被拒绝。复制数据库时，元数据、
导出和导入沿用同一次捕获，不在中途改读当前会话。分页响应接收 Runtime 快照移交的表数据和行 ID，
不再克隆完整分页数据；行数限制和错误映射保持在原用例边界。

列取值初始化按单列读取整列非空去重值，用于前端语义映射草稿；沿用 Runtime 的有界读取，不使用表格当前页或分布查询的截断类别。查询本身不修改语义或数据。

编辑、保存、删除按数据库 ID 从 Project 读取声明；重命名只读取同域的声明快照，均不复制整份项目。
修改协调器从捕获会话与目标声明取得项目和数据库身份，不要求调用方再传一组相同 ID；客户端预期
revision、操作身份和最终 Project/Database 提交检查继续显式保留。

runtime 登记必须关联实际物理准备及其存储恢复记录，schema 变化由准备前后的快照决定。未提交的物理准备通过释放对象清理；已提交或提交结果不确定的变更保留给 SQLite operation record 恢复。Application 只在存储尚未提交时补偿 runtime 登记，会话关闭后由 `resolve_storage_recoveries` 统一核对持久化结果。

导入、复制、删除、保存和示例导入在提交后需要刷新运行时组合时，将本次用例最初捕获的会话传给 session slot。slot 在进入替换的状态锁内核对身份；若其他生命周期操作已安装新会话，迟到刷新返回现有会话刷新错误，不关闭或重建新会话的 Execution/Database。该拒绝不撤销已经提交的数据，持久化结果仍由原提交与恢复协议拥有。

源码按读取、编辑和提交边界组织；`mod.rs` 只保留公开项、通用修改回执和会话捕获/刷新入口。

| 源码                                                       | 职责                                                                                                |
| ---------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| [query.rs](query.rs)                                       | 元数据、分页行、分布、列取值和编辑状态查询；校验预期 Project/Runtime 声明版本并重验原读取基线与会话 |
| [edit.rs](edit.rs)、[edit/operation.rs](edit/operation.rs) | 编辑、重命名、删除、保存用例与操作输入转换；重命名复用一次项目声明读取                              |
| [mutation.rs](mutation.rs)                                 | 跨 Project 与 Database 的准备、提交、补偿及恢复协议                                                 |
| [mutation/project.rs](mutation/project.rs)                 | Project 授权适配、声明 revision 检查和运行时修改请求准备                                            |
| [import.rs](import.rs)                                     | 来源发现、导入、复制和持久化发布交付                                                                |
| [export.rs](export.rs)                                     | 导出入口、临时文件、版本重验、原子替换和失败清理                                                    |
| [error.rs](error.rs)                                       | 应用失败类型及 Project/Database 错误映射                                                            |

用例依赖既有提交协调器和子系统 owner，不保存第二份声明或编辑历史。会话装配位于 `session/database.rs`。编辑历史是 Database Runtime 的私有实现，供 IPC 共享的 EditState 归 `yss-database-contract`。数据库导出、插件 JSON/文件导出和 Julia worker assets 直接使用 `atomicwrites::replace_atomic`；临时文件、内容同步、会话重验和失败清理由各调用方负责。窗口状态由官方插件独立持久化。

文件替换要求临时文件与目标位于同一文件系统。`replace_atomic` 在 Unix 重命名后同步父目录，返回错误时目标可能已经更新；调用方统一保守地报告发布结果不确定，不自动重试、不回滚或删除目标，只清理临时路径。不能通过临时文件消失推断持久化成功。
数据库导出返回 `database_export_publication_uncertain`，插件 JSON/文件导出返回 `plugin_file_publication_uncertain`，Julia assets 返回 `julia_worker_asset_publication_uncertain` 并停止本次 worker 准备；写入前及内容写入失败继续使用原有失败码。替换失败不提交成功响应或后续内存更新，调用方需检查目标后决定后续操作。该协议不提供多文件事务或跨平台完整断电保证；`replace_atomic` 不同步文件内容，内容同步仍属于调用方。

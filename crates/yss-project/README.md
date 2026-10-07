# yss-project

> Status: Current
> Scope: Project runtime authority、资源 revision、持久化与 publication 边界
> Canonical owners: 本 crate 的源码与测试拥有可执行事实；Graph 生命周期由 Graph 架构文档维护
> Update when: Project authority、revision 使用者、事务或 publication 边界改变时

`yss-project` 是项目运行期权威状态的唯一 owner。它负责 `ProjectState`、项目会话与实例身份、资源 revision、持久化事务，以及磁盘提交后的 publication。

项目索引分别发布 `events` 与 `functions`。Event 条目包含文件身份和资源版本；Function
条目还必需包含函数签名、签名版本和编辑投影。调用目录直接消费 Function 索引。
节点图的驻留正文、历史、解析及执行继续使用共享 Graph 基础设施；Graph 不作为项目文件分类。
文件定位接口按 Event、Function、Chart、Mind、Doc 的具体种类校验路径。

该 crate 组合 `yss-project-model`、`yss-project-history`、`yss-project-operation`、`yss-resource-lifecycle` 与 `yss-filesystem` 等更低层 crate，但不依赖 Tauri、Commands、IPC schema、Application 工作流或 Database runtime。

边界约束：

- `ProjectData` 是 resident project facts 的权威聚合；
- 项目默认名称与名称规范化由 `yss-project-model` 统一拥有，创建、注册和空项目模型共用同一规则；
- `yss-filesystem` 只拥有安全文件系统原语，`yss-project` 拥有 session/revision 校验与 publication；
- 对外返回 Project-owned typed facts，由 Application 和 API 层投影为事件与 DTO；
- `test-support` 只暴露跨 crate 测试所需的 fixture 与故障注入 seam。

Graph 索引头读取复用 `GraphResourceIndex` 已扫描的路径，不再次枚举目录。测试夹具使用正式 Graph 序列化入口，按资源路径写入，不根据显示名称重命名或自动生成避重路径。

项目元数据的 `exportTime` 使用不带时区的本机钟面时间。Manifest 构造与读取时遇到
旧的带偏移日期时间会保留其原日期、钟面和小数精度并去掉偏移，不换算到另一个时区。
日历展示不再依赖浏览器的本地时区转换；registry 的 Unix 秒计数保持数值时间点语义。

Graph 当前文档位于 ProjectData，撤销/重做与保存指纹由 GraphEditingMetadata 管理。普通编辑只提交内存数据，显式 Save 才写入图正文；前端不持有独立图草稿或历史。数据库编辑历史由 Database runtime 管理。Project 提交发布资源版本和 delta，不维护项目级撤销栈。`yss-project-history` 保留共享的资源身份、变更请求、函数文档、delta、错误及图驻留状态契约；文件事务回滚与失败恢复继续由 Project 和 filesystem owner 负责。

发布 delta 的资源身份由 `ResourceKey` 表达，文件生命周期种类由 `ResourceLifecycleKind` 表达；
delta 不提供另一套资源分类或逆补丁接口。Graph 撤销仍使用文档 owner 的 `GraphDocumentPatch`。

Graph 保存统一使用图编辑会话的 `save_graph_edit`，按当前编辑身份提交并更新 saved-content identity；Chart 使用自己的文档保存用例。工作台保存全部逐资源调用这些入口，Project 不再提供绕过编辑会话回执的整项目 flush 或独立 graph writer。

Save As 以捕获的当前 ProjectData 覆盖目标副本中的 Graph、Chart、Mind 和 Doc 正文；Mind/Doc 复用各自 `FileContent::encode`，包含未保存编辑。复制不保存或修改源项目，仍在目标 publication 前重验源项目身份及 authority generation。

Graph 复制、重命名及函数签名修改接收调用方当前会话的冻结 `NodeRegistry` 借用。Project 通过注册角色
识别函数引用，覆盖当前文档、磁盘正文和可逆历史，不按内置节点 ID 维护另一份角色表。
Project 不保存节点注册配置，也不依赖 Catalog、Analysis 或 Graph Runtime。重命名保留显式引用，当前适用的
默认引用会写成新路径的显式覆盖；未适用的默认声明不属于当前引用。普通文本和输入字面量不参与重写。
调用方闭包通过 Registry 的 `calls_function` 同时追踪直接 Call 和按组 Apply/Transform，
函数正文或签名变化继续传播到所有调用方；分组调用不维护单独的引用目录。
重命名的数据由 `GraphResourceRenameRequest` 绑定路径、预期版本、新名称、生命周期令牌和操作 ID；
Project 身份与冻结 Registry 作为调用上下文传入，请求不新增 wire 或持久状态。

函数签名事务从 WriterSnapshot 的当前驻留文档捕获消费者，并在 filesystem lease 内读取其调用可达的
未驻留函数正文。首次需要磁盘正文时扫描一次路径索引，后续读取复用它；正文不安装为驻留状态。
注册角色、函数 interface resolver 和函数资源参数确定直接签名消费者；Call、GroupApply 和
GroupTransform 角色形成正文调用边。
回执包含已修改函数、直接签名消费者及沿正文调用边反向可达的调用图；循环和重复调用按路径去重。
仅消费外层函数签名的节点不因外层正文所调用的内层函数签名变化而失效。这些依赖关系仅在本次准备期间存在。

驻留查询通过 `ProjectState::has_resident_graph` 和 `read_resident_graph` 读取存在性或单个资源，避免图编辑和运行准备为此复制整份 `ProjectData`。这些查询保留操作准入检查，不从磁盘加载未驻留图；需要读取已声明资源的用例仍使用 `read_graph_resource_snapshot`，提交与返回前的身份/版本重验仍由对应操作完成。

`read_resource_catalog` 提供资源身份、名称、版本和轻量状态；复用现有路径扫描和数据库声明，不读取正文、不复制 ProjectData、不打开关闭图。文件成员仍由磁盘路径决定，resident 状态只补充其版本、dirty 和初始业务引用。调用方通过原 `validate_project_index_version` 重验 publication 与 authority generation。数据库 dirty 仍归 Database owner；图尚未打开时不虚构编辑会话。

Graph、Chart、Mind、Doc 和 Database 的复制接受可选目标名称，在同一次复制提交中沿用名称分配规则；不通过复制后重命名实现。返回的实际名称和资源身份用于后续操作。

## Filesystem boundary

项目入口路径解释位于 `src/filesystem.rs`，索引变化策略与 ProjectIndexInvalidation 位于 `src/file_changes.rs`，业务失败由 `src/operation_error.rs` 的 ProjectOperationError 表达。FS 仅接收明确的目录、相对路径、字节与校验回调，不依赖项目契约。项目操作 ID 显式转换为 TransactionId；注册库的根身份与 FS RootIdentity 通过不解释内容的字符串投影比较，已有存储值保持不变。

所有项目写入在暂存阶段显式调用 Project 的文档校验器；通用 FS prepare 默认不限制文件格式。项目删除前对 metadata.yssbi 的校验也由 Project 执行。FilesystemError 在 Project 边界映射为既有业务错误类别，前端错误 wire 不变。

## Graph resource revisions

项目格式版本以 [`CURRENT_PROJECT_SCHEMA_VERSION`](src/manifest.rs) 为准。命名常量随 Event Graph/Function Graph 的 `GraphDocument.constants` 保存，属于 Graph 资源事务，不再维护全局变量、变量 revision、作用域迁移或 `variables.yssbi-vars` 的日常读写。

项目尚未发布，只支持当前格式。打开其他格式版本的项目会在 manifest 校验时失败，不执行旧变量资源或节点的兼容转换，也不改写原文件。新建和保存项目继续写入当前 `schemaVersion`。

常量增删改使用当前图编辑、Save 与后端图历史，项目查询不再发布独立变量集合。复制图时为常量分配新身份，并通过 Document Edit 共享规则重写注册的 `GraphConstant` 参数引用。格式、复制、类型和执行语义由 [Graph 与 Execution](../yss-application/src/graph/README.md) 维护。

`graph_resource_revisions` 是 Project-owned `GraphResourcePath → ResourceRevision` 索引，不是 editor projection 的请求计数器。它仍有生产读写方：

| 使用方                                                                                                                           | 作用                                                                                       |
| -------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| [Project activation](src/project_activation.rs) 与 [Graph lifecycle](src/project_state/graph_lifecycle.rs)                       | 安装活动资源的 revision，校验 rename 等操作捕获的资源版本，并在提交后更新 identity/version |
| [Graph document commit](src/project_state/graph_operation.rs)                                                                    | 与 committed document 的更新一起推进资源 revision                                          |
| [Function mutation](src/project_state/function_mutation.rs) 与 [resource publication](src/project_state/resource_publication.rs) | 检查提交身份、预期版本与 patch before-state，按已提交变更推进资源版本                      |
| [Execution authority](src/execution_authority.rs)                                                                                | 将 Graph resource revision 映射为执行资源 version/grant，参与资源 currentness 校验         |

因此当前不能直接删除此索引。移除 Graph Projection Channel 仅移除了那条传输链的 request-generation 协调；Save 不再接受 frontend `expectedRevision`，也没有移除 Rust 事务内部的版本校验。

以下字段不能互相替代：`ResourceRevision` 标识已提交资源版本；frontend lifecycle token 拒绝旧 editor 请求；编辑版本标识当前文档；semantic input hash 标识语义内容与分析输入，供运行校验和计划复用。单独的 revision 也不能替代 Project instance/session identity。若以后合并 revision 的存储位置，必须同时迁移以上使用方，保持提交前重验与事务/执行资源校验语义。

Graph 编辑、Save 和 Execute 的当前流程见 [Graph 与 Execution](../yss-application/src/graph/README.md)。

## Resource publication and external files

数据库声明读取由 [database_authority/read.rs](src/database_authority/read.rs) 拥有。
`read_database_snapshot` 在现有 publication 锁内捕获项目实例、会话、根目录、authority generation、
声明及其 revision/fingerprint observations；只复制数据库声明，不读取其他资源正文或扫描完整文件索引。
快照是带读取依据的不可变结果，不持有另一份可写项目状态。缺失 revision 或冲突的声明身份会被拒绝，
不补造零版本。Application 的会话工厂和数据库列表共用此入口，并通过 `revalidate_database_snapshot`
检查构造或查询期间的 Project 变化；物理数据与 schema 仍归 Database Runtime。
快照暴露捕获的 authority generation；Application 需要把数据库列表关联到既有项目索引时，复用
`validate_project_index_version` 同时校验调用方 publication revision，无须再构造或扫描完整索引。

单库编辑使用 `read_database_declaration(project_instance_id, id)`：在同一 publication 锁内
核对项目身份并按既有 ID 索引复制一份声明，不复制其他数据库或 Graph/Chart/Mind/Doc 正文。
重命名需要检查同项目的名称集合，继续使用数据库声明快照。读取声明不能替代修改准备与提交时的
Project revision 校验；同一个数据库 ID 在另一项目出现时，旧项目身份不能读取它。

数据库删除先准备 Project publication 和数据库资源的下一版本，再移除声明并发布同一回执。
版本耗尽等前置失败保留声明及现有版本，不能留下“删除失败但声明已移除”的部分提交。

Event Graph/Function Graph 创建、复制、删除、重命名复用标准 ProjectDataPatch 的提交与 lifecycle/move delta；资源操作推进
publication revision，而不仅是 authority generation。纯生命周期增删不伪造缺失的图投影，
重命名回执保留真实 move delta、递增发布版本及受影响图的恢复声明。

Graph 与 Chart writers 共用 WriterSnapshot 和 transaction context。取得文件系统 lease 后与暂存完成后，
均检查捕获的 authority generation、受影响资源版本以及源/目标路径存在性；重命名复用相同的 ownership lease
和 patch 发布入口。函数签名的 before-state、函数版本与所属 Graph 版本也在事务内校验，文件落盘成功后才发布，
发布失败则回滚文件。签名最终发布在锁内重验原 WriterSnapshot 的 authority generation，确保准备的依赖集合仍有效；
全部可失败检查完成后直接更新目标函数及其 revision，不另复制整份 ProjectData 或 revision 表。
Graph revision 不随驻留状态重置：卸载/重新加载保留版本，删除与移动后的旧路径保留 tombstone。

Watcher 的 rescan 在 filesystem lease 下读取文件，重验项目身份后同步驻留 graph/chart，
预先校验全部版本推进，再发布变更。无内容变化的重复 rescan 不再次推进版本。
未打开的 graph 保持按需加载；未保存图正文就是 Rust 当前驻留数据，watcher 不得用旧文件替换它。
ProjectIndex 的文件成员来自磁盘扫描，内存只提供适用的权威版本，不得复活已删除的 chart。

Chart 文档和文件不包含资源 revision；`chart_revisions` 是其唯一版本 authority，在项目激活时从初始版本开始，
与 Graph 的资源版本生命周期一致。Chart 的创建、保存、复制、移动和删除复用资源 patch 的标准回执，
writer 不再重复构造 delta。刷新文档时可绑定 Project publication revision，在同一次一致读取中验证。
Chart 重命名的目标版本高于源版本及目标路径保留的删除版本，原路径保留递增后的 tombstone；回执与状态安装复用同一计算，旧目标路径的保存基线不能重新匹配新资源。
图表文件采用 `yss-chart-document::CURRENT_CHART_SCHEMA_VERSION` 定义的严格格式，不读取含旧文档版本字段的格式，也不自动迁移。
图表类型由同一 crate 的 `ChartType` 枚举拥有，只允许 histogram、scatter、line；文档、索引和资源变更状态
直接持有该类型，未知类型在反序列化入口拒绝，不能作为任意字符串进入已提交状态。
Chart writer 支持调用者提供读取时的预期 revision；Harness 设置编辑使用它，GUI 的独立 Save
继续按 Rust 当前基线覆盖。Graph/Chart 重命名接受内部生命周期请求，由既有 lifecycle owner
分配令牌，不占用客户端令牌水位，也不由 Harness 维护另一套计数器。

`yss-resource-lifecycle` 独占资源生命周期登记、令牌准入和守卫回退链；Project 负责项目身份、
文件事务与发布。项目激活移出旧登记后，其守卫可能仍在释放中，因此登记 ID 在同一个 registry
的整个生命周期内不重用；新项目只重置资源 owner 与令牌水位，旧守卫不能影响新登记。
`yss-project-operation` 单独拥有操作准入及重放账本。Graph 已原子保存有界命令回执后，
通过 `complete_in_owner` 释放准入，避免再维护一份无界的已完成 Graph 命令索引。

## 当前文档编辑

Mind 与 Doc 的当前正文分别驻留在 `ProjectData.minds`、`ProjectData.docs`，项目索引也分别提供
`minds`、`docs`。格式和树约束由 [Project model](../yss-project-model/README.md) 拥有。
`minds.rs`、`docs.rs` 各自提供强类型查询和命令入口；`file_resources.rs` 通过泛型复用
Create/Edit/Save/Discard/Rename/Duplicate/Delete 的文件 lease、WriterSnapshot、
ProjectDataPatch、文件事务回滚和 resource publication，不使用混合文档枚举分派编辑。
普通编辑只更新 Rust 当前正文，Save 写入捕获版本并更新保存指纹；重命名只移动
已有文件并保留当前未保存内容。复制 Mind 会重新分配内部节点 ID。

读取和编辑回执携带项目身份、文档路径、编辑会话与资源 revision。新建资源与重开项目
创建新的文档会话，旧提交不能授权同路径的新资源。Watcher 更新干净文档，保留脏文档；
Save 还会比较磁盘正文与保存指纹，拒绝覆盖未处理的外部修改。Discard 按捕获的编辑版本
重新读取磁盘；文件已被外部删除时，Discard 放弃其脏驻留内容并发布移除回执，不重新创建文件。
文件成员仍由目录扫描决定；文档按项目加载并受单文件大小和节点数限制。
知识来源读取使用 `read_doc_source`：只读取一份文档及其保存文件，核对磁盘保存指纹与捕获的编辑版本，不复制整份 ProjectData 或扫描项目索引。正常未保存编辑可以成为显式来源；外部删除或尚未处理的外部修改会拒绝引用，编辑器的原始 `read_doc` 仍可用于恢复脏正文。

两类资源事件分别使用 `ResourceKey::Mind`、`ResourceKey::Doc`，生命周期种类与各自身份一致。
生命周期 patch 的 before/after 同时存在时表示文档版本更新；正文由对应读快照交付。
UI 与其他 Application 调用者使用同一个 typed command API，新增类型无须另建文件系统层。

图文件通过 `GraphResourceFile` 直接序列化、反序列化当前类型契约。项目尚未发布，不提供旧类型
声明迁移或兼容转换，也不维护单独的类型迁移版本字段。

`read_graph_editing` 返回文档只读快照和编辑身份。`capture_graph_edit` 检查编辑会话与资源修订，`commit_graph_edit` 在同一 publication 边界安装候选文档、revision 及可逆历史。内容指纹用于 dirty 判断；历史没有完整文档或解析投影副本。保存、重命名与图卸载在各自事务中同步维护这些元数据。详情见 [Graph 与 Execution](../yss-application/src/graph/README.md)。

图命令回执的 `GraphEditCorrelation.result_facts` 可保留有界的调用方结果事实，与文档和实际提交版本原子记录。Application 拥有其中的 Harness 差分编码，Project 仅执行序列化预算和回执生命周期管理，不解释节点语义。查询原命令返回原始事实，不用当前图重建历史结果。单次 correlation 序列化上限为 1 MiB，以容纳批量编辑的完整差分；每张图全部回执仍受 2 MiB 和 128 条上限约束，超出总预算时淘汰旧回执，单次超限在提交前拒绝。

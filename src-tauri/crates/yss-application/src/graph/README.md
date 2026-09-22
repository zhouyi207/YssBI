# Graph application

> Status: Current
> Scope: 图用例编排、当前文档、投影同步、保存及模块调用关系
> Canonical owners: Application graph 用例与 Project 图编辑状态；解析、执行和呈现契约分别由对应模块维护
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## Authority and identities

| 事实                                    | Authority                    | Frontend representation  |
| --------------------------------------- | ---------------------------- | ------------------------ |
| 当前可编辑 GraphDocument、资源 revision | Rust ProjectData             | 只读文档投影             |
| 撤销、重做、已保存内容指纹              | Rust GraphEditingMetadata    | dirty、canUndo、canRedo  |
| 已保存正文                              | Project 文件事务             | 保存成功回执             |
| 解析后的端口、类型、Schema、诊断、依赖  | Rust GraphSemanticSnapshot   | 编辑器投影               |
| 执行计划、run、结果                     | Rust Execution / ResultStore | 查询、运行事件与 UI 投影 |

图编辑直接更新 Project 的当前驻留文档，不维护独立 Draft 文档或前端历史。临时变更候选用于验证及原子提交，不构成另一套可写状态。保存状态比较当前内容与已保存内容的指纹，不保留完整 savedDocument 副本。

GraphEditVersion 包含后端编辑会话 ID 与 Project resource revision，编辑 revision 跨 JS 边界使用十进制字符串。前端 sessionId、projectionGeneration 只控制视图和请求生命周期。Project instance/session、图路径、语义输入 hash、run/result、panel/group 身份继续分别校验。

资源 revision 同时服务文件事务和执行资源校验，语义内容由独立的 semanticInputHash 表达，见 [Project authority](../../../yss-project/README.md#graph-resource-revisions)。

## Module ownership

| Owner                                                                     | 职责                                                                             |
| ------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| yss-node-protocol / yss-node-registry / yss-node-catalog                  | 节点能力与接口声明、注册校验与指纹、内置定义及节点目录本地化                     |
| yss-graph-document                                                        | 节点实例、连线、位置、文档意图、稳定地址和语义文档 fingerprint                   |
| yss-graph-document-edit / editor                                          | structural validation、typed mutation、连接预检、端口顺序、clipboard、编辑器投影 |
| yss-graph-analysis                                                        | concrete interface、type/schema/lineage、canonical diagnostics、Ready proof      |
| yss-graph-resource-contract                                               | immutable resource facts 与一次 Resolve 的 dependency observations               |
| yss-graph-runtime                                                         | 唯一 resolve_graph_draft facade、claim 编排、编辑解析缓存                        |
| yss-graph-diagnostics                                                     | 图诊断代码、模板与定义校验                                                       |
| yss-project                                                               | committed authority、资源版本、文件事务与 publication                            |
| yss-application                                                           | 一致事实 capture/revalidation、Graph↔Project↔Execution 编排                      |
| yss-graph-execution                                                       | 计划构建与缓存、demand/DAG、内核调用适配、ResultStore、运行事件                  |
| yss-node-kernel                                                           | 中立内核调用契约、运行值、冻结 KernelRegistry 与内置执行适配                     |
| yss-application::ipc / yss-ipc-event / yss-ipc-channel / yss-ipc-contract | 命令适配、事件发送、通道交付及共享 wire 协议                                     |

完整清单见 [Module Map](../../../../../docs/reference/MODULE_MAP.md)。

Node 描述一种节点的端口、参数、类型约束及执行语义，不拥有某张图的节点实例或解析结果。
节点编辑投影的 `capabilities` 仅携带 `managed`；复制、创建副本、删除和剪切在前端统一要求存在明确的非受管理节点投影。Rust 继续按节点协议拒绝受管理节点的非法修改与子图导出。参数和内联字面量编辑直接消费各自投影，不另传节点级汇总开关。
三个 `yss-node-*` crate 均不依赖 Graph；`DocumentNode`、位置与连线属于图文档，连接后的类型、
Schema、血缘与诊断仍由 `GraphSemanticSnapshot` 统一管理。`yss-graph-type-mapping` 留在 Graph。
节点目录接收调用方提供的资源创建描述，不读取项目状态，也不解析图中连接。

`yss-graph-editor::projection` 拥有编辑器投影模型与纯映射，消费 Graph Analysis 的语义事实，
生成节点、端口、Schema、诊断与解析结果。Application 捕获输入、调用该能力并重验会话与资源身份；
IPC 只将模型转换为 wire DTO。投影不依赖 Application，也不构成第二份语义 authority。

`yss-graph-runtime` 负责编辑解析；`yss-graph-execution::graph_preparation` 消费只读的 `GraphAnalysis`，
直接构建已有 `ExecutionPlan`、`PlanParameterBundle` 和输出契约。计划缓存归执行会话，资源授权依据
在每次准备时重新绑定。Application 组织资源和会话校验，不再持有一套图包到执行包的转换。
调度器消费执行计划；`kernel_invocation` 将已求值输入、输入模板名、已解析参数及输出类型/字段投影为中立内核调用，kernel 不消费 Graph 计划。运行准备不读取 UI 投影或 Project 可变状态。

Application 的图用例集中在 `graph/`，包括编辑、解析、资源、运行与结果查询；`session/` 独立拥有整个
Project/Database/Graph/Execution 会话的组装和替换。`chart/` 按资源操作、数据库查询及纯图表投影组织，
Chart 预览和图结果复用前端渲染器，各自保留数据持有与失效规则。

执行能力由 `yss-node-kernel` 的冻结 `KernelRegistry` 拥有。`NodeComponents` 在组装时检查定义与已安装实现的参数字段及
输出数量，随后冻结 kernel 表；未安装的实现由编辑解析产生 `graph.node.kernel_unavailable` 阻断。
编辑阶段的支持检查与实际调度读取同一会话的注册表。注册的标识、显式实现 revision 和调用契约共同
形成能力指纹；实现行为改变时必须更新 revision，配置改变通过新的会话配置生效。

Application 的 GUI 创建目录和兼容节点目录保留完整定义，按会话 KernelRegistry 标注 `available`，未接入的节点置灰且禁止目录创建；AI 搜索只返回可用节点。结构节点和透明节点无需叶内核。完整定义注册表仍用于已有图的解析及缺少实现诊断。目录可创建不代表具体图已满足端口、资源或函数依赖要求，最终运行准入仍由语义解析决定。

能力指纹进入 Graph 的语义输入哈希和 Execution 的 `PlanBasis`。Execution 在计划准备、资源准备和执行入口
检查该指纹；同一图文档不会复用能力版本不同的计划。节点输出缓存
的输入指纹也包含该能力指纹。普通数值扩展的[注册与执行样例](../session/components/tests.rs)
复用现有 Node 类型及调度语义；注册 API 不承担新的语言语义或动态插件加载。

## 当前图文档与投影生命周期

```text
GUI / Harness typed command
  → 捕获当前 Project / Graph 编辑版本
  → Application 按图串行编排
  → Graph Document Editor 准备可逆补丁与完整语义投影
  → Project 原子提交当前文档、revision、历史和请求回执
  → Execution 更新结果有效性，发布图变更通知
  → 命令返回 snapshot/delta，React 安装只读投影
```

普通编辑、undo/redo 与 Save 使用同一后端图身份。GUI 队列保留请求与显示顺序，最终版本校验和写入顺序由 Rust 决定。按图协调只持有操作预约；Resolve、文件 I/O 和交付期间不持有 Project 数据锁，独立图的编辑不共享一个长临界区。

GUI 的 saving 标志在保存入队时关闭新编辑与历史请求的准入；此前已接受的任务继续执行和安装结果，保存到达队首后读取最新版本。saving 不用于取消已接受的任务，项目与视图身份校验仍独立生效。

前端通过 `installGraphSession` 将会话、画布实体与结果有效性摘要一次安装到统一的图只读 Store，先用 `canAcceptGraphSession` 拒绝同一后端编辑会话的旧 revision。项目快照在最终同步提交前复用该接纳规则，并重新检查 dirty/saving；同一批图快照同样一次发布。未变化的节点、端口、连线和结果条目保留引用，完整快照也遵守此规则。

GUI 与 Rust 的每图队列均有容量限制，过载返回 `graph_edit_busy`。每个成功接受的编辑、历史或保存命令推进 revision，包括文档未变化的命令；dirty 仍由内容指纹判断。Project 在同一次提交内保存 operation ID、请求指纹、原始编辑版本及实际回执，最近回执同时限制条目数和序列化预算。重复的相同请求复用原提交，不再修改文档或历史；复用 ID 改写请求会被拒绝。回执淘汰后，旧请求的版本不能再次授权写入。

`get_graph_edit_receipt` 按项目、图、编辑会话、请求版本与 operation ID 查询，等待该图在途提交完成。空结果表示没有保留的回执，不证明操作从未发生。GUI 在传输或响应解析失败时先查询原提交，再读取最新投影；旧保存回执只证明原修订已保存，不能清除后续编辑的 dirty。回执随驻留编辑会话结束而释放，不提供跨进程提交恢复。

GraphDocumentPatch 的 before/after 操作提供可逆历史，一个普通操作或 Harness 批次对应一个事务。历史按条目数和序列化字节数限制，阈值由 [graph_editing.rs](../../../yss-project/src/project_state/graph_editing.rs) 拥有。Undo/redo 恢复文档意图后重新 Resolve，不能恢复历史中的类型、诊断或结果 payload。结构有效但暂时不能运行的图仍可编辑，阻断诊断由 Graph Problems 展示。

普通编辑、撤销、运行、兼容节点查询或复制导出不上传完整图文档，而是携带图身份和后端 version。导入或粘贴等本身包含新内容的操作仍交付真实输入。过期版本被拒绝，不自动合并并行修改。

编辑器读取与写入响应使用同一投影同步协议。Rust 按 window/project/graph/locale 缓存有界基线；首次读取、基线失效、后端编辑会话变化或增量不划算时发送 snapshot。小变动使用 set/remove 路径批次，客户端在候选上应用并验证完整结构后发布。未变化的节点、端口及连线复用引用。该协议只交付读投影，不代替 Graph typed 编辑操作。

`GraphEditorSessionDto.resultState` 与文档、语义投影来自同一次图操作。ResultStore 在更新输入依据的同一锁内捕获结果摘要，携带执行会话内的结果 revision；运行发布、运行准入和依赖重验推进该顺序，独立于图编辑 revision。前端要求摘要与投影的 semanticInputHash 一致，保留已安装的更高结果 revision，防止迟到的编辑/读取回执回退运行结果。

两端基线均限制为 32 条、32 MiB 序列化预算，单条超过 16 MiB 时只读而不缓存。交付中的 `snapshotBytes` 由 Rust 在编码时计算，前端据此计量，避免主线程为了缓存预算重新编码完整图。增量最多 512 个操作；字节预算与条目限制不代替真实桌面的安装耗时、长任务及帧率验收。

公共运行路径在提交执行状态后统一发布图活动，GUI、Harness 与内部调用遵循同一规则；调用者的专属 sink 不负责公共发布。GUI 仍保留 RunEvent Channel 和终态排空。
`run_graph_with_sink` 成功时返回 `RunGraphReceipt`，包含运行身份和本次 handoff 实际发布的结果引用、输出与类别。回执在交付结果查看意图和终态之前捕获，不通过随后可能已变化的当前结果索引重建，也不复制结果 payload。Harness 对这些引用执行响应条目预算并明确报告总数和完整性；只需要 RunId 的内部调用继续使用 `run_graph`。
Application 保存运行事实的恢复投影：每个活动运行的开始身份与输出，以及每张图最新运行的终态和失败详情；不保存操作事件历史。`get_execution_snapshot` 按项目会话返回这些当前投影，初始订阅和 Resync 不再依赖前端曾收到的 RunId。按 RunId 的状态查询仍可使用。
通知与命令回执在既有 Graph FIFO 中协调。前端恢复期间有界暂存后续事件，先安装快照再处理通知；公共订阅、专属 Channel 和恢复共享按执行会话、RunId 与终态去重的安装规则。结果打开意图只由公共观察路径处理，不在恢复时重放。
执行提交中、后端运行状态与同步中断分别处理。只有 RunErrored 或权威恢复中的失败事实才写入 Output；Command 拒绝不等于计算失败。Channel 失败先查询当前执行投影，无法确认时保留“运行状态未知”，不自动重跑。成功结果始终先提交再通知。
运行事件由统一安装器直接写入运行投影。Pin 查看查询当前输出结果，不另行提交运行；执行需求统一使用默认需求或显式输出集合。前端恢复统一查询当前执行快照，后端按 RunId 的查询能力继续保留。

关闭视图、HMR、窗口销毁和 Project replacement 释放订阅或使旧回调失效。最后一个编辑器关闭并完成既有保存/放弃流程后可卸载驻留数据，再次打开生成新的编辑身份。Harness 可直接打开已关闭的图，内部读取使用后端分配的 lifecycle token。

关闭最后一个 Graph 标签页前先经过该图 FIFO 屏障，再读取 dirty 和编辑版本决定保存/丢弃。普通缓存释放也进入同一队列，Rust 在提交边界拒绝释放 dirty 文档；明确丢弃携带用户确认时的 GraphEditVersion，版本变化则拒绝丢弃。卸载返回是否允许释放（已经不驻留也视为成功），只有后端确认且 lifecycle 仍有效时才清除前端文档、投影和资源状态；保留或失败时恢复缓存可用标记。清理后的迟到回执不能覆盖重新打开的会话。

Project watcher 保留未保存的当前文档。资源重命名和函数签名事务只持久化其负责的变更，不顺带保存图正文；重命名同步更新历史中的资源引用及保存指纹。节点目录和资源索引仍由现有 Project publication owner 安装。

Graph 重命名在文件系统 lease 内枚举持久化图及驻留图，对当前文档、磁盘正文和可逆历史分别重写引用，再通过同一文件事务与版本校验提交；未加载的调用图保持未加载。Project 的 `graph_references` 共用实现只改写函数 call 的 target、entry/return 的 function 参数及动态端口 FunctionParameter 来源，保留普通文本、字面量和端口实例身份。复制复用此引用规则，但仍单独重新分配节点、连接与动态端口实例身份。

从 Pin 查询兼容节点时读取匹配版本的当前文档，使用同一次 Resolve 的端口类型、Schema 和约束。查询不 claim 端口，真正连接时才原子登记。Canvas、Details、Problems 和运行准入共享同一语义投影，没有面板自有的图事实源。

兼容目录仅保留具有可连接反向端口的节点。提交创建并连接、直接连接或迁移连线时，Graph Runtime 将当前语义快照交给 Editor，再次按相同规则校验；不能通过直接提交目录描述绕过类型检查。连接不兼容时拒绝整个补丁，不残留新节点或端口绑定。

## 编辑、保存与运行

| 操作                   | 修改当前 Project 文档            | 写入文件     | 结果                       |
| ---------------------- | -------------------------------- | ------------ | -------------------------- |
| GUI Edit / Undo / Redo | 是                               | 否           | 新编辑版本、历史能力及投影 |
| Assistant Edit         | 是                               | 自动保存     | 可撤销批次及持久化提交回执 |
| Resolve                | 否                               | 否           | 当前版本的语义投影         |
| Save                   | 保持捕获正文，更新保存指纹       | 是           | 文件事务回执与编辑状态     |
| Execute                | 只执行明确的 finalization effect | 不隐式保存图 | run、Results、Output       |

### 编辑解析与运行准备

编辑阶段解析类型、Schema、血缘、输入转换及执行能力，交付完整诊断和结果有效性依据。未绑定输入、orphan、cycle、函数语义错误和缺少 kernel 均在编辑投影中阻断运行；不为普通诊断产生技术故障弹窗。

运行准备消费当前不可变的 `GraphAnalysis`，由 `ExecutionRuntimeState::prepare_graph_package` 生成或复用完整计划。它要求当前语义快照满足 Ready 条件，直接构造操作、参数、输入绑定、输出契约和查看意图；选中 output 的 demand 由后续执行调度处理。命中计划缓存前也必须检查当前解析结果的可运行性。计划准备没有独立的用户操作或前端状态。

执行能力检查新增阻断诊断时，同步将完整解析的 snapshot 标为 Incomplete，投影为 `analysisBlocked`；`success` 不得同时携带阻断诊断。已有 InternalFailure 保留原故障信息。

`semanticInputHash` 来自语义文档内容、registry 与实际读取的 dependency manifest，排除 node position/user label、无引用 derived metadata 和相关展示字段。manifest 记录所用函数签名/正文、变量类型、数据库 Schema 以及 absent lookup；basis 同时传递 resource versions/observations。无关 catalog 变化不改变 artifact identity。函数正文读取由 Project owner 完成，Graph resolver 不读文件。

保存状态与计划准备分开。内部计划缓存按图路径与语义输入匹配；移动节点、修改显示标签可以复用计划。请求通过编辑版本和语义身份判定 currentness，前端只消费相应投影，不保存独立的计划身份或准备状态。

### Save

Save 接收当前编辑 version 与 operation ID，由 Rust 读取匹配的当前文档并通过文件事务持久化。它不接收前端 authored document，也不把版本不匹配的请求自动覆盖到新内容上。投影在事务前准备，成功后通过同一回执交付；视图增量恢复只执行读取，不能重放写操作。

成功后更新保存指纹并清理图历史，失败保留当前内存编辑与历史。前端 saving 只用于交互反馈，后端按图协调及版本校验保护真正的提交。保存后的视图恢复读取最新状态，不能把较新的内存编辑误标为已保存。

Assistant 的 `apply_graph_edit` 默认通过同一文件事务保存整个当前图文档，包含调用前已有的手动编辑。
Project 在一次提交内安装编辑、保存指纹、历史及幂等回执；写入失败或版本失效时回滚文件，保留调用前的内存文档和历史。
自动保存保留批次的撤销记录；手动撤销使图重新变为未保存，重做回到已保存内容后清除 dirty。
助手编辑不要求打开图面板，也不通过 Webview 确认提交。

图表的保存与配置草稿契约由 [Chart application](../chart/README.md#保存与版本) 维护。

## Cross-boundary routing

| 信息                            | 去向                                   |
| ------------------------------- | -------------------------------------- |
| 普通 Graph validation / Blocked | Graph Projection / Problems            |
| 计算结果                        | ResultStore + typed query              |
| 图运行失败                      | RunErrored + Output panel 失败摘要     |
| 内部技术故障                    | sanitized tracing / incident           |
| command rejection               | yss-application::ipc stable error wire |
| 用户反馈                        | React localization / UI                |

详见 [Runtime Signals](../../../../../src/features/application/observability/README.md)、[API contract](../ipc/README.md)、[Workbench](../../../../../src/modules/workbench/README.md) 与 [Rust workspace 验证](../../../../README.md)。

## 相关模块

[语义解析](../../../yss-graph-analysis/README.md) · [解析缓存](../../../yss-graph-runtime/README.md) · [执行与 ResultStore](../../../yss-graph-execution/README.md) · [数据契约](../../../yss-data-contract/README.md) · [Node Kernel](../../../yss-node-kernel/README.md) · [Results 查询](../../../../../src/features/application/results/README.md) · [画布](../../../../../src/modules/graph-editor/README.md) · [Problems](../../../../../src/modules/problems/README.md) · [Output](../../../../../src/modules/output/README.md)

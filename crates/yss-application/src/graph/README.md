# Graph application

> Status: Current
> Scope: 图用例编排、当前文档、投影同步、保存及模块调用关系
> Canonical owners: Application graph 用例与 Project 图编辑状态；解析、执行和呈现契约分别由对应模块维护
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## Authority and identities

`node_creation_form` 是项目会话内的只读查询，调用 Graph Runtime 后重验项目身份，不要求图已载入。
创建请求同时携带 `parameters` 与 `portCounts`：前者保存显式参数，后者指定用户可变端口的初始总数。
Graph Editor 在同一候选补丁中生成参数、完整成员组和可选连接，Project 仍只提交一次历史变更。
GUI 与 Harness 消费同一 Protocol、参数合并与数量校验；命令层不自行推导可创建端口。

执行 demand 支持全图、显式输出和单节点；单节点分别要求当前输入结果或允许补算依赖。
局部需求由 Execution 在计划准备前选择范围，Analysis 按范围判断就绪，无关未完成节点不阻断执行。
View 可观察已有有效结果，Application 不再要求观察值必须由本次运行生成；ResultStore 在原子发布时验证它仍然有效且来源未变。

GroupBy、Apply、Transform 复用相同的局部运行与结果身份。分组值的有界 JSON 检查投影只展示
`groupedDataFrame` 种类、分组键和源列名，不触发组函数或把分组值伪装成普通表。Apply/Transform
返回可分页 DataFrame，实际列通过原 Schema 观测入口更新 Decompose；函数正文变化使调用结果过时。

`inputs/schema_observations` 只读取稳定关系的字段元数据，将当前执行会话的结果 ID 交给 Analysis。
`GraphResolutionContext` 用候选解析产生的输入依据向 ResultStore 核对生成参数、资源和源结果身份，
逐次去掉不匹配观测后再交付；中间候选不写入结果状态。撤销可以复用仍被持有且依据匹配的列，
新会话没有结果时只按声明解析。Deferred 输出运行成功后发布既有 Graph Changed 通知，刷新列选择器和
Decompose 端口，不改写图文档或编辑 revision；已引用但消失的列继续走原有 orphan 修复流程。
前端图活动队列只跳过较旧的编辑通知；相同编辑 revision 仍可包含新的运行 Schema，继续读取投影。

`run/stages` 编排同一 RunId 的连续执行。Execution 选择最早可执行的动态结构边界，Application
逐阶段重验原资源授权、项目与图输入，发布稳定边界后通过同一解析入口继续准备下游。
初次请求捕获的文档和资源范围保持不变；不能借分阶段执行扩大 Harness 授权或采用后来编辑的文档。
各阶段共用取消对象和 deadline。后续缺列等错误保留已完成边界的结果，并为整次运行交付失败终态。
结束时无论成功或失败，只要运行影响动态结构，都会请求图投影刷新；不写文档或保存展示列。
`RunStarted.outputs` 可以随阶段扩展为同一运行的累计输出集合；恢复快照保留最新集合，
前端只接纳新增输出，不重新失效同一运行已公告的输出。终态和运行回执仍对应同一请求身份。

| 事实                                    | Authority                    | Frontend representation  |
| --------------------------------------- | ---------------------------- | ------------------------ |
| 当前可编辑 GraphDocument、资源 revision | Rust ProjectData             | 只读文档投影             |
| 撤销、重做、已保存内容指纹              | Rust GraphEditingMetadata    | dirty、canUndo、canRedo  |
| 已保存正文                              | Project 文件事务             | 保存成功回执             |
| 解析后的端口、类型、Schema、诊断、依赖  | Rust GraphSemanticSnapshot   | 编辑器投影               |
| 执行计划、run、结果                     | Rust Execution / ResultStore | 查询、运行事件与 UI 投影 |

图编辑直接更新 Project 的当前驻留文档，不维护独立 Draft 文档或前端历史。临时变更候选用于验证及原子提交，不构成另一套可写状态。保存状态比较当前内容与已保存内容的指纹，不保留完整 savedDocument 副本。

资源复制接受可选名称，由原 Project 事务一次分配名称、路径及新元素身份；GUI 未指定时保留默认命名。Harness 轻量目录和元信息查询不打开图，实际编辑与执行才建立所需编辑/语义依据。

GraphEditVersion 包含后端编辑会话 ID 与 Project resource revision，编辑 revision 跨 JS 边界使用十进制字符串。前端 sessionId、projectionGeneration 只控制视图和请求生命周期。Project instance/session、图路径、语义输入 hash、run/result、panel/group 身份继续分别校验。

资源 revision 同时服务文件事务和执行资源校验，语义内容由独立的 semanticInputHash 表达，见 [Project authority](../../../yss-project/README.md#graph-resource-revisions)。

## Module ownership

Harness 结果适配集中在 `automation/results.rs`，复用 Results 的不可变读取、结构投影与查询控制。
图结果列表通过 Project 资源目录核对图是否存在，不以驻留状态判断存在性；已存在但未载入的图可以返回空结果，无须打开编辑器、载入正文或执行图。
公开概览只交付标量或结构、分页 schema 和完整表引用；关系选列先调用原 `project` 再读页。
统计报告与嵌套数组在原投影生成位置接受引用映射，桌面保留其原协议，模型使用可原样传回的业务引用。
两者使用同一结果身份、内容、行范围与清理判据；不扫描 JSON 键名替换用户字段。

Harness 图执行由 Application 依据实际执行范围的语义依赖继承项目共享读取权限，显式资源版本仍由执行准备入口核验，写权限不继承。范围拒绝和版本冲突保留资源、所需操作及预期/实际版本，避免仅返回无定位的 `agent_scope_denied`。图检查同时提供完整编辑版本；只核对版本的 Harness 读取不包含图正文。

Harness 公开图概览提供计数、就绪、已有运行状态及预算内的整体结构。`automation/graph/inspection/overview` 从同一次文档与解析投影生成精简节点参数、字面量和连接，超过 32 KiB 时依次省略配置或全部结构，并明确返回所处层级。预算复用 `result_encoding` 的编码字节计数，不另建缓存或图状态。节点查找、定向详情和连接查询各有独立入口，从同一次捕获投影筛选后分页。节点的 Pin 与 Pin 的已知 schema 可分别续读，未知 schema 不伪装成空列。查询观察标识绑定图版本、语义身份和筛选/分页条件。内部完整检查仍服务编辑及函数用例，内部图编辑回执继续保留完整的已变实体；Contract 的模型投影省略重复的编辑器元数据、参数选项和列 schema，需要这些信息时通过定向检查读取。查询不执行节点、不读取运行值，也不产生第二份图状态。Harness 的内部调用上下文可携带已观察的语义输入依据，Application 在同一次解析投影上核对后再处理图操作；幂等回执查询仍返回原提交结果。模型侧字段和分页约定见 [Harness 查询契约](../../../yss-harness-core/README.md)。

`validate_graph` 从同次打开回执的 Analysis 校验全图或指定节点的上游闭包；范围由 Execution 的
`GraphExecutionScope` 选择，诊断归属和 readiness 共用 Analysis 规则。诊断分页独立于完整就绪判断，
无关分支不阻断局部范围；该检查不生成运行值，也不保证当前缓存输入存在。公开执行输入使用 graph 与
可选 nodeId/mode，转换为原全图或节点 demand；没有 nodeId 时不得单独传 mode。

执行回执、图概览和按运行筛选的结果列表读取 Execution 原运行记录的阶段耗时；活动阶段随查询增长，
终态固定。准入前失败或元数据已淘汰时返回未知，不能拿工具调用时间补作计算耗时。

Harness 节点、连接和常量工具以明确业务输入进入同一 `GraphDocumentEditor`，完整候选成功后才保存并发布。
Graph Editor 拥有标签、Pin 数量调整及按连接 ID 批量重接；数量更新复用创建协议和成员组规则，缩减保留
顺序靠前的实例，端口删除连同相关连接与字面量一起进入补丁。重接保留原连接 ID，先释放本批连接后检查
新端点，允许交换已占用端点，拒绝本批提案之间的容量竞争。
子图导出保留内部源身份映射，实例化交付实际分配的节点与端口映射；GUI 和 Harness 复制复用同一 owner。
Application 将这些映射与常量创建映射一起写入 Project 原提交回执，中断恢复不从当前图猜测创建结果。
常量业务读写位于 `automation/graph/constants.rs`，类型和值直接消费 Data Contract；修改复用原
`SetConstant` 规范化、类型校验和可逆补丁。删除常量保持引用节点，由同一语义投影报告失效引用。

`update_nodes.removePins` 复用原精确端口删除能力，保留其余 Pin 的身份和顺序；与数量调整互斥。
`undo_resource` / `redo_resource` 复用 Project 图历史及过期版本检查，不自动保存；`save_resource`
调用正常 Save。模型目录不再注册通用图批次或单独图保存别名，内部编辑请求与恢复回执仍供明确工具共用。

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
| yss-desktop-gpui | 原生界面调度与类型化图投影消费 |

完整清单见 [Module Map](../../../../docs/reference/MODULE_MAP.md)。

Node 描述一种节点的端口、参数、类型约束及执行语义，不拥有某张图的节点实例或解析结果。
节点编辑投影的 `capabilities` 仅携带 `managed`；复制、创建副本、删除和剪切在前端统一要求存在明确的非受管理节点投影。Rust 继续按节点协议拒绝受管理节点的非法修改与子图导出。参数和内联字面量编辑直接消费各自投影，不另传节点级汇总开关。
三个 `yss-node-*` crate 均不依赖 Graph；`DocumentNode`、位置与连线属于图文档，连接后的类型、
Schema、血缘与诊断仍由 `GraphSemanticSnapshot` 统一管理。`yss-graph-type-mapping` 留在 Graph。
节点目录接收调用方提供的资源创建描述，不读取项目状态，也不解析图中连接。

`yss-graph-editor::projection` 拥有编辑器投影模型与纯映射，消费 Graph Analysis 的语义事实，
生成节点、端口、Schema、诊断与解析结果。Application 捕获输入、调用该能力并重验会话与资源身份；
原生宿主直接消费模型。投影不依赖 Application，也不构成第二份语义 authority。
资源成员是否为 orphan 由当前语义快照决定；持久化端口绑定中的 Resolved/Orphan 标记不覆盖资源丢失或恢复后的解析结果。投影仍核对节点、端点地址、绑定来源及连接数量，不改写文档。
迁移连线检查起点与终点，插入转接点检查原连线两端；这些地址统一经过 Graph Runtime 现有语义准入。已恢复成员的绑定调整随可逆编辑补丁提交，当前仍失效的成员继续拒绝操作，不在查询时改写文档。

`graph_connection_candidates` 为一个起点和 connect / moveConnections 意图查询当前图的全部端口决策。
Application 捕获当前编辑版本与资源事实、解析一次语义，在查询结束时重验资源、应用会话和图版本。
查询与编辑复用同一资源输入捕获；Application 查询直接持有当前文档引用，不构造 `GraphDocumentEditor` 的原始/待编辑文档副本或累计历史补丁。
Graph Runtime 对每个候选复用提交所用的 mutation planner，包括动态端口认领、重复连接、方向、同节点、类型、容量和顺序检查；查询不提交补丁、不写历史或 dirty。
Graph Editor 将规划补丁映射为 append、replace 或携带稳定原因码的 invalid；迁移预览的替换列表只包含被挤掉的连接，不包含自身迁移的连接。
桌面端口投影只交付连接数量与可用动作，不发送接受类型域、连接上限或顺序供前端再推导。Assistant 的图检查仍消费后端的接受类型说明和连接上限。
响应携带语义输入 hash，前端按编辑版本、语义和资源发布身份接纳，并在下一次身份变化时丢弃。
投影按当前起点提供 O(端口数) 的载荷，避免发布整图所有端口对；同一查询共享解析快照。
预检只说明该快照下的连接决策，实际提交仍验证最新文档与资源。

创建目录、编辑校验与剪贴板导出共用 `ProjectCatalogResources` 中已验证的项目声明。
编辑目录将资源 revision 与已有 `FunctionSignature` 绑定，不再另建字符串函数签名或在连接时重新解析
类型名。剪贴板导出只捕获声明，不读取数据库 Schema。目录查询与创建并连接共用声明端口及函数动态
成员的候选构造，保留成员身份、顺序、类型和回退标签。既有端口优先消费语义快照；同一原子补丁中新建
的端口及无快照的纯 Editor 调用，仍由协议、文档常量和捕获资源进行校验。
`CreateNode` 必须携带 `parameters`（快速创建为空映射）。Graph Editor 将其与资源绑定参数合并，
复用后续 `SetParameters` 的 null 清除、条件字段清理和值校验，再建立候选节点和端口。
默认值保持协议所有；编辑可以缺少必填项，但不能提交错误类型或固有约束错误。
初始参数不能删除或改写目录绑定的资源。创建并连接读取已配置候选的常量或资源类型，
整个创建、初始参数、端口及连线仍属于同一个可逆补丁。
无快照常量连接预检复用 Analysis 的 `referenced_constant`，按当前有效的显式值、默认值和条件读取，
不因默认选择而退回未约束类型，也不以隐藏的历史引用收窄当前端口。
初始用户端口按所属成员分组的最小数量筛选，未分组模板使用自身最小数量；函数动态候选按 resolver 和
方向选择，模板 key 只作为身份。无快照连接与剪贴板导入共用函数成员类型查找，同时核对 resolver、
方向和成员 ID，不能仅凭类型相同接受归属错误的绑定。

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

Harness 的 `browse_nodes` 复用该目录的搜索与类别过滤，分页只交付类型、名称、短说明和资源绑定；`inspect_node_type` 按精确类型 ID 批量投影 Protocol 的配置 Schema 与端口数量约束。两者均不要求打开图或运行节点，完整定义不随目录页重复传输。

能力指纹进入 Graph 的语义输入哈希和 Execution 的 `PlanBasis`。Execution 在计划准备、资源准备和执行入口
检查该指纹；同一图文档不会复用能力版本不同的计划。节点输出缓存
的输入指纹也包含该能力指纹。普通数值扩展的[注册与执行样例](../session/components/tests.rs)
复用现有 Node 类型及调度语义；注册 API 不承担新的语言语义或动态插件加载。

Harness Worker 的执行请求可带资源授权上限。Application 在准备时核对实际资源需求、
解析后的函数/数据库依赖以及捕获的资源版本；超出范围的图不能取得公开 run ID 或开始计算。
资源需求直接从本次请求携带的图文档读取，不为此复制整份 ProjectData；Project 的执行授权仍在原准入入口核对。
GUI 请求继续使用原有项目授权；角色策略本身由 Harness Core 拥有。

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

编辑器读取与写入响应使用同一投影同步协议。Rust 按 window/project/graph/locale 缓存有界基线；首次读取、基线失效、后端编辑会话变化或增量不划算时发送 snapshot。小变动使用 set/remove/splice 路径批次，客户端在候选上应用补丁、冻结并完成结构与引用一致性检查后发布。校验可以复用未变化且已验证的不可变对象，图身份、实体引用和跨字段约束仍针对当前候选检查，失败时不接纳该基线。未变化的节点、端口及连线复用引用；[前端投影 Store](../../../../react/src/features/README.md)按变化实体更新索引并原子安装。该协议只交付读投影，不代替 Graph typed 编辑操作。

`GraphEditorSessionDto.resultState` 与文档、语义投影来自同一次图操作。ResultStore 在更新输入依据的同一锁内捕获结果摘要，携带执行会话内的结果 revision；运行发布、运行准入和依赖重验推进该顺序，独立于图编辑 revision。前端要求摘要与投影的 semanticInputHash 一致，保留已安装的更高结果 revision，防止迟到的编辑/读取回执回退运行结果。

两端基线均限制为 32 条、32 MiB 序列化预算，单条超过 16 MiB 时只读而不缓存。交付中的 `snapshotBytes` 由 Rust 在编码时计算，前端据此计量，避免主线程为了缓存预算重新编码完整图。增量最多 512 个操作；字节预算与条目限制不代替真实桌面的安装耗时、长任务及帧率验收。

公共运行路径在提交执行状态后统一发布图活动，GUI、Harness 与内部调用遵循同一规则；调用者的专属 sink 不负责公共发布。GPUI 直接消费同一类型化运行通知。
Application 的 `RunIdentity` 包含执行会话、图路径、RunId 与本次准入已验证的 semantic input hash；所有通知、执行回执和恢复快照保留这份身份。宿主只转交该 hash，前端不能用通知到达时的当前图替换它。加载图的运行与失败展示按该依据筛选，已保留结果的查看仍按完整结果引用处理。
取消请求携带开始事件的 executionSessionId 与 RunId，Application 核对捕获的执行会话后才取消其中的运行，拒绝旧会话对后继同编号运行的请求。前端运行状态保存完整 run 身份，终态同时匹配会话与 RunId；取消不依赖输出绑定，因此 outputs 为空的运行同样可取消。
`RunApplicationEvent` 另携带产生事件时从 ResultStore 捕获的 `result_revision`：开始通知在输出失效后捕获，成功终态在结果发布后捕获。失败和取消也读取已提交的结果顺序；版本不属于整次运行不变的身份。公共通知、专属 sink 和恢复保存同一事件值。前端按摘要是否覆盖这一版本派生输出等待状态，不在查询完成时另写确认标记。
`run_graph_with_sink` 成功时返回 `RunGraphReceipt`，包含运行身份和本次 handoff 实际发布的结果引用、输出与类别。回执在交付结果查看意图和终态之前捕获，不通过随后可能已变化的当前结果索引重建，也不复制结果 payload。Harness 对这些引用执行响应条目预算并明确报告总数和完整性；只需要 RunId 的内部调用继续使用 `run_graph`。
Application 按结果 ID 与完整 requester 身份检查观察请求重复；多个 View 节点可以查看同一结果，同一 requester 对同一结果的重复请求仍被拒绝。
Application 保存运行事实的恢复投影：每个活动运行的开始身份与输出，以及每张图最新运行的终态和失败详情；不保存操作事件历史。`get_execution_snapshot` 按项目会话返回这些当前投影，初始订阅和 Resync 不再依赖前端曾收到的 RunId。
通知与命令回执在既有 Graph FIFO 中协调。前端恢复期间有界暂存后续事件，先安装快照再处理通知；公共订阅、专属 sink 和恢复共享按执行会话、RunId 与终态去重的安装规则。结果打开意图只由公共观察路径处理，不在恢复时重放。
执行提交中、后端运行状态与同步中断分别处理。只有 RunErrored 或权威恢复中的失败事实才写入 Output；Command 拒绝不等于计算失败。交付中断后先查询当前执行投影，无法确认时保留“运行状态未知”，不自动重跑。成功结果始终先提交再通知。
运行事件由统一安装器直接写入运行投影。Pin 查看查询当前输出结果，不另行提交运行；执行需求统一使用默认需求或显式输出集合。前端恢复统一查询当前执行快照，运行注册表内部仍按 RunId 管理准入、取消和终态。

关闭视图、HMR、窗口销毁和 Project replacement 释放订阅或使旧回调失效。最后一个编辑器关闭并完成既有保存/放弃流程后可卸载驻留数据，再次打开生成新的编辑身份。Harness 可直接打开已关闭的图，内部读取使用后端分配的 lifecycle token。

关闭最后一个 Graph 标签页前先经过该图 FIFO 屏障，再读取 dirty 和编辑版本决定保存/丢弃。普通缓存释放也进入同一队列，Rust 在提交边界拒绝释放 dirty 文档；明确丢弃携带用户确认时的 GraphEditVersion，版本变化则拒绝丢弃。卸载返回是否允许释放（已经不驻留也视为成功），只有后端确认且 lifecycle 仍有效时才清除前端文档、投影和资源状态；保留或失败时恢复缓存可用标记。清理后的迟到回执不能覆盖重新打开的会话。

Graph Service 在 IPC 边界仅接纳布尔卸载确认；函数签名修改的回复复用资源回执 parser，
Application 继续核对请求关联、签名版本和项目发布身份，组件不重复解析这些已验证回复。

Project watcher 保留未保存的当前文档。资源重命名和函数签名事务只持久化其负责的变更，不顺带保存图正文；重命名同步更新历史中的资源引用及保存指纹。节点目录和资源索引仍由现有 Project publication owner 安装。

Graph 重命名在文件系统 lease 内枚举持久化图及驻留图，对当前文档、磁盘正文和可逆历史分别重写引用，再通过同一文件事务与版本校验提交；未加载的调用图保持未加载。Application 将捕获会话的冻结 Registry 借给 Project；`graph_references` 通过注册角色选择 call 的 target、entry/return 的 function 参数，并重写动态端口 FunctionParameter 来源，保留普通文本、字面量和端口实例身份。当前适用的默认引用会成为新路径的显式覆盖，不修改全局节点声明。复制复用此引用规则，但仍单独重新分配节点、连接与动态端口实例身份。

常量复制引用由 Document Edit 按注册协议的 `GraphConstant` 参数声明统一识别；Registry 校验保证
`ConstantOutput` 指向此类参数。整图复制和剪贴板共用该读取及重映射，不依赖内置节点 ID 或固定参数名。
常量编辑请求携带明确的类型和值；Rust Document 负责规范化及校验，类型变更所用初值由调用方在同一请求中提供。
显式引用即使隐藏也保留，缺少显式值时只读取当前适用的默认值；非法显式值不回退，普通文本不参与重写。
剪贴板将源默认引用写入副本参数并携带去重后的常量，源文档不变；导入按原有身份/内容及名称冲突策略复用或复制常量，
引用与常量作为同一可逆补丁提交。整图复制为常量分配新身份，保持对副本常量的引用；条件在重映射前按源参数求值。

函数签名回执包含当前驻留图及其调用可达函数中的直接签名消费者，并沿正文调用边传播到外层调用图。
中间函数未驻留时，Project 在事务准备期间临时读取正文；Application 按回执使受影响图的结果失效，
只向仍驻留的图发布 `GraphActivity::Changed`，不自动打开中间函数。仅读取外层签名的叶节点不消费其正文依赖。
当前结果查询仍独立重验依赖版本；明确指定的历史结果和已有结果租约遵循原有读取生命周期。

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

运行准备消费当前不可变的 `GraphAnalysis`。Execution 的 `GraphExecutionScope` 先按 output demand 选出生产者与必要上游，
Application 使用 Analysis 的范围就绪检查与资源引用准备授权，再由 `prepare_graph_package` 生成或复用对应范围的计划。
普通全部运行仍要求全图可运行；明确选定输出时，无关未完成节点、旁支资源和独立依赖环不阻断目标范围。
命中计划缓存前也必须检查当前范围的可运行性。计划准备没有独立的用户操作或前端状态。

执行能力检查新增阻断诊断时，同步将完整解析的 snapshot 标为 Incomplete，投影为 `analysisBlocked`；`success` 不得同时携带阻断诊断。已有 InternalFailure 保留原故障信息。

`semanticInputHash` 来自语义文档内容、registry 与实际读取的 dependency manifest，排除 node position/user label、无引用 derived metadata 和相关展示字段。manifest 记录所用函数签名/正文、变量类型、数据库 Schema 以及 absent lookup；basis 同时传递 resource versions/observations。无关 catalog 变化不改变 artifact identity。函数正文读取由 Project owner 完成，Graph resolver 不读文件。

函数正文捕获与 Analysis 调用图校验共同使用 `yss_graph_analysis::direct_function_dependencies`
及同一会话的冻结 Registry 识别直接调用目标，包括扩展角色和适用的默认引用；Application 继续遍历传递依赖、从捕获的 Project 会话读取正文并重验身份。
该遍历入口不持有资源状态，不读取文件，也不替代 Analysis 的函数 ABI 和循环诊断。

执行包的函数库使用同一次捕获的 Registry、资源事实和 Analysis；嵌套调用仍受原资源授权与最终版本重验约束。
表格函数返回后，列反馈通过 Analysis 的函数成员地址识别派生输出，包括未 claim 的结果 Pin；
它沿用当前 ResultStore 的身份和有效性检查，不把一次实参的列写入共享函数正文。
调用内部的缺列错误保留函数图路径及实际引用列的节点，由普通运行失败通道交付。

保存状态与计划准备分开。内部计划缓存按图路径、语义输入与节点范围匹配；移动节点、修改显示标签可以复用计划。请求通过编辑版本和语义身份判定 currentness，前端只消费相应投影，不保存独立的计划身份或准备状态。

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
| 操作拒绝 | 类型化 Application 错误 |
| 用户反馈 | 原生宿主本地化 |

详见 [Runtime Signals](../../../../react/src/features/application/observability/README.md)、[Application](../../README.md)、[GPUI host](../../../yss-desktop-gpui/README.md) 与 [Rust workspace 验证](../../../../README.md)。

## 相关模块

[语义解析](../../../yss-graph-analysis/README.md) · [解析缓存](../../../yss-graph-runtime/README.md) · [执行与 ResultStore](../../../yss-graph-execution/README.md) · [数据契约](../../../yss-data-contract/README.md) · [Node Kernel](../../../yss-node-kernel/README.md) · [Results 查询](../../../../react/src/features/application/results/README.md) · [画布](../../../../react/src/modules/graph-editor/README.md) · [Problems](../../../../react/src/modules/problems/README.md) · [Output](../../../../react/src/modules/output/README.md)

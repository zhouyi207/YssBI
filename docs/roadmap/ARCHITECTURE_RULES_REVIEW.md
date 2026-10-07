# 架构规则循环复核

> Status: Planned
> Scope: 截至 2026-10-03 的全项目前端与 Rust 架构规则复核、修复证据、性能测量及人工验收记录
> Canonical owners: 源码和模块 README 拥有当前实现；本文记录本次复核进度，基准源码和原始输出拥有测量事实
> Update when: 本次复核结论、用户验收或明确跳过的测量范围需要更正时

目标是让当前项目全部模块符合 [前端规则](../../react/src/.rules) 和
[Rust 组织规则](../../crates/.rules)，可通用的职责、分层和所有权要求同时用于两端。
规则本身允许的局部状态、框架状态及边界拷贝不算例外；确需偏离默认方案时，必须在本文提供
同语义 benchmark、行为验证和保留理由。**项目管理及七组功能人工验收已由用户确认通过；
用户明确要求本轮跳过画布 Performance 录制及量化分析，按用户确认的验收范围完成本轮复核。**
后文保留各批次当时的检查结果与局限，历史待办以本节及末尾最新验收回执为准。

## 核对要求

| 要求                   | 闭环所需证据                                                                | 当前状态                                                                                                           |
| ---------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Zustand 和窄订阅       | 逐个共享状态 owner、写入入口、selector 与实际消费者核对；一笔事务只发布一次 | 生产 owner、写入和读取边界已核对；事务与消费者回归见各批，用户确认七组功能交互通过                                 |
| 不可变批次与引用共享   | 变化分支更新、未变实体引用、候选失败不污染已发布状态的行为证据              | 内容、引用、隔离与跨 owner 发布回归见各批；用户确认资源编辑、切换及呈现功能通过                                    |
| 输入边界校验           | IPC、存储、导入入口复用既有 schema/parser；缓存身份和跨字段约束未丢失       | 项目八字段回执已修复并共用 Rust/TS 样本验证；用户于 2026-10-03 确认项目管理七项人工复验全部通过                    |
| Graph 增量同步         | 唯一同步与补丁入口、身份及基线校验、候选原子发布、恢复路径                  | 发布、恢复、身份与并发回归见各批；用户确认画布、项目切换、关闭重开与取消恢复功能通过                               |
| 高频路径               | 面板内暂态、按 ID 读取、稳定配置、独立数据与绘制测量                        | 功能人工验收通过；数据路径测量见各批，画布 Performance 录制及量化分析按用户要求本轮跳过，明确记为未测              |
| 清晰结构和单一职责     | Rust 规则第 1、2、3、7、9 项：实际职责、函数、调用关系与目录对应            | 模块生产源码及实际职责边界已核对；声明与入口方向已合并，修改入口的跨 owner 行为证据见各批                          |
| 最小接口和依赖方向     | Rust 规则第 4、8 项：imports、可见性、Cargo 依赖、状态生命周期              | 71 包、248 条非 dev 仓内声明、独立 dev/target/build 段及 17 个前端 public/静态与显式动态入口已核对，未发现新越层边 |
| 唯一事实源和不过度抽象 | Rust 规则第 5、6、10 项：无平行可写模型、重复实现或失效兼容路径             | 重复状态、计算和校验按原 owner 收敛；本批补齐 FE/RE 中心化共享及 IV 不可用诊断的 typed 边界                        |
| 有证据的例外           | 代表规模、同等语义、对照、原始结果、正确性与局限                            | constants.dataValue 与另外三个 Graph JSON 边界已取得同外壳直接对照的局部保留依据；不外推其他路径                   |
| 最终交付               | 全模块复查、受影响消费者检查、文档同步、差异检查及必要人工验收              | 按用户确认范围完成；源码、行为、规则例外 benchmark 及功能人工验收已有证据，画布量化性能保留未测记录                |

## 已修复的偏差

| 范围               | 修改及所有权                                                                                                                  | 行为证据                                                                                                     |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| 全局对话框和进度   | 现有 `UIStore` 内使用 Zustand；弹窗与进度分别订阅；删除自制监听器                                                             | 类型检查通过；用户确认项目管理和七组功能交互通过                                                             |
| 项目注册回执       | Services parser 严格接纳 Rust 八字段记录，保留身份状态和合法空身份；删除授权继续归 Rust                                       | Rust/TS 共用样本，真实旧红转绿；四个消费者文件 60 项通过，用户确认项目管理七项人工复验全部通过               |
| Assistant 投影     | session 编排、纯事件归约、React 适配分开；Context 传稳定读取接口与动作；消息内容直接承载工具、引用和计划                      | 现有运行时重放、迟到响应、文本顺序用例；新增未变分支及工具终态回归                                           |
| 日志 recent buffer | Zustand 唯一持有 stream、entries、水位和截断标记；批次不再重复复制数组                                                        | 批次一次通知、去重、截断、换流、清空保留水位                                                                 |
| 文件输入缓冲       | 原注册表继续拥有 buffer；buffer 内由 Zustand 保存 text、dirty、generation 和 version                                          | 提交失败、新输入、旧 discard、文件关闭和项目重置回归                                                         |
| 视口 session       | 每个 group/graph 使用独立 Zustand store，移除独立监听器表；提交相同坐标不重复通知                                             | 分屏隔离、两个订阅者各一次通知、提交、释放和重新订阅                                                         |
| 项目快照恢复       | `projectSnapshotResources` 一次 Immer 更新资源、文档和图表候选；协调器保留提交与身份重验                                      | dirty 文档保护、事件先到、迟到删除、重命名、原状态不变和未变引用复用                                         |
| 插件运行时         | 根模块保留 manager 与 registry；安装、激活、bridge 分开；文件读取归 storage 并复用重定向谓词                                  | 库与原生 NUTS/取消/卸载证据见各批；用户确认插件安装、维护、卸载、页面重载和运行取消交互通过                  |
| Graph Analysis     | 快照、解析和投影分开；类型求解按领域规则、流程与缓存分工；Schema、类型和循环诊断共用一次拓扑计算；索引借用本次输入            | Analysis 33、Editor 13、Runtime 13 个测试通过；新增两个缺失节点端点回归均先失败再通过                        |
| Graph Execution    | 状态 owner、准入、控制、调度选择、执行编排与生命周期分开；RunRegistry 同时持有运行状态和取消对象，删除独立控制表              | Execution 30 个测试通过；新增 3 个回归均先复现失败，再通过：unwind 释放、会话排空取消、finalization 期间取消 |
| ResultStore        | 单一 registry 和锁继续拥有结果；输入有效性、原子发布、租约回收和只读投影分开；结果在解锁后释放；删除未使用的整体重置入口      | 原有 8 个结果测试和 Execution 全部 30 个库测试通过；消费者检查见下文                                         |
| 资源生命周期       | Project 激活清空登记时保留 registry 登记 ID 的单调性，防止仍存活的旧守卫影响新项目回退链                                      | 新回归先失败再通过；生命周期 15 个、Project 46 个库测试通过                                                  |
| 文件事务           | 公开契约、准备/提交、路径规则、文件变更和修改前状态记录分开；工作区同时持有 lease 与暂存区并负责候选放弃清理                  | 新增 3 项回归；Filesystem 57 个、Project 46 个库测试通过                                                     |
| 文件监听           | 会话切换、epoch 准入与 source 排空分别归属明确模块；单个排空状态持有句柄和终态，启动候选用守卫撤销                            | 三项回归先失败再通过，覆盖并发排空、失败终态保留、启动失败和 unwind 后的旧 sink 撤销                         |
| Results 前端       | 唯一 Zustand 查询投影、运行编排与图展示读取分开；一次 Immer 更新发布相关绑定、数据及运行状态                                  | 两项回归先失败再通过，通知由四次降为一次；已持有报告引用不变，消费者检查见下文                               |
| Settings 前端      | 设置安装共用原 Store 入口和浅比较；重载一次发布，保留未变设置分支与本地待保存字段                                             | 两项状态回归先失败再通过，覆盖重复重载、同值更新、远端更新与保存                                             |
| 节点目录缓存       | 每个项目、语言只保留当前接受的响应；一次 Immer 更新响应、请求与水位并移除被替代版本                                           | 现有版本/指纹用例先复现旧缓存残留，再验证替换、其他 owner 隔离和旧快照不变                                   |
| 项目事件入口       | 原 FIFO owner 内消除消费者与恢复任务的循环等待；关闭先等待消费者再等待恢复；复用 Service 事件类型并删除重复声明               | 溢出后消费者再次请求恢复的回归先失败再通过；关闭期间迟到的拒绝恢复仍被排空等待                               |
| Workbench 读投影   | 原生 Model 提交投影独立到内部 Zustand owner；选择订阅替代监听器表；主 Logs 快照也由原 runtime 内的 Zustand 发布               | 既有通知、激活、浮窗及关闭、命令目标、项目切换和租约消费者共 81 个用例通过，未新增 UI 测试                   |
| Workbench 恢复     | 默认面板安装归原 Defaults，存储处理归原 Persistence；Controller 保留生命周期；解析拒绝未知封套字段                            | 新增纯存储契约回归先失败再通过，保留两部分独立恢复；17 份相关测试共 82 个用例通过                            |
| 平台窗口生命周期   | 原几何适配器清理部分订阅，仅成功恢复后绑定页面；窗口动作按挂载身份接纳回执；三个窗口消费者依赖稳定动作                        | 两个纯平台生命周期回归先失败再通过；12 份窗口、租约、初始化及数据库身份测试共 26 个用例通过                  |
| Application 会话   | 身份组合、slot 状态、替换与恢复分开；候选拒绝清理及切换通知在锁外执行；直接使用执行排空结果；测试构造复用生产初始化           | 8 个会话测试通过；新回归先复现通知期间无法读取 slot，再验证订阅者能捕获新 epoch                              |
| Application 数据库 | 查询、编辑、输入转换、Project 授权适配及导入导出各归所属模块；去掉父模块通配导入；复用声明读取和重命名快照                    | 聚焦 13 个用例通过；完整库和集成检查结果见下文                                                               |
| 项目元数据候选     | 数据库索引转换归原 databaseRecords；图元数据候选归原 GraphMetaStore，首次加载与发布共用转换；保留未变引用和输入隔离           | 两个纯候选回归先失败再通过；13 份发布、加载、函数和数据库消费者测试共 68 个用例通过                          |
| 资源最终发布       | ResourceStore 同时拥有索引和文档状态，摘要与文档一次 Immer 更新；删除独立文档 Store；同值安装保留引用且不重复通知             | 两个纯状态回归先失败再通过；28 份受影响状态、发布、保存/丢弃和项目切换测试共 95 个用例通过                   |
| 发布水位与恢复     | 空回执和同内容索引推进原资源版本；同内容恢复仍刷新干净图；同版本刷新保留已知回执指纹                                          | 三个纯应用层回归先失败再通过；15 份发布、目录、项目事件、加载切换和保存消费者测试共 78 个用例通过            |
| Graph/文件回执     | 原资源 owner 一次安装文档标记、摘要和修订；图保存使用回执资源修订；Mind/Doc 保留输入缓冲 dirty 和 missing                     | 两个纯状态回归先失败再通过；23 份状态、图操作、文件、发布及编辑器消费者测试共 96 个用例通过                  |
| 图表正文与状态     | 图表草稿并入原 ResourceStore，载入、编辑、保存结算、关闭及快照同时发布正文与状态；删除独立图表 Store，保留未变分支            | 两个纯状态/应用回归先失败再通过；24 份状态、发布、加载关闭、保存与 Details 消费者测试共 100 个用例通过       |
| Mind/Doc 投影      | 带类型正文快照并入 ResourceStore；安装、重命名、释放及项目裁剪与资源标记同步发布，删除独立投影 Store 和工厂；首读检查资源身份 | 三个纯状态/应用回归先失败再通过；32 份文件、项目、图表、图编辑和关闭消费者测试共 123 个用例通过              |
| Graph 当前结果     | 当前 Pin 与搜索使用图摘要和查询缓存的只读投影；失效绑定立即隐藏，报告租约保留；接纳、读取及清理共用有效性判断与输出索引       | 两个纯应用回归先失败再通过；25 份结果、图、执行、项目发布和生命周期测试共 106 个用例通过                     |
| Graph 与资源发布   | 图会话、实体和结果摘要并入 ResourceStore；回执、项目快照、卸载及重置与资源标记同步发布；删除独立 Graph Store                  | 两个纯应用回归先失败再通过；59 份受影响消费者测试共 271 个用例通过；现有解析/安装与批量发布基准已复跑        |

当前契约分别见 [Features](../../react/src/features/README.md)、
[文件操作](../../react/src/features/application/resource/README.md)、[Logs](../../react/src/modules/logs/README.md)、
[Harness](../../crates/yss-harness-core/README.md)、
[Plugin runtime](../../crates/yss-plugin-runtime/README.md)、
[Graph Analysis](../../crates/yss-graph-analysis/README.md) 和
[Graph Execution](../../crates/yss-graph-execution/README.md)、
[文件事务](../../crates/yss-filesystem/README.md)、
[Results 查询](../../react/src/features/application/results/README.md)、
[Application 会话](../../crates/yss-application/README.md#应用会话)及
[数据库用例](../../crates/yss-application/src/database/README.md)。

Execution 的问题有实际行为差异：原先通知回调 unwind 后保留取消对象，
`cancel_and_drain` 不发送取消，Finalizing 期间的取消请求也无法触达原控制对象。
现在运行登记与关闭共享准入锁，取消与终态共享 registry 锁；终态释放取消对象，
finalization 仍由 Application 提交。排空每次唤醒后重验工作数。
派生端口索引只在单次解析内借用已有绑定；本轮没有为它宣称已测量的加速比，也没有批准规则例外。

会话替换此前在持有 slot 写锁时调用旧会话订阅者，回调重入 `capture_session` 会死锁。
现在先原子安装完整候选并完成替换守卫，再释放锁、通知并释放旧 owner。
slot 仍是唯一可写状态；替换与恢复模块只推进其阶段，没有新增 manager、锁或平行会话模型。
数据库拆分保持公开类型、用例签名、授权重验和提交补偿协议；未增加兼容层或新的运行时状态。
这些调整遵守默认规则，没有用未测量的性能推断申请例外。

ResultStore 原先将输入重验、运行发布、结果投影和窗口租约集中在一个实现文件中，
最后持有者释放结果时还会在 registry 写锁内析构 payload 或关系句柄。现在按上述职责拆分，
所有写入仍通过同一事务入口，在锁内移除索引，将移出的值留到解锁后释放；没有新增常驻回收队列或锁。
候选 output 到 result ID 的映射在加锁前构建，完整批次的授权校验与安装仍在同一把锁内。
仓库没有 `ResultStore::clear` 的调用方，现删除该闲置入口，避免它重置会话内 revision 和已关闭 owner。
已有用例覆盖过时批次拒绝、共享值、依赖传播、窗口交接和最后租约回收；本次未为移动代码增加同构测试，
也未将锁外析构描述为已测量的性能加速。

Project 的激活事务已经将旧状态移到锁外释放，但 `ResourceLifecycleBoundary::take_state`
此前同时重置登记计数器。新项目复用 ID 后，旧项目守卫的析构会错误标记新登记；
撤销后继操作时就无法恢复仍存活的前驱。现在仅移出项目登记和令牌水位，登记 ID 在 registry
存活期间不复用，继续使用原有 owner、守卫和回退链，没有添加第二套会话或清理机制。
同时逐项检查了 `yss-project-operation` 的准入、完成、放弃和项目替换：它仅依赖身份契约，
Project 持有账本，Graph 回执已有自身 owner；现有实现职责内聚，保留原模块，不因文件长度拆分。

`yss-project-identity` 的实例、注册、根目录、会话和操作身份保持不同的值类型，revision 使用检查溢出的
推进入口；它不持有项目状态、文件访问或应用流程。`yss-resource-naming` 统一拥有名称校验、可移植
比较键和自动避重分配，Graph、Chart、Mind/Doc 路径及 Project writers 复用这些入口。
这两个 crate 的全部生产文件与上述消费者已核对，保留现有结构；各 3 个既有契约测试通过。

文件事务此前仅在提交后拥有 Drop 回滚守卫，准备成功后被 Project 的身份或版本重验拒绝时，
暂存文件仍会遗留。现在 `workspace` 沿准备、提交、完成过程持有同一份 lease 和暂存区，
放弃候选时先清理再释放 lease，清理失败交给已有 RecoveryMarker；显式提交与回滚继续拥有自己的错误处理。
事务契约、准备/提交编排、路径规则、文件变更和修改前状态记录按职责拆分，没有新增 registry 或业务状态。
清理前还从根目录重验完整祖先路径，避免暂存父目录被替换为重定向后误清理外部文件。
三项不同回归分别覆盖放弃准备后的清理、清理失败的恢复标记及父目录重定向保护；前两项在修复前实际失败。
可移植名称规则、移动失败恢复与 Project 授权边界保持原契约。

前端 Results 原先按输出逐条清理，两项输出同时失效会发布四次，并暴露中间的部分失效状态。
现在先由已有协调器取消相关请求，再在唯一 Zustand 投影的一次 Immer 更新中安装关联字段；
运行标记、旧数据回收和当前绑定的变化属于同一批次，图摘要对账也复用这一入口。
查询投影、运行编排与只读图展示分别归属明确模块；已有数据租约、在途请求及图事实 owner 保持不变。
两项纯状态回归覆盖运行开始和图卸载，均先复现四次通知，再验证一次完整通知与已持有报告引用不变。
这些调整使用默认方案，没有新增通用批处理框架、UI 单元测试或规则例外。

监听器原先用两个锁分别保存排空句柄和完成布尔值，句柄被另一线程取走时会错误返回 WorkerPanicked，
甚至提前安装新监听器；真正的 panic 终态在重试时又会变成 Drained。现在同一 DrainCell 状态区分待关闭、
可重试、正在排空、成功和失败，关闭及 join 回调都在锁外执行，等待者按截止时间等待并复用同一个句柄。
生命周期不再复制 finishing 标记；epoch 不再维护无人读取的 in-flight 计数。source session 继续负责已准入回调的完成。
启动失败或 unwind 会同时关闭旧 sink 的准入并释放 Starting 状态；竞争取得启动位置时重新处理当前会话。
没有新增业务 registry、跨层依赖或外部任务框架。协调器的最终 lease 取得不再返回永远成功的 Result，
唯一 Project 调用方同步更新；未使用的 roots 读取入口删除，只被单元测试使用的持有状态查询收回测试可见性。

本批次还逐文件阅读了 Filesystem 的根身份、原生路径比较、准入/排空、文件树访问、变化值类型、恢复标记和原生监听适配。
这些职责仍保留在原模块中；Cargo 没有内部 crate 依赖，Project/Application 继续拥有业务验证、项目身份和索引失效。
Graph 求解复用已存在的节点 binding 索引计算 Schema 指纹，并借用当前节点端口事实，不再为只读求解复制整份端口。
这两项保持字段、顺序、缓存身份和安装边界，不新增 benchmark 或宣称未测量的加速比。

随后将两份拓扑算法及三个阶段的重复求值统一到本次解析的 `DocumentIndex`，Schema、类型和循环诊断
共同借用一个顺序。原先缺失源节点会产生不存在的循环，缺失目标节点又会使全图 Schema 变为循环冲突。
现在只有两端节点存在的连接参与节点依赖，损坏连接仍产生阻断的 `PortUnknown`；无关分支保持已有
Schema 和类型。正常文档入口继续拒绝缺失端点，未放宽导入或运行准入；Analysis 直接调用也不能得到 Ready。
两个回归分别覆盖缺失源、缺失目标，均比较增量与完整解析的事实和资源依赖，并先在旧实现上实际失败。
没有外部调用方的独立循环查询入口直接删除，没有新增图状态或缓存层。

类型求解的根模块保留输入、缓存、输出和事实安装流程，类型域与泛型、节点声明与 coercion、
反向约束、缓存及指纹分别归属明确模块；正反向求解复用同一规则，移除子模块的父级通配导入。
这些函数移动保留公开类型、序列化字段、指纹域和缓存身份；原数值代数用例随所属规则移动，没有重复新增。
Schema 又复用同一 `DocumentIndex` 的输入连接和 binding，删除自身重复的两张端点地址表；
节点输入索引只借用文档连接，保留原指纹使用的文档顺序，单端口连接和动态 binding 各自的排序契约不变。
组合、聚合和变换求解不再为只读递归复制整份节点、协议和端点；其子模块使用明确的领域依赖。
既有缓存、重连、组合输入排序、聚合、变换和条件选择用例覆盖这些读取变化，没有新增同构测试。
此次结构调整没有采用规则例外，也未宣称未测量的性能加速；其余模块仍按覆盖清单继续复查。

静态 Schema 继续收回已有声明边界：残差/Cook 观测表及 Cox/参数生存模型预测表的固定字段
改用 Catalog 现有 `fixed_numeric_table` 声明，Graph 删除两组硬编码字段和专用 resolver 分支。
字段名称、顺序、Numeric 语义与 Kernel 已有输出校验保持一致；Registry 继续校验固定字段，
Graph 只解释 `SchemaExpr::Fixed`，没有新增类型、适配层或目录到执行层的依赖。
动态端口是否被引用改由投影读取已有连接计数和 literal；未引用的消失成员仍隐藏，已引用成员仍保留 orphan。

Application 捕获函数正文和 Analysis 校验函数调用图此前各自识别直接调用目标，现共同使用 Analysis 的
`direct_function_dependencies` 只读遍历入口。它保持原有合法路径、文档顺序和重复目标语义；
Application 仍负责从捕获会话读取 Project 正文，Analysis 仍负责 ABI、循环及缺失诊断。
没有新增依赖注册表、可写函数模型或跨层读取；原有传递调用、资源变化和未加载调用方重命名用例继续覆盖该路径。

前端目录缓存此前按全部历史 Registry 指纹和资源版本保留强引用，但请求和读取入口只指向每个项目、
语言的当前响应。现在接受新响应时，在原 Store 的一次 Immer 更新中移除同一请求的旧缓存项；
响应 key 的项目、语言身份保证不会删除其他请求的响应。刷新失败仍显示最后一次成功结果，
调用方已持有的旧响应和搜索索引也保持原值，没有新增回收队列、缓存 owner 或弱化迟到响应校验。

Settings 原先同步重载会先发布 loading 再发布数据，并在重载、保存或远端合并时替换未改变的设置分支。
现在所有安装路径共用领域更新函数，两个仅含标量的分支使用 Zustand 浅比较，未改变时返回原引用。
重载只有一次完整发布，相同值不触发重复通知；内部待保存补丁继续优先于远端同名字段，远端更新不回声。
JSON 序列化仅留在存储边界，不再用作状态等价判断；没有为简单标量字段引入通用深比较或补丁引擎。
两项新增测试是纯状态回归，不是 UI 单元测试；一项实际复现两次发布，另一项复现未改变分支的引用丢失。

项目事件入口原先在队列溢出时创建等待当前消费者的恢复任务，而消费者随后也可能请求并等待同一恢复，
形成 Promise 等待环，快照恢复与关闭排空都无法结束。现在请求恢复后消费者正常结束，恢复统一在其后执行；
关闭依次等待当前消费者和当时的恢复任务，包含关闭开始后由消费者拒绝触发的恢复。
事件协议与事件流条目分别复用 Service parser 和 stream 的类型，删除 Application 中的重复声明与无调用方转导出；
队列只保存通过入口接纳的事件，传输失败直接请求恢复，移除队列处理中不可达的失败分支。
没有新增队列、订阅框架、状态 owner、输入 schema 或协议转换，原有校验和项目发布入口保持不变。

Workbench 原先在绑定/FIFO 模块内用多组 Set 和按面板 Map 管理同一提交投影的监听器。
现在 `workbenchLayoutProjection` 单独负责投影与通知：一次发布冻结记录及语义、活动、成员通知快照，
Zustand 的选择订阅过滤无关变化。面板 selector 只读取自身记录、所属边栏和就绪字段，并作浅比较。
原生 Model 继续拥有拓扑、尺寸、顺序和选择，原 binding 继续适配原生动作/模型替换事件；
没有把原生可写布局或每帧几何镜像到 Store，也没有改变 FIFO、mutation revision 或事务基线校验。
主 Logs runtime 的既有离线布局快照同样用内部 Zustand 替换自制监听器，原接收/交付边界拷贝保持不变。
这次调整复用默认状态方案，没有以未测量的性能理由保留例外，也不宣称获得帧率提升。

Workbench 恢复协调器此前同时实现面板默认安装、恢复补全、JSON 解析、浏览器存储和异步生命周期。
默认安装及边栏默认值读取现归原 `workbenchLayoutDefaults`，存储读取/校验与已存项目面板清理
归原 `workbenchLayoutPersistence`，原位置直接删除。Controller 保留绑定、恢复、就绪回调、
持久化防抖及关闭 flush 的代际检查，安装仍通过原 hydration/事务入口；没有增加 manager 或平行状态。
同时修正持久化封套校验：原解析器接受 root/nested 与 nested.logs 之外的字段，与文档约束不符。
现在拒绝未知字段，仍独立标记缺失或无效的 root、Logs 部分；未添加旧格式兼容、迁移或第二份 schema。

## 性能测量

测量时间为 2026-10-02 01:22（Asia/Shanghai），代码基于 `7c7d843e` 的本次未提交工作树。
环境为 Windows x64、Intel Core i9-13900K、32 个逻辑处理器、约 95.7 GiB 内存、
Node v24.19.0、Vitest v4.1.10。共享开发环境未隔离其他进程；这是一次局部测量。

复现命令：

```powershell
pnpm bench:project:publication --outputJson docs/benchmark/probes/project-snapshot-results.json
```

[基准源码](../../react/src/tests/benchmarks/projectSnapshot.bench.ts)使用 100、1,000、5,000 个已加载图表，
包含资源元数据、文档状态及图表配置。输入在计时外建立并冻结；每例预热 100 ms，
正式测量至少 500 ms 且至少 20 次。两组调用同一个生产候选构建函数，对照组只多做一次
`structuredClone(current)`。它测量**已有状态额外深拷贝的成本**，不等同于历史实现整体前后对比。

| 图表数 | 直接共享平均 ms | 额外深拷贝平均 ms | 样本数 共享和拷贝 |
| ------ | --------------: | ----------------: | ----------------: |
| 100    |          1.0626 |            1.3415 |         471 / 373 |
| 1,000  |         11.5298 |           15.0572 |           44 / 34 |
| 5,000  |         56.3886 |           74.9886 |           20 / 20 |

原始分位数、误差和样本保存在 [JSON](../benchmark/probes/project-snapshot-results.json)。
该样本中共享路径比额外拷贝路径约快 1.26–1.33 倍，支持删除内部已有状态的冗余拷贝，
没有理由据此绕过 Immer、Zustand 或输入校验。绝对耗时仅覆盖资源候选准备，
不包含 Graph 解析、IPC、Store 提交、React 渲染或浏览器布局绘制；也没有测量分配量或内存峰值。

## 范围覆盖和后续循环

以 [生成模块索引](../reference/MODULE_MAP.md)和当前 manifests 为清单来源，不能仅扫描大文件或
只处理本轮已修改的模块。文件长度只用于定位审查入口，不是自动判定架构违规的阈值。

| 范围                                          | 已取得证据                                                                                                                                                                                                                                                                                                                                                                                           | 尚需完成                                                                                                                   |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| app、components、shared、services、lib、utils | App 非 locale 逻辑 17、components/ui 23、ui-presentation 14、AG Grid 主题 1、lib 1、shared/theme 5、shared/charts 24、shared/ui 24、utils 4 已逐读；Services 52、shared/types 85 累计全文覆盖；另核对 12 份 Shared 辅助 TS、3 份 CSS 及 locale 静态结构                                                                                                                                              | 原生窗口、选择器/剪贴板/PDF/外链、Assistant 配置入口、语言切换和呈现验收；不扩为全部供应商或 locale 逐句校对               |
| features 的 Core、Domain、Application         | 当前 Domain 14、Core 95、Application 250 个生产 TS/TSX 累计全文复核；去重范围与目录、存储、投影及项目发布边界修复见下文                                                                                                                                                                                                                                                                              | 已修改项目发布/生命周期的真实界面验收；对应状态与应用回归见各批                                                            |
| assistant                                     | 已逐文件读取 5 个生产呈现入口与 CSS、5 个 Application 文件及 Service/Parser；Harness Core 22 个生产 Rust、内嵌提示及注册流程累计全文复核；工具摘要、重放、HMR 和排队依赖修复见各批                                                                                                                                                                                                                   | 真实流式内容、记忆、切换会话、故障重载、链接、HMR 与模型执行验收                                                           |
| document-editor                               | 已逐文件读取 6 个生产入口、输入缓冲/文档队列与 PDF Service；修复 Mind 节点引用复用、尺寸释放和视口逐帧 React 更新，36 项关联检查通过                                                                                                                                                                                                                                                                 | 真实编辑、保存/放弃、分屏、树布局、选择/测量、取消恢复和 PDF/外链验收                                                      |
| graph-editor                                  | 已逐文件读取当前 33 个生产 TS/TSX/CSS 文件及直接画布/结果读取链（含本批移回的 nodeAppearance）；共享适配、窄订阅及结果搜索证据见下文                                                                                                                                                                                                                                                                 | 项目替换后同路径图关闭/重开与取消恢复；真实画布/目录/结果搜索及分屏验收                                                    |
| logs                                          | 已读取全部 15 个呈现文件、6 个 Application 文件及 Service/Parser/Channel 链；原缓冲增量维护领域索引；日志插件 17 个生产 Rust 文件已复核，终止交付修复后 23 项 Rust 与 18 项前端检查通过                                                                                                                                                                                                              | 真实多领域分屏、持续追加、筛选、Details、队列积压/存储失败恢复、主/独立窗口与滚动验收                                      |
| chart、database-editor                        | 已逐入口检查 Chart 4 个、Database Editor 12 个生产文件；合并选择规则、收窄订阅并复用 AG Grid 行索引及批量接口，38 项关联验证通过                                                                                                                                                                                                                                                                     | 真实分页/拖选/复制、Ctrl/Cmd 全选、窗口失焦和图表编辑/迟到响应验收                                                         |
| details                                       | 已逐入口检查当前 37 个生产文件及所用 Core/Application 边界；修复常量输入、端口索引、参数草稿、目标订阅和 Mind/Chart 派生读取                                                                                                                                                                                                                                                                         | 真实目标切换、节点删除回退、草稿保留、连接选择、Mind 结构编辑及图表编码验收                                                |
| data-explorer                                 | 已检查全部生产呈现文件、公开入口及导入 Application 回调；修复全局进度操作所有权，23 项消费者验证通过                                                                                                                                                                                                                                                                                                 | 真实导入、嵌套弹窗、绘制与桌面取消验收                                                                                     |
| commands、node-catalog                        | 已检查全部生产呈现文件和公开入口，追踪活动面板、历史可用性、目录请求/搜索/创建调用；窄订阅及规范化复用后 32 项既有验证通过                                                                                                                                                                                                                                                                           | 真实菜单、目录拖拽、搜索及焦点交互验收，与侧栏和 Graph 使用同一条桌面验收链                                                |
| output、problems                              | 已复核两模块全部呈现文件、公开入口及诊断/失败读取链；修复共同定位流程的迟到激活                                                                                                                                                                                                                                                                                                                      | 真实桌面定位、分屏切换及参数字段焦点验收                                                                                   |
| project-explorer                              | 已逐文件检查原呈现链并将动作移入 Application；本批资源图标颜色归回本模块，当前 21 个生产文件，诊断/选择订阅、目录扫描、弹窗及操作准入证据见下文                                                                                                                                                                                                                                                      | 桌面资源选择/拖拽、重命名、确认期间换项目、新建/删除弹窗重开、并发操作禁用与迟到回执的跨 owner 验收                        |
| settings                                      | 已检查呈现、Application、Core 共 10 个生产文件及主题/语言/插件/弹窗消费者；收窄订阅并明确重置确认归属，33 项关联验证通过                                                                                                                                                                                                                                                                             | 真实主题/语言切换、跨窗口同步、嵌套确认焦点、页面关闭及重置反馈验收                                                        |
| plugins                                       | 已检查全部 4 个呈现入口、当前 5 个 Application 文件及 Service/Parser、Provider/根面板消费者；共享投影、操作准入和维护身份修复后 27 项验证通过                                                                                                                                                                                                                                                        | 真实安装/卸载与维护分页、关闭/重开、隔离页面、主题/语言及用户发起运行中取消的界面验收；原生计算链证据见各批                |
| results                                       | 已逐文件读取 16 个呈现生产文件并追踪加载、分页、报告 parser 和租约消费者；置信带统一由 SCI 产生，修复快速关闭面板的租约回收，关联检查见下文                                                                                                                                                                                                                                                          | 真实分页、报告切换/补选、快速关闭及独立窗口租约验收；已修改错误语义回归见各批                                              |
| workbench                                     | 已逐文件读取当前 68 个生产 TS/TSX 文件；完成原生复合命令发布、移动激活与浮窗最大化修复，90 项布局及直接消费者验证通过                                                                                                                                                                                                                                                                                | 真实拖动、分屏、浮窗移入/停靠、恢复、选择、折叠和关闭，以及面板/项目 owner 的跨生命周期验收                                |
| Rust Application、Graph、Execution、Project   | 已检查 Cargo 方向；Application 当前 113、Analysis 25、Graph Document 7、Document Edit 5、Diagnostics 1、Runtime 3、Editor 11、Execution 当前 32、Project Model 5、History 1、Registry 三包 4 及 Project 39 个含生产代码文件的生产区累计全文复核；四个图契约/映射包九文件已核对，各批实际范围另列                                                                                                     | 已修改路径的实际桌面、项目切换和文件生命周期验收；已定位并发及求解消费者回归见各批                                         |
| Rust Database、Node、SCI、Linalg              | Database Runtime 11、Store 8、Engine 17、Arrow 7、IO 3、Schema 1、Database Contract 9、Source 7、Dataset Profile 4、Data Contract 8、Relational Contract 4、Node Registry 4、Node Protocol 10、Kernel 当前 77/77、Catalog 当前 69/69、SCI Runtime 当前 40/40、SCI Contract 55、SCI Linalg 4 个生产文件已全文检查；SCI 当前 207/207 份生产实现已全文核销，精确清单见最新批次；仅 Linalg 直接依赖 faer | 已修改统计报告与数据窗口的桌面呈现验收；SCI→Runtime→Kernel 的已定位错误边界见各批，保留无关 Meta 改动                      |
| Rust Harness、IPC、UI contract                | Cargo 方向及 IPC Contract 十二文件、Channel/Event、Harness Core 二十二文件、SQLite/Contract/Rig 和 UI Contract 累计全部生产源码已逐读；profile 中断分类、Assistant 空正文与排队依赖发布修复见各批                                                                                                                                                                                                    | 已修改 Assistant 流程的真实模型执行与原生订阅生命周期验收；计划取值和错误分类证据见各批                                    |
| Rust Plugin runtime、protocol、SDK            | 宿主 Runtime 当前十个生产文件、protocol 五个 Rust 文件、生成脚本、SDK、Julia native 32/32 生产 Rust、Julia Web 80/80 生产 TS/TSX 及计算脚本 7/7 已全文检查；任务、视图、授权、迟到取消和 worker 回收见各批；原生计算验收按所测包和源码版本记录于对应批次                                                                                                                                             | iframe 重载后的 MessagePort 撤销、桌面运行中取消/后继状态及 Exponential 中英文 Rate 标签验收；采样中途取消进程证据见最新批 |
| 桌面与构建工具                                | 桌面组合三文件、插件封装/打包/原生检查三个脚本、依赖/模块索引两个生成器及实际 before-build 的 build_samples.rs/Samples README 已全文复核；依赖快照 71 crates/248 声明一致，签名/版本规则与各包的原生计算证据见对应批次                                                                                                                                                                               | 桌面生命周期验收                                                                                                           |
| 其余契约、命名、文件系统和基础 crate          | 已检查 Cargo 直接依赖；Project operation、Resource lifecycle、Project identity、Resource naming、Filesystem，以及 Canonical Hash、Display Naming、Project Progress、Project Layout、Math Expr、Chart Document 生产源码已逐读；修复与消费者证据见各批                                                                                                                                                 | 已修改平台窗口与真实文件监听场景的人工验收；相应生命周期和消费者回归见各批                                                 |

最新一批闭合 IV 已复现的三类不可用诊断、FE/RE 单向中心化重复实现及 Exponential 标签；
前一批的 Julia 取消、White/IM 准入和依赖方向证据继续有效。具体跨 owner 调用链的修复与回归见各批，
本表保留受影响模块的真实界面验收，不再用无具体问题的“其他跨 owner 组合”扩展循环范围。
前端 Core/Domain/Application、Rust Application、Graph Execution、Kernel、Catalog、SCI Runtime、SCI、Plugin Runtime、Julia native、Julia Web 与计算脚本当前生产清单已在下方各批核销，后续不重复列为未读范围。
源码全文覆盖仅核销阅读范围，各动态端口、函数捕获及状态边界的修复和验证仍以对应批次为准。
普通算法精度探索、locale 正文逐句校对和插件目标中尚未实现的产品能力，不自动扩大本架构目标；
已实现能力的分层、唯一事实源、输入边界及本次改动引起的行为和性能验收继续保留。
前端 Settings 安装、目录响应生命周期及项目事件恢复/排空已修复。已读取 `readProjection`、
Graph 会话安装、Execution 读取和 Results 展示的实际调用路径。后续复核进一步移除了 Execution
读取与 Results 内部展示的通用递归合并，改为实际字段浅比较与一次 Immer 发布，具体证据见后文。
当前全生产调用已清点，`shareProjection` 仅剩四个 Graph JSON 边界，已分别取得下文同外壳直接
对照的局部保留依据；布局等旧调用已删除。不重复将这四处列为无证据例外，也不把它们的结论
外推到其他边界或新调用。相关源码未变化时复用既有行为与 benchmark 结果。
Settings 同步的实际调用方每次挂载创建独立 coordinator，未为没有调用依据的重复 start 场景增加抽象。
Workbench 的提交投影订阅、默认布局安装与存储处理已按上述边界修复，恢复协调器的绑定、就绪回调、
防抖和关闭流程已读取并保留身份重验。平台窗口的恢复失败、部分订阅和挂载身份已修复，创建回执与结果租约沿用原入口；
项目发布候选中的数据库声明、修订及图元数据已复用所属 owner 和引用共享；ResourceStore 的资源与文档状态
已合并为同一发布事务。发布协调器的快捷路径、恢复路径和同版本回执指纹已修复；已读取项目身份重验、重试、
dirty 图保护和迟到移动回执的实际调用链。Graph 与 Mind/Doc 回执的资源标记和修订现已合并发布，
图保存直接安装 Rust 的资源修订，文件继续保护输入缓冲和 missing 状态。图表正文及草稿现已并入
ResourceStore，加载、编辑、保存结算、关闭和项目快照同步发布相关状态，旧图表 Store 已删除。
Mind/Doc 正文、文档标记及资源摘要现已合并发布，同值安装保留未变节点，重命名及项目删除在同一事务中
移除旧路径；首次读取期间资源索引变化会使读取失效。Application 队列和输入缓冲继续保持独立职责。
Graph 帧与当前 Pin 绑定之间的读取边界已修复：已加载图按新摘要立即隐藏旧绑定，报告租约保留，
同值可见投影不重复通知。Graph 会话、实体与结果摘要现已和资源标记统一在 ResourceStore 发布；
单图回执、项目快照及卸载已覆盖同步可见性，独立 Graph Store 已删除。
运行事件、输出运行记录及失败摘要现已保留 Rust 准入时的真实语义身份；当前画布按该身份筛选展示，
RunErrored 的终态与失败详情在同一 Execution 更新中发布。
GraphMeta 的函数签名与资源名称、修订也已统一发布，独立元数据 Store 已删除。
逐输出运行活动、图终态及失败也已统一由 Execution Store 发布，Results 只保留查询和租约职责。
此前 Rust Catalog 的可用性用例及 Application Graph 的 49 个用例现已在当前工作树复核通过。
结果摘要确认已改为比较 Rust 事件版本，删除查询回执对输出等待标记的另一次写入。
前端日志生成已统一为无应用层依赖的入口，Core 领域信息不再经 console 丢失；
Application 统一释放发送连接、console 捕获与待发送队列，数据库删除也复用发布入口清理详情。
Database/Project 替换入口现已在 slot 写锁内核对原会话；数据库列表与会话工厂共用 Project
声明快照，插件调用复用一次捕获，声明删除在版本推进失败时保留原状态。
前端数据库声明也已并入资源发布事务，修改命令和图表预览复用资源 revision；独立 DatabaseStore
及数据库 revision 表删除，元数据补全不再接受声明身份或名称，缺失数据库不再套用文件缓冲保留规则。
数据库异步读取已统一核对项目、资源投影 revision 与调用方有效性；编辑器用一个请求身份完成元数据和分页，
并按资源版本刷新及隐藏失效页。后端查询已贯通预期资源版本；首次加载的 schema 绑定索引发布版本，
导入补全绑定创建回执，缓存随资源版本失效，详情复用同一版本读取入口。相关桌面验收仍开放。
其后各批已逐读其余生产入口，并沿具体调用链验证修改过的发布、身份和清理边界。
这不将所有独立 Store 视为一个整体事务；每笔事务的 owner 和受影响消费者证据见对应批次。
当前余项以覆盖表中受影响的真实桌面交互及最新批次点名的验收为准，不再把没有具体入口的
“其余生命周期/其他模块”作为继续扩大源码修改范围的依据。源码与数据处理测量不代替界面验收。

## 验证记录和剩余验收

各批次使用 L1/L2；没有运行完整 `pnpm ci`，也没有把类型检查或零匹配测试记作行为验证。
前端与插件批次的最终 L2 前端聚焦测试为 15 个文件、50 个用例通过；插件为 9 个用例通过；文档契约 6 个用例通过。
类型检查与插件 Clippy 通过。快照 fixture 补齐必填 chart 字段后，类型检查、该回归及 benchmark 已重新执行。
主要命令如下（前端测试参数列出实际的完整范围）：

```powershell
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/features/application/assistant/assistantHarnessRuntime.test.tsx src/features/application/assistant/assistantMessageContent.test.ts src/features/application/assistant/assistantHarnessProjection.test.ts src/features/application/log/logBuffer.test.ts src/features/application/log/logStore.test.ts src/features/application/resource/fileTextInput.test.ts src/features/core/viewport/viewportSession.test.ts src/features/core/viewport/resolveInitialGraphViewport.test.ts src/features/core/viewport/editorViewStateMemento.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/editorMutation/projectSnapshotResources.test.ts src/features/core/chart/chartDocumentStore.test.ts --reporter=verbose
pnpm test:rs:package -p yss-plugin-runtime --lib --test installation
pnpm lint:rs:package -p yss-plugin-runtime --lib --tests
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
```

TypeScript lint 退出成功，但报告了五处已有 warning，位于 projectEventStream、globalEvent、
logsRuntime 和两个既有 UI 测试。本轮不将它描述为无 warning。

Graph/Execution 批次执行了以下四个 crate 的完整库测试：87 通过，1 个原有手工 timing probe 忽略。
这次忽略项不计入通过数。

```powershell
pnpm test:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime -p yss-graph-execution --lib
```

Application 的消费者检查如下：

```powershell
pnpm test:rs:package -p yss-application --lib session::
pnpm test:rs:package -p yss-application --lib graph::
pnpm lint:rs:package -p yss-graph-analysis -p yss-graph-execution --lib --tests
pnpm format:rs:package -p yss-graph-analysis -p yss-graph-execution -- --check
```

`session::` 的 7 个用例通过。`graph::` 匹配 Application Graph 及 Automation Graph 的 49 个用例，
47 通过、2 失败。失败为 `localized_catalog_returns_resources_from_the_same_coherent_snapshot`
和 `all_non_deferred_builtin_nodes_are_available_in_catalog_and_sidebar`，涉及并行更新的统计节点
可用性清单；后一个用例当时报告 70 个尚不可用且未归入 deferred 的节点。该批次未调整这些断言或节点清单，
也没有将它们记为通过；后续 Application 批次的检查和修正另列如下。相关 SCI import 和 cluster-robust binding 的中途失败，在并行依赖更新后已恢复。

本批次 Rust 共 141 个用例通过、2 个目录用例失败、1 个原有手工 benchmark 忽略。
Graph Analysis/Execution 的最终 Clippy 退出成功，本轮修改的两个 crate 没有 warning；
依赖 `yss-sci` 的 hypothesis 模块报告 6 个 warning，保留在其并行变更范围。
两个 crate 的格式检查、三个文档的格式检查及 6 个文档契约检查通过，差异检查通过。
测试期间未运行完整 CI。

Application 会话/数据库批次完成了以下 L2 检查（2026-10-02）：

```powershell
pnpm test:rs:package -p yss-application --lib
pnpm test:rs:package -p yss-application --test database_test
pnpm test:rs:package -p yss-application --test numeric_execution cluster_and_adjusted_prediction_nodes_use_real_fits_and_canonical_parameters
pnpm lint:rs:package -p yss-application --lib --tests
pnpm format:rs:package -p yss-application -- --check
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
```

最终库测试运行 138 个用例：137 通过、1 失败。会话、数据库、项目生命周期、Harness、自动化、
图运行和 IPC 消费者的相关用例均已实际执行；单独的数据库集成 4 个用例及推断数值 1 个用例通过。
本批次共 142 个 Rust 用例通过、1 个失败，不重复累计中途的聚焦检查。
Application 的最终 Clippy 退出成功且该 crate 无 warning；依赖 `yss-sci` 仍报告 6 个 hypothesis
warning。Application 格式检查、三个修改文档的格式检查、6 个文档契约用例及差异检查通过。

两个目录用例的旧预期将已注册并可执行的 adjusted-predictions 视为不可用，现已按内核注册与
实际数值结果修正；IPC 测试复用同一组目录/Activity 可用性预期。数据库集成的旧输入具有 Text
语义，原本期待强制转整数成功，与现有“物理转换保留语义”契约不符。修正后的用例先检查强制
转换不能绕过 Text 约束，再显式设为 Identifier，验证无效值转 null 与撤销；底层转换规则未放宽。

剩余失败为 `all_non_deferred_builtin_nodes_are_available_in_catalog_and_sidebar`：该次运行仍有
58 个非 deferred 节点不可用，涉及 labels、决策、功效、设计质量、心理测量及调查分析。
未删除该断言，也未为通过检查扩充 deferred 清单。节点实现属于持续更新的工作树，后续必须
按最新代码复查，不能把本次会话/数据库修复描述为整个 Application 已通过。

本批次未运行完整 CI、桌面交互或原生插件验收；整体目标继续保持未完成。

ResultStore 与资源生命周期批次完成以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:rs:package -p yss-graph-execution --lib result_store::
pnpm test:rs:package -p yss-graph-execution --lib
pnpm test:rs:package -p yss-application --lib graph::results::
pnpm test:rs:package -p yss-application --lib graph::run::
pnpm test:rs:package -p yss-resource-lifecycle --lib retired_project_guard_cannot_abandon_a_replacement_registration
pnpm test:rs:package -p yss-resource-lifecycle -p yss-project-operation --lib
pnpm test:rs:package -p yss-project --lib
pnpm test:rs:package -p yss-project-identity -p yss-resource-naming --lib
pnpm lint:rs:package -p yss-graph-execution -p yss-resource-lifecycle --lib --tests
pnpm format:rs:package -p yss-graph-execution -p yss-resource-lifecycle -- --check
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src-tauri/crates/yss-graph-execution/README.md src-tauri/crates/yss-project/README.md
git diff --check
```

ResultStore 的原有 8 个测试在拆分前后通过，Execution 最终 30 个库测试通过。Application 的结果读取
16 个、图运行 3 个消费者测试通过；未重跑上一批的全 Application 库测试，不能据此覆盖目录可用性问题。
资源生命周期回归先实际失败，修复后该 crate 全部 15 个测试通过；操作准入 4 个、Project 46 个、
身份与命名各 3 个测试通过。本批次共 120 个不同 Rust 用例通过，不重复累计中途聚焦检查。
Project 消费者检查约 62 秒，其中主要时间用于依赖编译，46 个用例实际执行约 0.44 秒。

中途 Clippy 曾被并行 Kernel 改动中的 `Input::one` 调用阻断；工作树恢复为现有 `Input::fixed`
后重新执行成功，没有修改或回退该并行实现。最终两个修改 crate 无 warning；依赖 `yss-node-catalog`
报告 1 个类型复杂度 warning，`yss-sci` 报告 8 个 warning（6 个 hypothesis、2 个 decision）。
两个修改 crate 的格式检查通过；本批次没有新的 benchmark 或获批规则例外。
三份修改文档的格式检查、6 个文档契约用例及差异检查通过；Git 仅提示工作树的 CRLF 转 LF。

本批次仍未运行完整 CI、桌面交互或真实插件进程验收。

Filesystem 事务与前端 Results 批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:rs:package -p yss-filesystem --lib abandoned_preparation
pnpm test:rs:package -p yss-filesystem -p yss-project --lib
pnpm lint:rs:package -p yss-filesystem --lib --tests
pnpm format:rs:package -p yss-filesystem -- --check
pnpm test:ts src/features/application/results/runtime.test.ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/results/graphPresentation.test.ts src/features/application/results/resultLeases.test.ts src/features/application/results/usePagedResultRows.test.tsx src/features/application/results/resultReadErrors.test.tsx src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts --reporter=verbose
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src/features/application/results/runtime.ts src/features/application/results/runtime.test.ts src/features/application/results/resultProjection.ts src/features/application/results/graphPresentationRead.ts src/features/application/results/README.md src-tauri/crates/yss-filesystem/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

Filesystem 最终 57 个、其唯一生产事务消费者 Project 的 46 个库测试通过，共 103 个 Rust 用例。
前端 8 个文件、26 个查询、租约、图展示、既有读取组件和项目发布测试通过；复用既有 UI 测试，没有新增 UI 测试。
上述计数不重复累计中途聚焦检查。两个暂存清理回归和两个批次通知回归先实际失败，修复后通过；
第三个文件系统回归在 Windows 上实际验证了父目录 junction 替换后的外部文件保护。
Rust Clippy 和格式检查通过且无 warning；TypeScript 检查通过，lint 退出成功但仍有五处已有 warning，
位于 projectEventStream、globalEvent、logsRuntime 和两个既有 UI 测试，没有位于本轮修改的 Results 文件。
四份修改代码与三份文档的格式检查、6 个文档契约用例及差异检查通过；Git 仅提示部分工作树文件的 CRLF 转 LF。
本批次没有新增 benchmark 或规则例外；未运行完整 CI、桌面交互、真实插件进程或 Linux/macOS 原生验证。

Filesystem 监听与 Graph 求解读取批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:rs:package -p yss-filesystem --lib watcher::tests::
pnpm test:rs:package -p yss-filesystem --lib watcher::tests::failed_start
pnpm test:rs:package -p yss-filesystem --lib watcher::
pnpm test:rs:package -p yss-filesystem -p yss-project --lib
pnpm test:rs:package -p yss-application --lib project::
pnpm test:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime --lib
pnpm test:rs:package -p yss-graph-runtime --lib
pnpm lint:rs:package -p yss-filesystem --lib --tests
pnpm lint:rs:package -p yss-graph-analysis -p yss-graph-runtime -p yss-project --lib --tests
pnpm format:rs:package -p yss-filesystem -p yss-project -p yss-graph-analysis -p yss-graph-runtime -- --check
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src-tauri/crates/yss-filesystem/README.md src-tauri/crates/yss-graph-analysis/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

三个监听回归先在原实现上失败，修复后通过；并发排空用例进一步覆盖真实 unwind 与有限截止时间等待。
Filesystem 全部 60 个、Project 全部 46 个库测试通过。Application 的 `project::` 过滤实际运行 14 个
项目生命周期、错误和命令适配用例，全部通过；该检查不替代真实平台监听或桌面操作验收。
Graph Analysis 31 个、Editor 13 个、Runtime 最终 13 个通过，Runtime 原有手工 timing probe 1 个忽略。
本批次共 177 个不同 Rust 用例通过，不重复累计中途聚焦检查，也不把忽略项计作通过。

Application 初次消费者构建被并行目录改动中缺失的 8 个心理测量说明文件阻断，文件出现后重跑成功；
没有删除节点声明或生成临时占位说明。Graph Runtime 初次运行的失败来自单次插补节点的旧空端口预期，
现按已注册的真实接口修正，保留未安装内核时不可用的检查；重新执行全部 Runtime 库测试通过。
本批次未重跑完整 Application 库和目录可用性门槛，不据此消除之前记录的全库剩余问题。
四个修改 crate 的格式检查通过；本批次没有新增 UI 测试、benchmark 或规则例外。
Filesystem、Graph Analysis、Graph Runtime 和 Project 的 Clippy 均通过，本次输出没有 warning。
三份修改文档的格式检查、6 个文档契约用例和差异检查通过；Git 只提示工作树部分文件的 CRLF 转 LF。
未运行完整 CI、桌面操作、真实平台监听压力测试、原生插件或其他操作系统验收。

Graph 拓扑、类型职责与 Schema 索引批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:rs:package -p yss-graph-analysis --lib missing_
pnpm test:rs:package -p yss-graph-analysis --lib
pnpm test:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime -p yss-graph-execution --lib
pnpm test:rs:package -p yss-application --lib graph::run::
pnpm test:rs:package -p yss-application --lib graph::results::
pnpm lint:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime -p yss-graph-execution --lib --tests
pnpm format:rs:package -p yss-graph-analysis -- --check
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src-tauri/crates/yss-graph-analysis/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

新增两个端点回归先在原拓扑实现中失败；修复和索引复用后，Analysis 33、Editor 13、Runtime 13、
Execution 30 个库测试通过。Application 的运行 3 个、结果读取 16 个用例在最终代码上通过，
共 108 个不同 Rust 用例；Runtime 原有手工 timing probe 1 个忽略，不计入通过数。
四个 Graph crate 的 Clippy 通过，其自身没有 warning；依赖 `yss-sci::hypothesis` 报告 6 条
并行改动警告，涉及取模、复杂返回类型、显式计数、冗余 return 和参数个数，未按本次改动处理。

聚焦测试中途曾被并行目录改动中缺失的 6 个质量分析说明文件阻断；文件出现后实际执行并复现回归，
没有删除声明或填入占位文档。最终格式、6 个文档契约用例和差异检查通过；未新增 UI 单元测试、
benchmark 或规则例外。未运行完整 Application 库、完整 CI、桌面交互和其他操作系统验收。
本批次只完成上述求解边界，不据此核销全项目覆盖清单。

Graph/Catalog 静态声明、动态引用和函数依赖入口批次执行以下检查（2026-10-02）：

```powershell
pnpm check:rs:package -p yss-graph-analysis --lib
pnpm test:rs:package -p yss-node-catalog -p yss-node-registry -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime -p yss-graph-execution --lib
pnpm test:rs:package -p yss-application --lib graph::
pnpm lint:rs:package -p yss-graph-analysis -p yss-node-catalog -p yss-application --lib --tests
pnpm format:rs:package -p yss-graph-analysis -p yss-node-catalog -p yss-application -- --check
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src-tauri/crates/yss-graph-analysis/README.md src-tauri/crates/yss-node-catalog/README.md src-tauri/crates/yss-application/src/graph/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

Analysis 33、Editor 13、Execution 30、Runtime 13、Catalog 14、Registry 4 个库测试通过，共 107 个；
Runtime 原有手工 timing probe 1 个忽略。Application 的 `graph::` 过滤实际覆盖 Graph 和 Automation
Graph 的 49 个用例，其中 48 个通过，目录可用性门槛失败。当前失败列出 31 个未暂缓但不可用节点：
数据标签 1 个、DoE 2 个、功效/样本量 16 个、复杂抽样 7 个、路径及中介/调节 5 个。
这取代先前 58 个缺口作为本批次观察值，不代表未来工作树中的最终数量；没有修改暂缓清单、跳过或放宽该测试。
本批次合计 155 个不同 Rust 用例通过、1 个失败、1 个既有忽略，不能记作全部消费者检查通过。

初次库测试构建被并行 DoE 目录引用的 6 个缺失说明文件阻断，文件出现后再实际执行。
三个修改 crate 的 Clippy 通过，自身没有 warning；依赖 SCI 有原假设检验 6 条及 DoE 循环 1 条警告。
三个 crate 的格式、四份文档格式、6 个文档契约用例和差异检查通过。
没有新增同构重构测试、UI 单元测试、benchmark 或规则例外；未运行完整 CI、完整 Application 库或桌面验收。
前端 Settings、命令与节点目录只进行了后续审查入口定位，未据此宣称其所有模块和交互已经复核完成。

前端 Settings 与目录缓存批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:ts src/features/core/nodeCatalog/nodeCatalogStore.test.ts --reporter=verbose
pnpm test:ts src/features/core/settings/settingsStore.test.ts --reporter=verbose
pnpm test:ts src/features/core/settings/settingsStore.test.ts src/shared/types/settings/parseSettings.test.ts src/features/core/nodeCatalog/nodeCatalogStore.test.ts src/features/core/nodeCatalog/localizedSearchIndex.test.ts src/features/domain/nodeCatalog/searchDocument.test.ts src/features/domain/nodeCatalog/localizedCatalogTree.test.ts src/services/nodeSystem/catalogService.test.ts src/services/nodeSystem/catalogSearchWireGolden.test.ts src/features/application/nodeCatalog/createNodeFromDescriptor.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts --reporter=verbose
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src/features/core/settings/settingsStore.ts src/features/core/settings/settingsStore.test.ts src/features/core/nodeCatalog/nodeCatalogStore.ts src/features/core/nodeCatalog/nodeCatalogStore.test.ts src/features/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

目录的既有四维身份用例同步核对缓存归属，其中 Registry 指纹和资源版本两项先复现旧响应残留；
修复后仍保留不同项目、语言的独立响应和已持有的旧快照。新增两个 Settings 状态回归也先失败再通过，
重载观察到的通知由两次变为一次，同值重载/更新不额外通知；远端合并保留本地待保存字段和未变分支，
随后保存只发布本地补丁。没有修改 UI 测试或用测试替代界面人工验收。

最终 11 份状态、领域、服务及项目发布消费者测试共 65 个用例通过；另有 6 个文档契约用例通过。
TypeScript 检查通过。前端 lint 退出成功，未修改文件仍有 5 条警告，涉及事件/日志监听器的展开操作和
两个现有 UI 测试的 this 别名；本次修改文件没有 lint 警告。四份源码/测试与两份文档的格式及差异检查通过。
本批次没有新增 benchmark 或规则例外，未运行完整 CI、Rust 测试及桌面交互；此前的 Rust 目录可用性缺口未据此核销。

前端项目事件类型与恢复排空批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:ts src/features/application/project/projectEventIngress.test.ts --reporter=verbose
pnpm test:ts src/features/application/project/projectEventIngress.test.ts src/features/application/project/projectEventConsumer.test.ts src/services/project/projectEventStream.test.ts src/services/project/projectEventParser.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/features/application/initialization/useProjectSync.test.tsx src/features/application/project/projectHydration.test.ts src/features/application/project/closeProject.test.ts src/features/application/projectLifecycleReceipt.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts --reporter=verbose
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src/features/application/project/projectEventConsumer.ts src/features/application/project/projectEventIngress.ts src/features/application/project/projectEventIngress.test.ts src/features/application/project/index.ts src/features/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

新增两个纯 Application 生命周期用例：溢出与消费者恢复重叠的用例先失败，权威快照从未开始；
修复后只请求一次恢复，且排空等待其结束。另一个用例保护关闭之后消费者才拒绝的现有语义，
确保恢复串行化后不会提前交付排空。原有 FIFO、溢出丢弃尾部与恢复后重新接纳测试继续通过。
最终 11 份服务、编排、回执和消费者测试共 76 个用例通过；现有挂载生命周期测试只运行，没有新增或修改 UI 测试。
TypeScript 检查通过；前端 lint 退出成功，未修改文件仍有前述 5 条警告，本次修改文件没有警告。
6 个文档契约用例、四份源码/测试和两份文档的格式检查通过；差异检查通过，仅有工作树既有文件的 CRLF 转 LF 提示。
本批次没有新增 benchmark 或规则例外，未运行完整 CI、Rust 测试或桌面交互；此前 Rust 目录可用性缺口未据此核销。

Workbench 提交投影与 Logs 快照发布批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:ts src/modules/workbench/internal/layout/workbenchNotifications.test.ts src/modules/workbench/internal/layout/workbenchActivation.test.ts src/modules/workbench/internal/layout/workbenchFloatingLayout.test.ts --reporter=verbose
pnpm test:ts src/modules/workbench/internal/layout/workbenchNotifications.test.ts src/modules/workbench/internal/layout/workbenchActivation.test.ts src/modules/workbench/internal/layout/workbenchFloatingLayout.test.ts src/modules/workbench/internal/layout/workbenchPanelModel.test.ts src/modules/workbench/internal/layout/editorPaneStateStore.test.ts src/modules/workbench/internal/application/workbenchLayoutActions.test.ts src/features/application/editor/workbenchPanelClose.test.ts src/features/application/editor/editorPanelCloseCommands.test.ts src/features/application/editor/editorPanelActivation.layout.test.ts src/features/application/editor/editorCommandFocus.test.ts src/features/application/editor/pruneEditorPanels.test.ts src/features/application/editor/synchronizeVisibleGraphPanel.test.ts src/features/application/results/resultLeases.test.ts src/features/application/project/projectWorkbenchLifecycle.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts --reporter=verbose
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src/modules/workbench/internal/layout/workbenchLayoutProjection.ts src/modules/workbench/internal/layout/workbenchLayoutInternal.ts src/modules/workbench/internal/layout/logsRuntime.ts src/modules/workbench/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

原有通知边界测试继续验证纯几何提交只通知持久化、面板字段与选择按实际作用域通知、
模型替换不重复广播相同语义、解除绑定交付未就绪状态；浮窗中间手势继续推进 mutation revision
而不发布业务通知。未新建同构重构测试，也未新增或修改 UI 单元测试。
最终 16 份相关状态及应用消费者测试共 81 个用例通过；TypeScript 检查通过。
首次类型检查发现 Zustand 初始化函数推断过窄，显式标注已有状态契约后通过；不把该失败算作通过记录。
前端 lint 退出成功，仍有 4 条未修改文件警告；旧 Logs 快照监听器的展开警告随实现替换消除。
6 个文档契约用例、三份源码和两份文档的格式检查及差异检查通过；Git 仅提示工作树部分既有文件的 CRLF 转 LF。
未运行完整 CI、Rust 测试或真实桌面交互；保留原生拖放、重置、关闭取消、持久化和窗口生命周期人工验收。
本批次不据此宣称全 Workbench 或全项目合规，也不核销此前 Rust 目录可用性失败。

Workbench 恢复职责与持久化封套批次执行以下检查（2026-10-02）：

```powershell
pnpm test:ts src/modules/workbench/internal/layout/workbenchLayoutPersistence.test.ts --reporter=verbose
pnpm test:ts src/modules/workbench/internal/layout/workbenchLayoutPersistence.test.ts src/modules/workbench/internal/layout/workbenchNotifications.test.ts src/modules/workbench/internal/layout/workbenchActivation.test.ts src/modules/workbench/internal/layout/workbenchFloatingLayout.test.ts src/modules/workbench/internal/layout/workbenchPanelModel.test.ts src/modules/workbench/internal/layout/editorPaneStateStore.test.ts src/modules/workbench/internal/application/workbenchLayoutActions.test.ts src/features/application/editor/workbenchPanelClose.test.ts src/features/application/editor/editorPanelCloseCommands.test.ts src/features/application/editor/editorPanelActivation.layout.test.ts src/features/application/editor/editorCommandFocus.test.ts src/features/application/editor/pruneEditorPanels.test.ts src/features/application/editor/synchronizeVisibleGraphPanel.test.ts src/features/application/results/resultLeases.test.ts src/features/application/project/projectWorkbenchLifecycle.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts --reporter=verbose
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src/modules/workbench/internal/application/workbenchLayoutController.ts src/modules/workbench/internal/application/workbenchLayoutActions.ts src/modules/workbench/internal/layout/workbenchLayoutDefaults.ts src/modules/workbench/internal/layout/workbenchLayoutOperations.ts src/modules/workbench/internal/layout/workbenchLayoutPersistence.ts src/modules/workbench/internal/layout/workbenchLayoutPersistence.test.ts src/modules/workbench/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

新增一个只读取 JSON 对象的存储契约用例，先复现额外封套字段被接纳，再验证拒绝未知字段及两部分独立恢复。
没有新增组件、渲染或交互单元测试；默认安装和恢复继续使用原有激活、浮窗、重置及应用消费者测试。
最终 17 份相关测试共 82 个用例通过；TypeScript 检查通过。移动后遗留的未使用 import 在首次类型检查
发现并移除，最终检查通过；前端 lint 仍有 4 条未修改文件中的既有警告。
6 个文档契约用例、六份源码/契约测试和两份文档的格式及差异检查通过；Git 仅提示工作树部分既有文件的 CRLF 转 LF。
本批次没有新增 benchmark 或规则例外；未运行完整 CI、Rust 测试或真实桌面验收，未核销此前 Rust 目录可用性失败。

平台窗口恢复与挂载生命周期批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:ts src/services/platform/mainWindowGeometry.test.ts --reporter=verbose
pnpm test:ts src/services/platform/mainWindowGeometry.test.ts src/features/application/window/useCurrentWindowActions.test.tsx --reporter=verbose
pnpm test:ts src/services/platform/mainWindowGeometry.test.ts src/services/platform/webviewWindow.test.ts src/features/application/window/createPersistedWindow.test.ts src/features/application/window/openWindowHelpers.test.ts src/features/application/window/windowDecorationPolicy.test.ts src/features/application/window/useCurrentWindowActions.test.tsx src/features/application/presentation/usePresentationWindow.test.tsx src/features/application/presentation/loadPresentationWindow.test.ts src/features/application/presentation/parsePresentationWindowQuery.test.ts src/features/application/results/resultLeases.test.ts src/features/application/project/projectHydration.test.ts src/modules/database-editor/internal/ui/DatabaseEditorWindow.databaseIdentity.test.tsx --reporter=verbose
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src/services/platform/mainWindowGeometry.ts src/services/platform/mainWindowGeometry.test.ts src/features/application/window/useCurrentWindowActions.ts src/features/application/presentation/usePresentationWindow.ts src/modules/database-editor/internal/ui/DatabaseEditorWindow.tsx src/modules/logs/internal/ui/LogWindow.tsx src/modules/workbench/README.md src/features/application/results/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

新增两个纯平台服务生命周期回归，分别先复现监听器未释放，以及恢复失败后的移动事件覆盖已保存的位置。
修复后部分订阅被撤销，失败期间不采样任一页面，同页重试成功且保留两页独立偏好。
原生窗口继续拥有实际几何，模块只管理串行操作、事件订阅和偏好缓存；没有新增窗口模型、Store 或通用调度层。

窗口动作的异步任务改为捕获每次挂载身份，避免 StrictMode 重新挂载后旧任务重新获得有效资格。
结果、数据库及 Logs 的 effect 复用已有稳定动作，最大化状态或错误变化不重新初始化、接管租约或显示窗口。
独立结果加载在接管租约及设置标题后重验取消状态。以上 UI 行为通过调用链审查与既有测试检查，
没有新增或修改 UI 单元测试；真实窗口、StrictMode 和开发期热替换的原生行为仍待人工验收。

最终 12 份相关测试共 26 个用例通过；TypeScript 检查通过，前端 lint 退出成功，仍有 4 条未修改文件的既有警告。
6 个文档契约用例、六份源码/平台测试和三份文档的格式及差异检查通过；Git 仅提示部分既有文件的 CRLF 转 LF。
本批次没有新增 benchmark 或规则例外；未运行完整 CI、Rust 测试、应用构建或真实桌面验收，未核销此前 Rust 目录可用性失败。

项目发布元数据候选批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:ts src/features/application/editorMutation/projectSnapshotMetadata.test.ts --reporter=verbose
pnpm test:ts src/features/application/editorMutation/projectSnapshotMetadata.test.ts src/features/application/editorMutation/projectSnapshotResources.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/dataManagement/databaseRecords.test.ts src/features/application/project/projectHydration.test.ts src/features/application/project/projectLifecycleOperations.test.tsx src/features/core/resource/functionResourceView.test.ts src/features/core/dataStore/graphProjectionStore.test.ts src/services/project/projectService.test.ts src/modules/database-editor/internal/ui/DatabaseEditorWindow.databaseIdentity.test.tsx --reporter=verbose
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src/features/application/dataManagement/databaseRecords.ts src/features/core/dataStore/graphMeta.ts src/features/application/project/authoritativeProjectLoadPlan.ts src/features/application/editorMutation/projectPublicationSnapshot.ts src/features/application/editorMutation/projectSnapshotMetadata.test.ts src/features/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

新增两个纯候选测试分别复现重复快照重建数据库表和图元数据表，修复后复用未变映射及条目。
同一用例继续验证删除和修订更新保留运行时列信息，函数参数修改保留输出及未变类型引用，
修改外部输入不能改写已准备的签名候选；候选本身不安装 Store。首次加载和索引发布共用原 GraphMetaStore
内的准备函数，数据库转换归回原 databaseRecords，未新增比较器、数据模型或发布入口。
图列表在本次候选内只物化一次，按路径查询刷新资源类型，取消原来每个会话都扫描完整图列表的路径。

最终 13 份相关测试共 68 个用例通过；已有的加载、发布及函数消费者覆盖迟到项目快照、删除授权、
编辑期间的候选重建和函数权威更新。没有新增或修改 UI 单元测试。首次类型检查发现只读语义列与 Immer draft
类型不匹配及测试映射推断过窄，使用现有 castDraft 入口并补齐映射类型后，最终 TypeScript 检查通过。
前端 lint 退出成功，仍有 4 条未修改文件警告；6 个文档契约用例、五份源码/候选测试和两份文档的格式与差异检查通过。
本批次没有新增 benchmark 或规则例外，不声称已测得耗时改善；未运行完整 CI、Rust 测试、应用构建或真实桌面验收，
此前 Rust 目录可用性失败未据此核销，完整跨 owner 发布和全项目复核仍未完成。

资源索引与文档状态合并批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:ts src/features/core/resource/resourceStore.test.ts --reporter=verbose
pnpm test:ts src/features/core/resource/resourceStore.test.ts src/features/core/resource/documentStateQueries.test.ts src/features/core/resource/resourceSnapshotProjection.test.ts src/features/core/state/readonlySnapshot.test.ts --reporter=verbose
pnpm test:ts src/modules/graph-editor/internal/ui/Pins/Pin.preview.test.tsx src/features/core/chart/chartDocumentStore.test.ts src/features/core/dataStore/graphDocumentLoadPolicy.test.ts src/features/core/resource/resourceStore.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/core/resource/resourceSnapshotProjection.test.ts src/features/core/resource/documentStateQueries.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/chart/chartViewActions.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editor/editorPanelDirty.test.ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/resource/docActions.test.ts src/features/application/resource/createFileActions.test.ts src/features/application/project/projectIOStore.test.ts src/features/application/resource/fileTextInput.test.ts src/features/application/editor/pruneEditorPanels.test.ts src/features/application/editorMutation/projectSnapshotMetadata.test.ts src/features/application/editorMutation/projectSnapshotResources.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/project/projectHydration.test.ts src/features/application/project/projectLifecycleOperations.test.tsx src/features/application/project/closeProject.test.ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/editor/editorPanelCloseCommands.test.ts src/features/application/editor/useProjectOperations.saveActiveFile.test.tsx src/features/core/dataStore/projectedEditorCapabilities.test.ts --reporter=dot
pnpm test:ts src/features/application/project/projectHydration.test.ts src/features/application/project/projectLifecycleOperations.test.tsx --reporter=verbose
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src/modules/graph-editor/internal/ui/Pins/Pin.preview.test.tsx src/features/core/chart/chartDocumentStore.test.ts src/features/core/dataStore/graphDocumentLoadPolicy.test.ts src/features/core/resource/resourceStore.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/core/resource/resourceSnapshotProjection.ts src/features/core/resource/resourceSnapshotProjection.test.ts src/features/core/resource/read.ts src/features/core/resource/index.ts src/features/core/resource/documentStateQueries.ts src/features/core/resource/documentStateQueries.test.ts src/features/core/resource/documentStateActions.ts src/features/application/editorMutation/projectPublicationSnapshot.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/chart/chartViewActions.test.ts src/features/application/editorMutation/projectPublicationCoordinator.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editor/editorPanelDirty.test.ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/resource/docActions.test.ts src/features/application/resource/createFileActions.test.ts src/features/application/project/projectIOStore.test.ts src/features/application/project/projectHydration.ts src/features/application/project/projectReset.ts src/features/application/resource/fileTextInput.test.ts src/features/core/resource/resourceStore.ts src/features/application/editor/pruneEditorPanels.test.ts src/features/README.md src/features/application/project/projectHydration.test.ts src/features/application/project/projectLifecycleOperations.test.tsx docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

新增两个纯状态用例，分别先复现文档 dirty 已改变而资源摘要尚未更新，以及同值完整快照重复发布。
修复后原 ResourceStore 用一次 Immer 更新文档及摘要，删除独立文档 Store；完整安装、恢复失败标记和重置
都通过同一 owner。用例也验证恢复文档与资源同时可见、重复 stale 标记不通知、仅修订改变保留读取引用，
以及重命名、删除和排序保留未变条目与旧快照。资源普通补丁不再允许独立改写文档摘要。
所有旧 Store 调用方直接迁移，无兼容 facade；实际资源消费者按 ID 查找，排序仍由 graphOrder 明确保存。

L1 的 4 份测试共 9 个用例通过。L2 首次 28 份测试中 93 个通过、2 个失败，原因是旧测试依赖未变化的
空资源快照仍发布通知；加载测试改为断言同值不发布且 Store 引用不变，项目切换用例改为安装实际新增的图，
继续验证清理先于新资源发布。修正后这两份测试的 16 个用例通过，其余未修改用例复用本批次通过结果，
最终覆盖 28 份测试中的 95 个相关用例。现有 Pin、项目生命周期等测试只适配 Store 接口和上述测试输入，
没有新增组件、渲染或交互测试。

最终 TypeScript 检查通过；首次检查发现目标库不包含 Object.hasOwn，改用已有目标支持的自有属性检查。
前端 lint 退出成功，仍有 4 条未修改文件警告；6 个文档契约用例、31 份本批次源码/测试/文档的格式检查
及差异检查通过。Git 仅提示部分既有文件的 CRLF 转 LF。
本批次没有新增 benchmark 或规则例外，不声称已测得耗时改善；未运行完整 CI、Rust 测试、应用构建
或真实桌面验收，未据此核销此前 Rust 目录可用性失败，完整跨 owner 发布和全项目复核仍未完成。

项目发布水位、恢复与回执身份批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:ts src/features/application/editorMutation/projectPublicationIntegration.test.ts --reporter=verbose
pnpm test:ts src/features/application/editorMutation/projectPublicationIntegration.test.ts -t 'retains receipt conflict detection' --reporter=verbose
pnpm test:ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/core/resource/resourceStore.test.ts --reporter=verbose
pnpm test:ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/core/resource/resourceStore.test.ts src/features/core/nodeCatalog/nodeCatalogStore.test.ts src/features/application/nodeCatalog/useCompatibleNodeCatalog.test.tsx src/features/application/nodeCatalog/useLocalizedNodeCatalog.test.tsx src/features/application/project/projectHydration.test.ts src/features/application/project/projectLifecycleOperations.test.tsx src/features/application/project/projectEventConsumer.test.ts src/features/application/project/projectEventIngress.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/project/projectIOStore.test.ts src/features/application/project/closeProject.test.ts --reporter=dot
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src/features/core/resource/resourceStore.ts src/features/application/editorMutation/projectPublicationCoordinator.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

新增三个纯应用层回归，三个不同失败均在生产修复前实际复现。前两个分别发现空回执和同内容索引
未推进连接候选订阅的资源版本，以及 incomplete 恢复在索引内容不变时从未准备干净图。
第三个用例补充独立协议风险：同版本 watcher 刷新丢弃已知指纹，冲突回执被错误接纳为 duplicate；
前两个用例不涉及指纹，因此不能覆盖这一回归。聚焦第三项时的 7 项过滤不算通过或减少覆盖，最终 L2 全部运行。

ResourceStore 用现有 owner 的简单标量更新推进版本，未新增状态镜像或重建完整资源表；
相同版本不通知，资源、文档与排序的读取引用不变。恢复仍复用原图会话准备、候选和提交入口，
只刷新已加载的干净图，dirty 图继续保留，恢复完成后的重复 watcher 不再次加载图。
同版本刷新保留当前已知指纹，前进到新版本时不会误用旧指纹拒绝合法的迟到回执。
项目、epoch、候选提交与图保存状态的既有约束保留；没有新增 UI 单元测试。

最终 L1 的 2 份测试共 10 个用例及 L2 的 15 份测试共 78 个用例通过；TypeScript 检查通过，
前端 lint 退出成功，仍有 4 条未修改文件中的既有警告。6 个文档契约用例、本批次五份源码/测试/文档
的格式检查及差异检查通过；Git 仅提示部分既有文件的 CRLF 转 LF。
本批次没有新增 benchmark 或规则例外；未运行完整 CI、Rust 测试、应用构建或真实桌面验收。
图/文件资源回执的剩余分次发布已记录为下一审查入口，本批次不声称完整跨 owner 发布或全项目复核完成，
也不据此核销此前 Rust 目录可用性失败。

Graph 与 Mind/Doc 回执的资源发布批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/resource/createFileActions.test.ts --reporter=verbose
pnpm test:ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/resource/createFileActions.test.ts src/features/core/resource/resourceStore.test.ts --reporter=verbose
pnpm test:ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/resource/createFileActions.test.ts src/features/core/resource/resourceStore.test.ts src/features/core/resource/documentStateQueries.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/core/resource/resourceSnapshotProjection.test.ts src/features/application/resource/docActions.test.ts src/features/application/resource/mindActions.test.ts src/features/application/resource/fileTextInput.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/core/dataStore/graphDocumentLoadPolicy.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/graphEditing/graphConstantActions.test.ts src/features/application/graphEditing/editorCommands.test.ts src/features/application/editor/editorPanelDirty.test.ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/resource/resourceActions.test.ts src/features/application/resource/fileManagement.test.ts src/features/application/editor/useProjectOperations.saveActiveFile.test.tsx src/features/core/chart/chartDocumentStore.test.ts src/features/application/chart/chartViewActions.test.ts --reporter=dot
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src/features/core/resource/resourceStore.ts src/features/core/resource/documentStateActions.ts src/features/application/graphEditing/graphEditCoordinator.ts src/features/application/graphEditing/saveGraph.ts src/features/application/graphProjection/graphProjectionLifecycle.ts src/features/application/resource/createFileActions.ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/resource/createFileActions.test.ts src/features/README.md src/features/application/resource/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

新增两个纯状态/应用用例，均先实际失败再通过：图编辑回执暴露三次资源状态，图保存先公布编辑修订、
再公布 Rust 的资源修订；文件读取先改 loaded/missing，再改 dirty，最后才改修订。
修复后复用原 ResourceStore 的文档写入方法，在一次 Immer 中同步相关摘要与可选资源修订。
Core 的回执动作共用原 dirty 规则；Graph 安装的保存分支必须提供资源类型和资源修订，删除保存末尾
重复的资源修订及 dirty 写入。Graph 载入、刷新、编辑、历史和保存共用原会话安装入口。

回归确认每份回执只发布最终资源状态，旧状态保持不变；保存资源修订与编辑会话版本可以不同。
文件回执保留新输入造成的 dirty、已有冲突/过时标记及 missing 状态，不能通过载入恢复索引存在性。
既有回归继续检查编辑与刷新排队、保存失败释放、无效投影拒绝、重命名输入迁移、文件丢弃和项目发布。
本批次的原子范围是 ResourceStore 内的相关状态；Graph/文件正文仍有各自 owner，跨 owner 边界继续复核。

最终 L1 的 3 份测试共 10 个用例及 L2 的 23 份测试共 96 个用例通过，没有新增或修改 UI 单元测试。
首次类型检查发现被移除私有辅助函数遗留的未使用类型 import，删除后 TypeScript 检查通过。
前端 lint 退出成功，仍有 4 条未修改文件中的既有警告；6 个文档契约用例、本批次 11 份源码/测试/文档的
格式检查及差异检查通过，Git 仅提示部分既有文件的 CRLF 转 LF。
本批次没有新增 benchmark、通用批处理框架、平行 Store 或规则例外；未运行完整 CI、Rust 测试、
应用构建或真实桌面验收，未据此核销此前 Rust 目录可用性失败或宣称全项目合规。

图表正文、草稿和资源状态合并批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:ts src/features/core/chart/chartDocumentStore.test.ts --reporter=verbose
pnpm test:ts src/features/core/chart/chartDocumentStore.test.ts src/features/application/chart/chartViewActions.test.ts src/features/core/resource/resourceStore.test.ts src/features/core/resource/documentStateQueries.test.ts src/features/core/resource/resourceSnapshotProjection.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectSnapshotResources.test.ts src/features/application/editorMutation/projectSnapshotMetadata.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/project/projectHydration.test.ts src/features/application/project/projectLifecycleOperations.test.tsx src/features/application/project/projectIOStore.test.ts src/features/application/project/closeProject.test.ts src/features/application/editor/workbenchPanelClose.test.ts src/features/application/editor/editorPanelCloseCommands.test.ts src/features/application/editor/editorPanelDirty.test.ts src/features/application/editor/pruneEditorPanels.test.ts src/features/application/editor/useProjectOperations.saveActiveFile.test.tsx src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/resource/createFileActions.test.ts src/modules/details/internal/ui/useDetailPanelModel.test.tsx --reporter=dot
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src/features/core/resource/resourceStore.ts src/features/core/resource/documentStateActions.ts src/features/core/chart/read.ts src/features/core/chart/ui.ts src/features/core/chart/chartDocumentStore.test.ts src/features/application/chart/chartViewActions.ts src/features/application/chart/chartViewActions.test.ts src/features/application/chart/saveChartDocument.ts src/features/application/editor/workbenchPanelClose.ts src/features/application/editor/workbenchPanelClose.test.ts src/features/application/editorMutation/projectPublicationCoordinator.ts src/features/application/editorMutation/projectPublicationSnapshot.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/project/projectHydration.ts src/features/application/project/projectReset.ts src/modules/details/internal/ui/useDetailPanelModel.test.tsx src/features/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

新增两个纯状态/应用用例，生产修复前均实际失败。第一个发现载入先发布 loaded、正文尚未安装，
编辑和保存结算也暴露正文与 dirty 不一致的状态；第二个发现同值安装及无变化编辑重复通知、误标 dirty，
并重建未变化的 encodings。图表正文和读取令牌现移至原 ResourceStore，删除独立图表 Store，
原 `core/chart` 读取与编辑入口继续复用通用只读投影和结构共享，未新增平行模型或兼容 facade。

载入、编辑、关闭、保存结算和项目快照各在同一 Immer 中发布正文、文档标记及资源摘要；
保存仍由 Application 校验项目、操作和回执身份，Store 仅在资源修订匹配时结算，保留保存期间的新编辑。
同值安装和无变化编辑不通知、不改变 clean 状态，实际编辑继续共享未变分支并保护旧快照。
初始读取的关闭/重新打开、项目切换和迟到响应保护保持原行为。

补充完整快照安装断言后，第一次 L2 出现 6 个保存用例失败：刚安装进 Immer 的已冻结候选不是 draft，
标记归一化直接修改它会抛错。改为对标量文档记录和摘要浅比较后替换，保留一次事务与未变引用；
最终图表 L1 的 8 个用例及 L2 的 24 份测试共 100 个用例全部通过。既有 Details 和面板关闭测试
仅迁移状态 owner、初始化和 mock 接口，没有新增 UI 单元测试或 UI 验收断言。

TypeScript 检查通过，前端 lint 退出成功，仍有 4 条未修改文件中的既有警告。
6 个文档契约用例、18 份本批次源码/测试/文档的格式检查及差异检查通过；
Git 仅提示部分既有文件的 CRLF 转 LF。
本批次没有新增 benchmark 或规则例外，不声称已测得耗时改善；未运行完整 CI、Rust 测试、
应用构建或真实桌面验收。Graph/文件正文跨 owner 读取和全模块复核仍未完成，
未据此核销此前 Rust 目录可用性失败。

Mind/Doc 正文、引用共享与资源发布批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:ts src/features/application/resource/createFileActions.test.ts src/features/core/resource/fileSnapshots.test.ts --reporter=verbose
pnpm test:ts src/features/application/resource/docActions.test.ts --reporter=verbose
pnpm test:ts src/features/application/resource/createFileActions.test.ts src/features/core/resource/fileSnapshots.test.ts src/features/application/resource/docActions.test.ts src/features/application/resource/mindActions.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editorMutation/projectSnapshotResources.test.ts --reporter=verbose
pnpm test:ts src/features/core/chart/chartDocumentStore.test.ts src/features/application/chart/chartViewActions.test.ts src/features/core/resource/resourceStore.test.ts src/features/core/resource/documentStateQueries.test.ts src/features/core/resource/resourceSnapshotProjection.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectSnapshotResources.test.ts src/features/application/editorMutation/projectSnapshotMetadata.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/project/projectHydration.test.ts src/features/application/project/projectLifecycleOperations.test.tsx src/features/application/project/projectIOStore.test.ts src/features/application/project/closeProject.test.ts src/features/application/editor/workbenchPanelClose.test.ts src/features/application/editor/editorPanelCloseCommands.test.ts src/features/application/editor/editorPanelDirty.test.ts src/features/application/editor/pruneEditorPanels.test.ts src/features/application/editor/useProjectOperations.saveActiveFile.test.tsx src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/resource/createFileActions.test.ts src/modules/details/internal/ui/useDetailPanelModel.test.tsx src/features/core/resource/fileSnapshots.test.ts src/features/application/resource/docActions.test.ts src/features/application/resource/mindActions.test.ts src/features/application/resource/fileTextInput.test.ts src/features/application/resource/resourceActions.test.ts src/features/application/resource/fileManagement.test.ts src/features/application/project/projectWorkbenchLifecycle.test.ts src/features/application/editor/openFileInEditor.test.ts --reporter=dot
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src/features/core/resource/resourceStore.ts src/features/core/resource/documentStateActions.ts src/features/core/resource/fileSnapshots.test.ts src/features/application/resource/createFileActions.ts src/features/application/resource/createFileActions.test.ts src/features/application/resource/docActions.ts src/features/application/resource/docActions.test.ts src/features/application/resource/mindActions.ts src/features/application/resource/mindActions.test.ts src/modules/document-editor/internal/FileEditor.tsx src/modules/document-editor/internal/DocEditor.tsx src/modules/document-editor/internal/MindEditor.tsx src/modules/details/internal/ui/panels/MindDetailPanel.tsx src/features/application/project/projectHydration.ts src/features/application/project/projectReset.ts src/features/application/editorMutation/projectSnapshotResources.ts src/features/application/editorMutation/projectPublicationCoordinator.ts src/features/application/editorMutation/projectPublicationSnapshot.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/tests/helpers/projectSnapshotFixtures.ts src/features/README.md src/features/application/resource/README.md src/modules/document-editor/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

新增三个纯状态/应用回归，生产修复前均实际失败。第一项发现文件载入先暴露正文、资源标记仍旧，
释放先清标记但正文尚在；第二项发现同值安装与删除不存在路径重复通知，变化快照也重建未变 Mind 节点。
第三项针对独立的首次读取窗口：资源已从索引移除，较早响应仍被接纳并装回正文；前两项从已返回的快照
出发，无法覆盖这一准入问题，增加前已说明其独立必要性。既有重命名用例补充发布观察后也实际失败，
发现源路径和目标路径分两次更新；合并后仅通知一次，仍保留并提交重命名期间的新输入。

Mind/Doc 的带类型快照及读取令牌并入原 ResourceStore，删除原两个投影 Store 和通用 Store 工厂，
直接迁移所有消费者，不保留兼容入口。Rust Project 仍拥有当前文档，Application 继续负责命令队列、
IPC/项目身份检查和输入缓冲；FileEditor 与 Mind Details 按类型和路径选择快照。
安装、释放和重命名共用一次 Immer 发布正文、文档标记和资源修订；复用原 dirty 规则及 shareProjection，
同值单项/完整快照安装不通知，未变节点和旧快照保持引用与内容。旧版本不能覆盖同一会话的新版本。
读取令牌同时检查资源修订和存在性，关闭、项目切换、新响应及索引变化继续阻止迟到响应装回正文。

项目候选同时裁剪文件快照，保留外部缺失文件的 dirty 内容与未提交输入，显式删除一次清除正文和资源。
重命名先转移输入身份，再在项目提交中移除旧路径；未持有正文的目标不假报 loaded。
项目加载和重置只通过原 ResourceStore 清空这些状态。候选 benchmark 的公共夹具补齐空文件快照字段，
本批次未重跑该 benchmark，也没有把原有图表候选测量解释为本次 Mind/Doc 的性能结果。

最终 L1 的 6 份测试共 15 个用例通过；完整快照同值断言补入第二项后，该纯状态用例也单独通过。
L2 的 32 份测试共 123 个用例通过，覆盖文件输入、保存/丢弃、项目发布与切换、图表、图编辑及关闭消费者。
没有新增 UI 单元测试。类型检查先发现旧 import 清理和候选夹具遗漏，随后发现重命名分支需保留
显式 discriminant 缩窄；直接修正后 TypeScript 检查通过。lint 退出成功，仍有 4 条既有文件警告。
6 个文档契约用例、25 份本批次源码/测试/文档的格式检查及差异检查通过；
Git 仅提示部分既有文件的 CRLF 转 LF。

本批次没有新增通用批处理框架、平行 Store、benchmark 或规则例外；未运行完整 CI、Rust 测试、
应用构建或真实桌面验收，未据此核销此前 Rust 目录可用性失败。Graph 会话与其他 owner 的交付边界、
其余模块及全项目复核仍未完成。

Graph 当前 Pin 结果读取批次执行以下 L1/L2 检查（2026-10-02）：

```powershell
pnpm test:ts src/features/application/results/runtime.test.ts --reporter=verbose
pnpm test:ts src/features/application/results/runtime.test.ts -t 'keeps valid pin bindings stable' --reporter=verbose
pnpm test:ts src/features/application/results/runtime.test.ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/results/resultLeases.test.ts src/features/application/results/inspectableResult.test.ts src/features/application/results/pinResultSearch.test.ts src/features/application/results/graphPresentation.test.ts src/features/application/results/addLinearSummaryContents.test.ts src/features/application/results/resultReadErrors.test.tsx src/features/application/results/usePagedResultRows.test.tsx src/features/core/dataStore/graphProjectionStore.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/core/execution/useExecutionStore.lifecycle.test.ts src/features/core/execution/graphRunArtifacts.test.ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/editor/graphPanelSession.test.ts src/features/application/editor/synchronizeVisibleGraphPanel.test.ts src/features/application/editor/useProjectOperations.execution.test.tsx src/features/application/project/projectHydration.test.ts src/features/application/project/projectLifecycleOperations.test.tsx src/features/application/project/closeProject.test.ts src/services/nodeSystem/graphProjectionService.test.ts src/services/nodeSystem/graphEditorSync.test.ts --reporter=dot
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts --reporter=verbose
pnpm format:check:ts src/features/application/results/resultProjection.ts src/features/application/results/runtime.ts src/features/application/results/runtime.test.ts src/features/application/results/README.md src/features/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git diff --check
```

新增两个纯应用回归，均在生产修复前实际复现问题：已加载图的新语义帧发布后，命令式当前 Pin 仍返回
旧 descriptor，结果订阅者也未收到失效通知；仅结果摘要的权威 ResultId 改变时，同样错误保留旧绑定。
第二项以已交付的缓存对象检查引用稳定性，避免把服务边界原有的隔离复制误判为 Store 重建；
按测试名聚焦时未运行的 10 项不算通过，最终 L1/L2 均完整执行该文件。

现有 resultProjection 继续是唯一可写查询缓存。它的读取入口复用 createReadProjection，同时消费图摘要
与查询缓存，当前 Pin 和搜索共享可见绑定表。已加载图按语义身份、执行会话、输出有效性及 ResultId 筛选；
图帧发布即可隐藏失效绑定，不等待清理步骤。命令式读取另从当前源状态检查同一条件，保证同步图监听器
不受只读投影监听顺序影响；未加载图的当前输出查询继续接受 Rust 回执。

接纳、读取及清理复用同一匹配规则。输出索引按已发布、不可变的 outputs 数组用 WeakMap 复用，
原有逐 Pin 查找及每次对账重建索引被替换；selector 不扫描全部输出。无关 payload 更新不重新筛选绑定表，
可见绑定不变时保留引用；仅摘要修订变化不重复通知，清理已隐藏绑定保持当前 Pin 的 selector 值。descriptor、分页、
分析和报告数据继续按完整结果引用及已有租约保留，不通过清空报告来掩盖当前绑定失效。

L1 的 11 个用例通过；L2 的 25 份测试共 106 个用例通过，覆盖结果查询、持有租约、搜索、图投影
与同步、图卸载、执行和项目生命周期。TypeScript 检查通过；lint 退出成功，仍有 4 条既有文件警告。
本批次新增的两项测试均不渲染 UI，没有新增或修改 UI 单元测试。
6 个文档契约用例、6 份本批次源码/测试/文档的格式检查及差异检查通过；
Git 仅提示部分既有文件的 CRLF 转 LF。

本批次没有新增业务 Store、订阅框架、benchmark 或规则例外，不声称已测得耗时改善。
未运行完整 CI、Rust 测试、应用构建或真实桌面验收，也未据此核销此前 Rust 目录可用性失败。
Graph 会话和资源标记仍在不同 owner 中顺序安装，运行展示及其他跨 owner 边界仍需继续核对；
本批次修复当前 Pin 读取不能证明整个 Graph 交付事务或全项目复核已完成。

### Graph 会话与资源状态统一发布（2026-10-02）

本批次处理上一批留下的 Graph/Resource 发布边界。两个新纯应用回归在生产修改前实际失败：
图订阅者已经看到编辑版本 3、dirty=true 时，资源修订仍是 0，文档 loaded/dirty 尚未安装；
图卸载仍在等待 Rust 时，资源已发布 loaded=false。没有新增 UI 单元测试。

ResourceStore 现在同时拥有 Graph 的 sessions、graphEntities、resultStates 与既有资源/文档状态。
`core/dataStore/graphProjection.ts` 保留原校验、结构共享、实体索引和批量候选准备，显式接收基线；
删除独立 GraphProjectionStore，调用方直接使用已有 ResourceStore，命令式只读帮助函数归 core/graph/read。
单图安装在候选校验后用一次 Immer 发布会话与资源标记；保存使用 Rust 回执中的资源修订，同时清除保存锁。
普通保存锁与结果摘要更新保留资源、文档分支引用。无变化安装不通知；旧版本与无效投影不能部分发布。
原单独发布 Graph 资源标记的方法与无人使用的修订参数已移除。

完整项目快照在原提交边界重新准备，并通过 ResourceStore.setSnapshot 一次安装图和资源状态；
保留脏图时从被保留会话归一化 dirty/loaded，移除会话时一并清理文档标记。初次项目安装和重置也不再单独清空图 Store。
已扩展现有项目快照回归：准备后的编辑仍被保留，订阅者只收到图、文档与资源摘要一致的 dirty 状态。

卸载只有得到当前 Rust 成功回执后才删除图与标记。待卸载标记位于既有 Application 生命周期记录，
使卸载中重新打开仍能发起新载入；拒绝、失败、保留条件改变或新请求取代旧请求不会提前删除当前图。
新回归同时检查拒绝卸载后缓存仍可使用；原有首读取消重开和旧卸载回执晚到回归继续通过。
Results 查询取消、保留租约、执行状态和视口释放继续各归原 owner，本批次没有声称它们与资源状态组成一个总事务。

验证实际使用仓库根 pnpm 脚本，并明确指定测试文件：

| 命令                                                                                                                                          | 范围与结果                                                                                                                                                                          |
| --------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm test:ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/editor/graphDocumentUnload.test.ts` | 此聚焦命令先复现两个问题；修复后的对应文件已纳入下述 L2 并通过                                                                                                                      |
| `pnpm test:ts`                                                                                                                                | L2 明确指定 59 份文件，271 个用例全部通过；覆盖直接迁移的 30 份测试，以及 ResourceStore、图表、Mind/Doc、项目快照/切换、关闭、Canvas/键盘现有消费者、Results、执行和 Graph 同步服务 |
| `pnpm check:ts`                                                                                                                               | TypeScript 检查通过                                                                                                                                                                 |
| `pnpm lint:ts`                                                                                                                                | 退出成功；4 条既有警告，位于 globalEvent、projectEventStream、useChartContainerSize.test 和 useLogPanelVirtualList.test                                                             |
| `pnpm test:ts src/tests/documentationContract.test.ts`                                                                                        | 6 个用例通过，包括既有模块图检查                                                                                                                                                    |
| `pnpm bench:graph`                                                                                                                            | 100、1000、5000 节点的 18 个解析/安装样本完成                                                                                                                                       |
| `pnpm bench:graph:publication`                                                                                                                | 100/500 图候选与 1000/5000 输出展示的 4 个样本完成                                                                                                                                  |

消费者迁移期间的类型/导入失败均已修正。一个旧缓存守卫用例原先依赖“安装图不安装 loaded”的夹具行为；
现在显式设置该用例需要的未加载标记，仍检查原有缓存守卫，没有删除用例或放宽断言。
已有 UI 测试仅迁移 owner 导入及必要夹具；这不替代桌面验收。图候选测试随模块改名为 graphProjection.test.ts。

基准沿用既有夹具和计时配置，单独运行于当前 Windows 工作区、Vitest 4.1.10；下表单位为 ms：

| 样本                    |    mean |     p75 |     RME |
| ----------------------- | ------: | ------: | ------: |
| 1000 节点增量解析与安装 |  1.5237 |  1.5300 |  ±0.72% |
| 1000 节点增量安装       |  0.7484 |  0.7498 |  ±0.44% |
| 5000 节点增量解析与安装 |  8.5462 |  8.9002 |  ±1.98% |
| 5000 节点增量安装       |  3.3594 |  3.4317 |  ±0.84% |
| 100 图批量候选          |  9.1584 |  5.6824 | ±36.85% |
| 500 图批量候选          | 43.3270 | 73.5746 | ±19.32% |
| 1000 输出失败展示更新   |  0.0073 |  0.0046 | ±37.55% |
| 5000 输出失败展示更新   |  0.0057 |  0.0046 | ±12.92% |

这些样本没有构造完整项目资源索引；它们测量 Graph 解析/安装、候选准备和既有结果展示路径，
不代表完整 Graph+Resource 项目提交、IPC、React 渲染或浏览器绘制的耗时。部分样本波动明显，
且没有同环境改动前对照，本批次不声称性能提升，也不把旧基准值当作新结果。

本批次 93 份源码、测试与文档的 `pnpm format:check:ts` 通过；`git diff --check` 通过，
Git 仅提示既有 Rust 工作树文件的 CRLF 转 LF。改名后的 graphProjection.test.ts 已实际执行 8 个用例并通过。

本批次没有新增业务 Store、订阅框架、兼容入口、协议转换或规则例外。
尚未运行完整 CI、Rust 测试、应用构建或真实桌面验收；此前 Rust Catalog 可用性失败未据此核销。
运行展示身份、其他派生读取边界和全模块复核仍未完成。

### 运行事件的真实语义身份与失败发布（2026-10-02）

本批次沿上批次的运行展示入口继续检查。生产修改前的聚焦回归实际复现两处问题：
旧语义 RunStarted 被当成当前图运行接纳；RunErrored 先发布 running 状态加错误，再发布 error 终态。
新增一个纯应用测试验证迟到运行的接纳边界，原有终态测试扩展为检查订阅者只收到一次完整失败发布；
没有新增 UI 单元测试，现有 UI 测试只迁移所需的事件夹具。

Rust Application 的 RunIdentity 直接携带 RunGraphRequest 准入已验证的 32 字节 semantic input hash，
公共活动、专属 Channel、执行回执和恢复快照使用同一身份。共享 IPC DTO 增加必填 semanticInputHash，
适配层编码为 64 位小写十六进制，前端严格 parser 复用已有 fingerprint guard 校验。
当前契约直接更新，没有旧事件兼容或由前端补造 hash 的路径。

Results 的输出运行记录和 Execution 失败摘要保留完整事件身份，原先在事件到达时读取画布 hash 的逻辑已删除。
已有 core/graph/read 统一检查已加载图的语义 hash 和执行会话；运行事件接纳、运行/等待摘要展示与失败读取
复用同一条件。新图帧即可隐藏旧依据的失败，不必等待 Application 后续清理；未加载图仍可接收公共运行事实。
结果查看意图沿用现有去重记录及完整结果引用，语义变化不会阻止同一运行的已保留结果查看，也不会重复打开。
清理结果、租约、请求控制与执行状态仍各归原 owner，本批次没有把多个 Store 宣称为一个总事务。

Execution 的 failExecution 在一次状态更新中接纳匹配活动 RunId 的失败并写入 error、清除请求及活动 RunId；
删除分步 recordRunFailure 入口，保留实际使用的 clearRunFailure。已有未知状态恢复也可接纳匹配失败。
本批次没有新增业务 Store、自制订阅框架或规则例外。

实际验证结果：

| 命令                                                                                                                                          | 范围与结果                                                                                                                                                       |
| --------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm test:ts`（显式文件参数）                                                                                                                | L1 3 文件 42 用例通过；L2 25 文件 166 用例通过，覆盖事件安装、Results 查询/租约/报告、Execution、Graph 发布、严格 parser、项目执行/通道及既有 ResultPanel 消费者 |
| `pnpm test:ts src/features/domain/editorProjection/editorProjection.test.ts src/tests/documentationContract.test.ts`                          | 共享 guard 消费者 21 用例、文档契约 6 用例通过；本批次前端行为验证共 26 文件 187 用例                                                                            |
| `pnpm check:ts`                                                                                                                               | 共享类型及全部 TypeScript 消费者检查通过                                                                                                                         |
| `pnpm lint:ts`                                                                                                                                | 退出成功，仍有既有 4 条警告，无新增警告                                                                                                                          |
| `pnpm test:rs:package -p yss-application --test numeric_execution project_dataset_graph_runs_through_application_authority_and_paged_results` | 1 用例通过，真实执行回执与公共/恢复事件身份相同，且保留请求准入的 hash                                                                                           |
| `pnpm test:rs:package -p yss-application --lib run_failure_wire_preserves_the_cause_phase_and_node`                                           | 1 用例通过，完整 RunEventDto 与两端共享 wire fixture 一致，包含必填 hash 和失败原因/阶段/定位                                                                    |
| `pnpm test:rs:package -p yss-application --lib graph::run::tests`                                                                             | 3 用例通过，包含准入取消、会话重验和实际参数执行                                                                                                                 |
| `pnpm test:rs:package -p yss-ipc-contract --lib`                                                                                              | 3 用例通过；Application 编译也覆盖 Event/Channel 对共享类型的消费                                                                                                |
| `pnpm format:rs:package -p yss-application -p yss-ipc-contract -- --check`                                                                    | 两包格式检查通过，没有对其他 Rust 文件进行格式写入                                                                                                               |
| `pnpm bench:graph:publication --outputJson <临时目录>/yssbi-run-basis-publication.json`                                                       | 单独执行，4 个样本完成，结果如下                                                                                                                                 |

基准使用当前 Windows 工作区（HEAD `7c7d843e`，含未提交改动）、Intel Core i9-13900K 及 Vitest 4.1.10。
既有展示样本已改为一次发布 error/idle 和匹配图会话语义身份的
失败摘要，继续消费真实 Execution Store 及 Results 读取投影；不再调用已删除的分步写入方法。
因此它与之前的测量不是相同输入，不能直接计算前后性能收益。单位为 ms：

| 样本                  |    mean |     p75 |     RME |
| --------------------- | ------: | ------: | ------: |
| 100 图批量候选        |  9.6288 |  5.8267 | ±39.52% |
| 500 图批量候选        | 45.1625 | 77.7159 | ±20.39% |
| 1000 输出失败展示更新 |  0.0079 |  0.0047 | ±42.51% |
| 5000 输出失败展示更新 |  0.0062 |  0.0046 | ±17.07% |

本批次 30 份源码、夹具与文档的 `pnpm format:check:ts` 通过，`git diff --check` 通过；
Git 只提示其他既有 Rust 工作树文件的 CRLF 转 LF。最终事件回归与文档契约再次执行 10 个用例并通过。

样本波动明显；只覆盖内存状态发布及派生读取，不包含事件接纳、完整项目资源索引、IPC、React 或浏览器绘制。
本批次不据此宣称提速或全项目性能达标。完整 CI、应用构建、真实桌面和插件进程验收尚未执行；
上述聚焦 Rust 用例也不核销此前 Catalog 可用性失败。GraphMeta、其他派生读取及剩余模块仍需继续复核。

### 函数元数据与项目资源统一发布（2026-10-02）

继续检查 GraphMeta 时，新的纯应用回归在生产修改前复现：项目快照的同步订阅者已看到资源修订 2，
Graph 读取快照中的函数签名修订仍是 undefined。原因是项目加载与提交先发布 ResourceStore，
再发布单独的 GraphMetaStore；函数目录同时消费两者。该元数据 Store 没有独立业务命令，只有快照替换与清空。

保留原候选准备及结构共享，并将纯模块改名为 `core/dataStore/graphMeta.ts`。
ResourceStore 的 graphMeta 分支现在随项目资源快照一次发布，项目重置同时清空。
项目加载、索引发布与函数签名协调器直接使用现有 owner，删除旧 Store、单独发布/清空步骤及其导入，
没有兼容转发层。关闭已加载图只释放图会话，项目索引的函数元数据仍按原生命周期保留。
Graph 读取投影改为订阅单一资源 owner，函数目录也读取同一来源；未变签名、端口数组和元数据映射保留引用。

新增回归同时验证发布和清空时资源与签名的同步可见性。原有结构共享、外部输入隔离和函数签名命令测试
继续保留；中途项目替换回归在实际读取签名属性时触发替换，仍要求拒绝调用、拒绝发布且原状态不变。
本轮两个批次合计只新增两个纯测试，没有新增 UI 单元测试。

L1 的 4 文件 19 用例通过。L2 显式选择 47 文件、261 个用例通过：包含上一批运行身份的 26 份消费者测试，
以及项目快照/加载/切换/关闭、函数签名、Graph 文档动作、资源命令、资源 owner 和只读快照。
`pnpm check:ts` 通过；`pnpm lint:ts` 退出成功，仍有原来的 4 条警告；文档契约 6 个用例通过。
两个批次合并后的 50 份改动文件格式检查通过，最终 `git diff --check` 通过；
源码检索确认没有旧 GraphMetaStore 或 recordRunFailure 消费者。
本批次没有修改 Rust、没有新建 benchmark 或规则例外，也没有声称耗时收益。
Results/Execution 的其他派生读取、剩余模块和真实桌面验收继续开放，局部统一发布不等于全项目已符合规则。

### 图运行与逐输出运行事实统一发布（2026-10-02）

前两批修复了真实语义身份、失败字段及 GraphMeta 发布。本批继续检查运行展示跨 owner 的读取：
原有终态回归加入真实图输出后，在生产修改前再次失败，Graph 展示先收到“无运行节点且无失败”，
随后才收到 kernelFailed。这是 Results 查询投影中的输出活动与 ExecutionStore 中的图终态分步写入造成的。

逐输出运行身份、活动状态及等待摘要标记现已移到 ExecutionStore 的相应图记录。
applyRunEvent 用一次 Immer 同时安装运行身份、逐输出活动和图终态/失败；删除不再使用的
startExecution、setActiveRunId、completeExecution 和 failExecution 分步写入入口。
命令提交、同步未知、命令取消及清理仍保留实际使用的窄入口。Graph 展示只订阅 Resource 和 Execution，
Results 查询投影不再持有 runs，也不参与拼接运行事实。

Application 继续负责查询、缓存、租约与事件接纳。Execution owner 提供本事件实际影响的输出范围，
Results 先按该范围撤销查询并一次清理未持有缓存；已持有结果仍按完整引用保留。最终执行写入复用同一
输出归属选择规则，并在查询监听器之后重验项目、语义与观察身份。较旧运行的终态只结束其仍拥有的输出，
不会覆盖最新图的 RunId；恢复、重复开始/终态和保留结果查看沿用同一入口。

当前 Pin 读取还检查仍适用的执行输出归属，迟到的旧运行结果不能重新绑定。
可见绑定表保留未变引用；运行错误、请求等字段变化而输出记录未变时，只检查图级引用，
不重新扫描所有 Pin。结果摘要的 IPC 查询与确认仍由原协调器校验，确认等待标记通过 Execution owner 批量更新；
本批次没有把资源摘要与全部查询缓存宣称为一个总事务。

没有新增测试文件或 UI 单元测试。已有纯回归增加了 RunStarted 单次发布、失败展示单次发布、
不同输出的重叠运行及旧终态隔离；已有结果失效单次通知、报告保留、迟到请求、取消和会话重建断言继续通过。
L1 先复现问题，最终 6 文件 27 用例通过；L2 显式选择 47 文件 261 用例通过，覆盖 Results、执行、
项目发布/加载/关闭、函数签名、资源 owner、严格 IPC 消费者和既有结果窗口测试。
TypeScript 检查通过；lint 退出成功，仍有原来的 4 条警告；最终聚焦回归与文档契约 3 文件 21 用例通过。

沿用现有 Graph 发布基准，补齐其执行状态夹具并单独运行
`pnpm bench:graph:publication --outputJson <临时目录>/yssbi-run-publication-results.json`。
Windows、Intel Core i9-13900K、Vitest 4.1.10，当前未提交工作树；单位为 ms：

| 样本                  |    mean |     p75 |     RME |
| --------------------- | ------: | ------: | ------: |
| 100 图批量候选        |  8.7302 |  5.4693 | ±35.77% |
| 500 图批量候选        | 45.7295 | 77.5585 | ±20.60% |
| 1000 输出失败展示更新 |  0.0080 |  0.0051 | ±36.22% |
| 5000 输出失败展示更新 |  0.0063 |  0.0053 | ±11.32% |

展示样本使用稳定的图结果摘要和交替失败状态，没有模拟完整 RunEvent 或填充大量逐输出运行记录；
它不覆盖 IPC、真实查询、React 或绘制，也不代替端到端运行性能测量。波动仍明显，不据此声称提速。
本批没有新建 Store、订阅框架、兼容入口或规则例外，未运行完整 CI、桌面构建或人工验收。
本批 20 份源码、测试与文档的格式检查及 `git diff --check` 通过。

### Rust Catalog 历史失败复核（2026-10-02）

前端批次完成后重新读取当前 Catalog 用例及其拥有的功能延期清单，执行：

| 命令                                                                                                                | 当前结果                                                                                                  |
| ------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-application --lib all_non_deferred_builtin_nodes_are_available_in_catalog_and_sidebar` | 实际运行 1 用例并通过；目录和侧栏按当前清单一致标注可用性                                                 |
| `pnpm test:rs:package -p yss-application --lib graph::`                                                             | 实际运行 49 用例，全部通过，无忽略；覆盖 Graph 与 Automation Graph 的目录、会话、编辑提交、执行和结果读取 |

因此上文记录的 31 个非延期不可用节点及该组“48 通过、1 失败”属于历史结果，
不再作为当前工作树的已知失败。本批没有修改 Rust 源码、延期清单或测试断言，
没有把其他工作树改动的实现归为本批修复。这个结果证明当前目录契约与所选 Graph 行为通过，
不代表所有科学算法、所有 crate 或桌面交互均已验证。功能延期清单也不构成对两份架构规则的豁免。

### 结果摘要按后端事件版本确认（2026-10-02）

继续核对前一批的摘要确认边界，新增一个纯数据回归，先复现旧查询摘要在运行结束后把失效输出
重新显示为有效：期望 `new`，实际 `valid`。原因是 `publishGraphState` 安装摘要后另写
Execution 的 `pendingState`，没有表达这份摘要究竟覆盖哪个运行事件；编辑回执携带的摘要也不能
独立完成确认。

本批复用 ResultStore 现有的单调 revision。Execution 只在 registry 读锁内读取该值，不复制
输出列表或结果 payload；Application 在开始失效、成功发布及失败/取消产生事件时捕获它。
`RunApplicationEvent` 和 IPC 信封携带事件自己的 `resultRevision`，保持 RunIdentity 不变；
公共通知、专属通道和恢复快照交付同一捕获值。IPC 使用十进制字符串，两端边界保持 u64 范围。

前端 ExecutionStore 保存事件事实，删除可写 `pendingState` 和 `confirmOutputStates`。
Results 与图读取共用现有只读入口，按语义身份、执行会话及摘要 revision 是否覆盖事件版本
派生等待状态。旧摘要不能确认新运行；查询、图编辑回执和项目快照均通过原 ResourceStore
安装入口立即生效，不再触发第二次 Execution 写入。严格 parser 复用结果摘要版本 guard，
没有兼容旧 wire、额外 store 或自制事务框架。

回归还覆盖超过 JS 安全整数的 revision、新图回执一次恢复有效展示，以及 Execution 记录
保持引用。已有 Rust 实际执行用例扩展检查公共通知、失败的专属 sink 和恢复快照的完整事件
相同，开始版本晚于运行前摘要、成功终态晚于开始、随后权威摘要覆盖终态。测试只使用公开
Application 查询，不为测试开放私有 session/Execution 访问。其他事件夹具直接迁移当前契约；
只新增上述一个纯数据用例，没有新增 UI 单元测试。

验证结果：

| 命令与范围                                                                                                                                    | 结果                                                                                                            |
| --------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `pnpm test:ts`，显式选择运行事件解析/通道/安装器、Results 查询和展示、Core Execution、Graph/项目发布消费者及文档契约共 28 份文件              | 179 用例通过，包含 6 个文档契约；强化相邻大整数版本断言后，该回归再次通过                                       |
| `pnpm test:rs:package -p yss-graph-execution --lib result_store::tests`                                                                       | 8 用例通过                                                                                                      |
| `pnpm test:rs:package -p yss-ipc-contract --lib`                                                                                              | 3 用例通过                                                                                                      |
| `pnpm test:rs:package -p yss-application --lib graph::`                                                                                       | 49 用例通过，包含 Catalog 可用性检查                                                                            |
| `pnpm test:rs:package -p yss-application --lib ipc::channel::execution::tests::run_failure_wire_preserves_the_cause_phase_and_node`           | 1 用例通过，完整 wire 与共享夹具一致                                                                            |
| `pnpm test:rs:package -p yss-application --test numeric_execution project_dataset_graph_runs_through_application_authority_and_paged_results` | 1 用例通过，验证真实运行、公开查询及两类交付/恢复的一致性                                                       |
| `pnpm check:ts`、`pnpm lint:ts`                                                                                                               | 均退出 0；前端保留原有 4 条 lint 警告                                                                           |
| `pnpm lint:rs:package -p yss-application -p yss-graph-execution -p yss-ipc-contract --lib --tests`                                            | 退出 0；SCI 依赖有 6 条警告，位于 hypothesis 的 categorical、nonparametric、sample_mean、variance，未在本批改动 |
| `pnpm format:rs:package -p yss-graph-execution -p yss-application -p yss-ipc-contract -- --check`                                             | 通过                                                                                                            |

最终复核 Results runtime 和文档契约共 18 用例通过；本批 27 份前端源码、夹具、基准及文档的
`pnpm format:check:ts` 与 `git diff --check` 通过。没有运行完整 CI、桌面构建或人工交互验收。

补齐既有 Graph publication 基准的运行记录：1000/5000 输出样本各持有相同数量的已结束
输出运行记录，实际经过结果版本与摘要比较；批量准备样本保持原范围。运行命令为
`pnpm bench:graph:publication --outputJson <TEMP>/yssbi-result-revision-publication.json`，
Windows、i9-13900K、Vitest 4.1.10、当前未提交工作树，HEAD 为 `7c7d843e`。
独立执行，未并行运行编译或其他 CPU 检查；以下单位为毫秒。

| 样本                              | mean    | p75     | p99     | RME    | samples |
| --------------------------------- | ------- | ------- | ------- | ------ | ------- |
| 100 图批量准备                    | 9.7539  | 5.9448  | 58.0492 | 38.00% | 52      |
| 500 图批量准备                    | 47.3642 | 78.2581 | 90.7794 | 20.45% | 30      |
| 1000 输出及运行记录，失败展示更新 | 0.1434  | 0.1405  | 0.2076  | 1.22%  | 3487    |
| 5000 输出及运行记录，失败展示更新 | 1.1411  | 1.1335  | 1.4772  | 2.62%  | 440     |

展示样本现在包含运行记录，不能与前一批空记录样本直接推断加速或回退。批量准备波动仍大；
基准不包含事件接纳、后端版本读取、IPC、查询、React 渲染或浏览器绘制，不替代桌面验收。
本批顺应两份规则，没有申请性能豁免；全项目检查和最终验收仍未完成。

### 前端日志生成与传输生命周期（2026-10-02）

复查 Core、Domain、shared 与 utils 的实际依赖，定位到两套重复 logger：Core 实现只写 console，
Application 实现生成结构化记录并直接引用传输；共享 Markdown 因此反向依赖 Application。
两个新增纯数据回归先实际失败：DatabaseStore 的 `data / DatabaseStore` 被捕获为
`ui / frontend.console`，来源字段丢失且展示前缀进入 message；释放安装器后旧待发送记录仍被发送，
迟到的旧 cleanup 还会清空新安装的登记，导致重复安装。

本批将唯一记录生成入口归入 `src/utils/frontendLogger.ts`，复用共享日志 DTO、现有级别配置和
console 抑制工具。各层直接调用同一个 logger，旧两套实现删除，不保留转发门面。
该入口只生成记录与 console 回显，既不引用 Application/Service，也不拥有 IPC 队列。
`frontendLogTransport` 在既有启动入口绑定单一发送回调并创建原 batcher；重复安装复用同一
清理函数，释放/HMR 同时解除两个入口并丢弃未发送队列，旧清理按回调身份和安装身份拒绝干扰
新安装。传输失败继续独立于业务结果。未安装时保留 console，不另加早期记录缓冲或事件总线。
当前契约同步更新在 [Observability README](../../react/src/features/application/observability/README.md)。

受影响消费者验证还发现数据库删除的旧夹具不完整：测试要求保留 `other` 选择，返回的完整索引
却不包含该数据库。单独运行仍失败；补齐删除前后实际存在的资源和修订后，保留原断言。
同时沿项目发布调用链确认详情清理由权威快照统一对账，删除动作中多余的第二次详情清理已移除。
没有绕过缺失资源的清理，也没有把 UI 验收替换为新增测试。

| 验证                                                                                                                                                                                                                                                                                                   | 当前结果                                                       |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------- |
| `pnpm test:ts src/utils/frontendLogger.test.ts src/utils/frontendLogBatcher.test.ts src/features/core/settings/settingsStore.test.ts src/features/application/databaseEditor/useDataLoader.test.ts src/features/application/window/openWindowHelpers.test.ts`                                          | L1，5 文件 22 用例通过                                         |
| `pnpm test:ts src/features/application/dataManagement/useDatabaseManagement.test.tsx src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/core/resource/documentStateQueries.test.ts` | 数据库与发布消费者 4 文件 17 用例通过                          |
| `pnpm test:ts`，显式选择本批日志、Core 状态、应用调用方、项目发布及文档契约共 37 份文件                                                                                                                                                                                                                | L2，194 用例全部通过；先前 34 文件中的单个夹具失败已修正后复核 |
| `pnpm check:ts`、`pnpm lint:ts`                                                                                                                                                                                                                                                                        | 均退出 0，仍为原有 4 条前端 lint 警告                          |

文档契约最终复核 6 用例通过；本批 55 份源码、测试与文档的 `pnpm format:check:ts` 及
`git diff --check` 通过。本批新增两个纯数据用例；其余测试仅迁移 logger 引用或补齐既有夹具，未新增 UI 单元测试。
Rust 日志插件、wire、batcher 算法与容量未改，未重复 Rust 检查或完整 CI；真实桌面/HMR 验收仍未完成。
此次修复按既有 owner 收拢重复逻辑和生命周期，不提出架构规则例外，也不据此声称性能提升或全项目合规。

### 会话替换绑定原用例身份（2026-10-02）

继续沿 Database 的提交、刷新与 Project 生命周期追踪所有权，发现替换入口不接收预期会话。
旧用例在提交或异步工作之后调用“重建当前会话”，可能排空并重建其他操作刚安装的新会话；
Project 激活前的锁外重验与开始替换之间也存在同样的时间窗口。新增单个回归用例先在原实现
实际失败：第一次替换完成后，旧请求的第二次刷新仍返回成功。

本批由现有 `ApplicationSessionSlot` 拥有最终判断：Database 导入、复制、删除、保存、示例导入
以及 Project 的打开、关闭、另存为、创建、删除，将最初捕获的 `Arc<ApplicationSession>`
传到同一个替换入口。slot 在状态写锁内先比较 Arc 身份，再推进 epoch 并进入 `Replacing`；
失配在关闭 Execution/Database 准入之前拒绝。原有排空、候选构造、发布通知和恢复流程继续复用，
没有增加状态副本、锁、协调器或兼容入口。Project 自身的实例、文件和资源 revision 核对保留。

新增回归在修复后确认迟到刷新失败、新会话仍是原 Arc，且 Execution 与 Database 仍可接收任务。
刷新失败不撤销已提交的存储结果；持久化提交与恢复继续由 Database 原协议拥有。
当前契约已同步到 [Application 会话](../../crates/yss-application/README.md)、
[Database 用例](../../crates/yss-application/src/database/README.md)和
[Project 生命周期](../../crates/yss-application/src/project/README.md)。

| 验证                                                                                                                                          | 当前结果                                                                                      |
| --------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-application --lib session::`                                                                                     | 9 用例通过，含新增回归、候选身份、观察者与恢复所有权                                          |
| `pnpm test:rs:package -p yss-application --lib database::`                                                                                    | 13 用例通过，含导入/编辑/保存/重开、最终激活失败与存储恢复                                    |
| `pnpm test:rs:package -p yss-application --lib project::lifecycle::tests`                                                                     | 11 用例通过，含 Project 生命周期及 IPC 消费者                                                 |
| `pnpm test:rs:package -p yss-application --test database_test`                                                                                | 4 用例通过，验证数据、历史及多数据集重开                                                      |
| `pnpm test:rs:package -p yss-application --test numeric_execution project_dataset_graph_runs_through_application_authority_and_paged_results` | 1 用例通过，验证真实数据集经应用会话完成执行及分页结果查询                                    |
| `pnpm lint:rs:package -p yss-application --lib --tests`                                                                                       | 退出 0；SCI hypothesis 的 categorical、nonparametric、sample_mean、variance 仍有原有 6 条警告 |

本批 Rust 聚焦验证共 38 用例通过，新增一个非 UI 回归用例。文档契约 6 用例、Application 的
`pnpm format:rs:package -p yss-application -- --check`、四份文档的 `pnpm format:check:ts`
及 `git diff --check` 通过。未运行完整 CI、桌面构建或真实桌面切换验收。
修复遵循既有 owner 与原子状态转换，不保留规则偏离，不以正确性测试宣称性能提升；
全项目复查及最终验收仍未完成。

### 数据库声明读取与提交的一致边界（2026-10-02）

继续检查 Project 资源用例，发现数据库列表先复制整份 ProjectData，再单独读取 Database catalog，
只重验后者和 Application session；缺失 schema 时还会在应用层补造零版本空项。两个新增回归
先实际失败：Project 已发布而当前 runtime 尚未绑定的声明被作为正常查询返回；数据库删除在
publication 版本耗尽后返回错误，却已经移除了声明。

本批由既有 Project database authority 的 [read.rs](../../crates/yss-project/src/database_authority/read.rs)
提供只含声明所需事实的快照。项目实例、项目会话、根目录、authority generation、声明及其
revision/fingerprint observations 在同一 publication 锁内读取；返回前或候选构造结束前按
捕获依据重验。缺失修订直接拒绝。该快照是只读结果，不新增版本计数器、缓存或可写权威。
写侧则先准备全部可失败的版本推进，再移除声明并发布；失败保留原声明和 revision。

Application 列表查询与会话工厂共用该 Project 入口。工厂不再克隆整份项目或为数据库声明扫描
完整 ProjectIndex，也不再自行拼装 observations 或兜底零版本。查询按数据库身份建立临时 schema
查找表，核对两端 observations，并在返回前重验 Project、Database 与 Application 三个实际 owner。
缺失关联项是读取冲突；Runtime 自身合法的空 schema 继续按其契约处理。新增测试中没有物理数据的
临时声明在恢复阶段先撤销，再检查新会话和旧捕获的隔离，未放宽 Runtime 的真实缺失数据错误。

PluginHostServices 每次调用只捕获一个 Application session，该捕获同时用于上下文身份校验和
实际读取。`data.list` 调用共享的捕获会话查询，消除“检查一次会话，再重新捕获另一个会话”的窗口；
Arrow 快照与结果提交继续执行既有最终重验。既有插件用例补充了正常列表及项目关闭后的旧上下文拒绝。
当前契约同步在 [Project](../../crates/yss-project/README.md)、
[Application](../../crates/yss-application/README.md)和
[Project 用例](../../crates/yss-application/src/project/README.md)。

| 验证                                                                                                                                          | 当前结果                                                                                          |
| --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-project --lib`                                                                                                   | 47 用例通过，含删除失败原子性、文件事务、revision、编辑与 watcher                                 |
| `pnpm test:rs:package -p yss-application --lib`                                                                                               | 140 用例通过，含两 owner 读取冲突、插件数据、会话、Project、Harness、Graph、Results 与 IPC 消费者 |
| `pnpm test:rs:package -p yss-application --test database_test`                                                                                | 4 用例通过，验证数据、历史及多数据集重开                                                          |
| `pnpm test:rs:package -p yss-application --test numeric_execution project_dataset_graph_runs_through_application_authority_and_paged_results` | 1 用例通过，验证真实数据集经会话、执行及分页结果读取                                              |
| `pnpm lint:rs:package -p yss-project -p yss-application --lib --tests`                                                                        | 退出 0；仍为 SCI hypothesis 原有 6 条警告，本批未修改相关科学算法                                 |

本批 L2 共 192 个 Rust 用例通过，新增两个非 UI 用例。文档契约 6 用例、两个包的
`pnpm format:rs:package -p yss-project -p yss-application -- --check`、四份文档的
`pnpm format:check:ts` 及 `git diff --check` 通过。没有运行完整 CI、桌面构建或真实插件进程验收。
减少复制、扫描和重复捕获是本次代码路径变化，未以测试耗时推算性能提升，也没有提出规则偏离或性能豁免。
全项目其余模块及最终验收继续保持未完成。

### 数据库查询复用与按需读取（2026-10-02）

继续检查同一调用链，发现单库编辑为取得一份声明复制整份 ProjectData，四种查询重复维护
读取基线和会话重验，复制数据库的元数据步骤还会重新捕获会话。分页返回已拥有表数据与行 ID，
Application 却再次复制；声明 observations 已有 ID 索引，Runtime 和 Application 仍重复线性查找。

本批复用既有 owner：Project 在原 publication 锁内核对项目身份，按已有索引返回一份声明；
需要同项目名称集合的重命名使用声明快照。编辑提交仍保留原 revision 校验，私有修改入口从
捕获会话和目标声明取得身份，删除重复 ID 参数及对应的参数数量 lint 豁免。Application 的
元数据、分页、列分布和编辑状态共用一个私有读取入口，在查询和返回值转换后重验运行时基线与
原会话。复制的元数据、导出、导入使用同一个捕获，不在中途换读当前会话。

Runtime 的 `DatabasePageSnapshot::into_parts` 移交已有表数据与行 ID，保留借用访问接口；
Contract 的 `DatabaseDeclarationObservationSet::get` 直接暴露已有索引的借用查询，准备、
提交、补偿与 Application 修改入口共同使用，删除旧线性查找 helper。没有新增缓存、索引、
版本计数器、锁或可写事实源。行数限制、错误映射及原提交/恢复协议继续由原 owner 拥有。
当前契约同步到 [Project](../../crates/yss-project/README.md)、
[Application Database](../../crates/yss-application/src/database/README.md)和
[Database Runtime](../../crates/yss-database-runtime/README.md)。

本批新增两个非 UI 回归：同一数据库 ID 在项目重新激活后，旧项目身份不能读到新声明；
查询取得数据之后若原 Application 会话被替换，不能返回旧结果或继续用旧捕获发起读取，
新会话仍可正常查询。没有新增 UI 单元测试或为访问器单独堆叠测试。

| 验证                                                                                                                                          | 当前结果                                                                        |
| --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-database-contract -p yss-database-runtime --lib`                                                                 | Contract 2、Runtime 11 用例通过                                                 |
| `pnpm test:rs:package -p yss-project --lib`                                                                                                   | 48 用例通过，含复用数据库 ID 的项目身份回归                                     |
| `pnpm test:rs:package -p yss-application --lib`                                                                                               | 首次进程异常退出；相同命令复跑 141 用例通过，见下方待查记录                     |
| `pnpm test:rs:package -p yss-application --lib -- -- --nocapture`                                                                             | 诊断复跑 141 用例通过；当前 pnpm 入口转交给 Cargo 的测试参数为 `-- --nocapture` |
| `pnpm test:rs:package -p yss-application --test database_test`                                                                                | 4 用例通过；前次进程结束后输出未保留，因此仅复跑此目标获取完整结果              |
| `pnpm test:rs:package -p yss-application --test numeric_execution project_dataset_graph_runs_through_application_authority_and_paged_results` | 1 用例通过，覆盖真实数据集、图执行与分页结果消费者                              |
| `pnpm lint:rs:package -p yss-project -p yss-application -p yss-database-runtime -p yss-database-contract --lib --tests`                       | 退出 0；仍为 SCI hypothesis 的原有 6 条警告                                     |

上述完整通过的运行覆盖 207 个不同 Rust 用例，重复运行不重复计数。首次 Application 库测试
进程以 `0xc0000409 / STATUS_STACK_BUFFER_OVERRUN` 终止，没有完整测试汇总；此运行不能记为
通过。其后未修改源码，同参数复跑及关闭输出捕获的诊断复跑分别完成 141 用例。未取得可用于
定位的 Windows 崩溃事件或转储，根因仍未确认；两次未复现不代表已修复，继续保留待查项。

文档契约 `pnpm test:ts src/tests/documentationContract.test.ts` 的 6 个用例通过；上述四个
Rust 包的 `pnpm format:rs:package ... -- --check`、三份模块 README 与本报告的
`pnpm format:check:ts` 及 `git diff --check` 通过。

本批未运行完整 CI、桌面构建或真实桌面/插件进程验收。减少复制与线性查找是已核对的代码路径
变化，没有独立性能测量，不以测试耗时宣称性能收益。本批没有保留规则偏离或提出 benchmark 豁免；
全模块审查及最终验收仍未完成。

### 前端数据库投影与资源版本统一发布（2026-10-02）

沿数据库查询的前端消费者检查，发现项目加载和快照提交先发布独立 DatabaseStore，再发布
ResourceStore。新增非 UI 回归在原实现实际失败：数据库新增、重命名及删除的同步订阅者均能
观察到上一版资源名称与发布版本；资源条目本身没有数据库 revision，另一个可写表保存该版本。

本批把数据库声明投影并入现有 ResourceStore 的快照与清空事务，删除 DatabaseStore 及其导出，
删除独立 revision 表和读取投影中的重复版本。修改命令与图表预览按资源键读取同一 revision。
首次加载和后续索引发布共用 `prepareDatabaseIndexSnapshot`；声明成员、名称、配置、资源路径
和 revision 来自项目索引，加载响应只补充运行时元数据。Application 继续负责候选准备、项目身份
校验、IPC 与提交编排；分页与选择仍归编辑器暂态，没有新增 Store、订阅框架或并行模型。

元数据补全收窄为 `updateDatabaseMetadata`，仅更新列、行数和列数；旧 `addDatabase`、任意声明
patch、未使用的 `updateDataFrame` 和导入回执的第二套声明构造删除。即使运行时响应含另一名称
或身份，也不能覆盖当前声明或独立新增声明。复用已有 `shareProjection` 在写入边界保留未变列，
同值更新不通知，实际变更用一次 Immer 更新发布。准备好的声明候选直接加入原快照事务，不在
提交时重复深遍历整份数据库集合。读取接口及 Details 模型继续传递只读投影，不复制数据库列表。

首次修复后的聚焦验证进一步揭示旧缺失资源规则把数据库 `loaded` 当作文档缓冲，删除后留下
资源条目。该保留规则现仅适用于文件文档；数据库从权威索引消失时，声明和资源在同一次发布中
移除。已有文件缺失、dirty、stale 和冲突行为的回归仍通过。当前契约已同步到
[Features README](../../react/src/features/README.md)。

新增两个非 UI 用例分别覆盖发布可见性，以及元数据写入范围、同值通知、引用共享与原子清空。
已有声明候选测试补充首次加载时索引名称与 revision 的接纳；现有 UI 测试只迁移 owner、夹具和
窄方法 spy，没有新增 UI 用例。既有 project snapshot benchmark 仅删除失效的空参数；没有复跑
基准、改写其历史测量结果或据此宣称本批性能提升。

| 验证                                                                                         | 当前结果                                              |
| -------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| `pnpm test:ts`，显式选择 ResourceStore、资源快照、数据库修改、声明候选和项目发布集成五份文件 | L1，20 用例通过；发布回归先在原实现失败，修复后通过   |
| `pnpm test:ts`，显式选择本批修改及受影响消费者共 33 份文件                                   | L2，135 用例通过，包含文档契约 6 用例                 |
| `pnpm check:ts`                                                                              | 通过；只读数据库投影已传到编辑器集合和 Details 消费者 |
| `pnpm lint:ts`                                                                               | 退出 0，仍为原有 4 条前端警告                         |

33 份变更源码、测试与文档的 `pnpm format:check:ts`、文档契约最终复核 6 用例及
`git diff --check` 均通过。源码搜索确认已无旧 Store、重复 revision 表及失效更新入口的引用。

L2 覆盖 ResourceStore、文件/图表/Graph 读取、项目加载/关闭/发布、数据库导入/编辑、资源操作、
Chart 与 Details、侧栏、日志及文档契约。本批没有修改 Rust 或 IPC wire，没有重跑 Rust、完整 CI、
桌面构建或真实交互验收；此前 Application 原生进程终止的根因仍未确认。
本批没有保留架构规则偏离，不提出 benchmark 豁免；全项目其余模块和最终验收仍保持未完成。

### 数据库异步读取的接纳与复用（2026-10-02）

继续检查数据库读取，发现元数据补全只校验项目身份；同项目数据库 revision 前进后，迟到响应仍可
写入共享投影。新增非 UI 回归在原实现实际失败：资源已到 revision 2，revision 1 期间发出的旧响应
仍把行数从 1 覆盖为 2。编辑器还为初始载入、翻页、数据刷新和项目刷新重复维护读取与接纳流程，
初始载入等待元数据后重新分配分页序号，可能取得本应属于较新请求的所有权。

本批把有副作用的读取从声明转换文件移到
[databaseRead.ts](../../react/src/features/application/dataManagement/databaseRead.ts)，`databaseRecords` 仅保留纯数据转换。
读取依据捕获原项目身份与现有资源 revision，查询前和接纳前检查资源存在性、revision 和调用方有效性；
同项目已发布的版本变化、删除、项目切换或取消均拒绝旧结果。没有新版本计数器、索引、缓存或可写状态。
分页范围统一按当前已知行数约束，表数据与 row ID 继续共享服务返回的数组。

`useDataLoader` 的四种入口共用一条应用层流程，一个原请求序号贯穿元数据及分页，不在等待之后
重新抢占所有权。数据刷新先查询元数据，再获取相应页；项目刷新先完成原用例的项目刷新并重验身份，
未选择数据库时仍可执行项目刷新。当前请求遇到元数据失败保留原日志及分页尝试，失效请求不报告旧错误。
页状态一起保留行、row ID、计时和读取依据；依据失效后隐藏整页，清空同步释放这些字段，卸载撤销请求。
删除未使用的第二个意图计数器、对应 getter 和独立行写入出口。

编辑器按资源 revision 重新载入第一页并清理选择，不再在渲染中序列化列 schema 作为刷新指纹；
补全元数据本身不再触发另一次初始载入。Details 调用新读取入口，保留已有组件取消标记。
当前契约同步在 [Features README](../../react/src/features/README.md)。

本批新增两个非 UI 用例，分别验证迟到元数据拒绝、迟到分页拒绝及失效读取不能重启，后者同时确认
新读取仍有效、页码受边界约束且数组不复制。现有编辑器/详情用例仅补齐真实资源 revision、迁移入口及
等待顺序；旧分页回归明确等到旧请求实际发出后再开始新请求，没有把零次请求当作迟到响应验证。
没有新增 UI 用例。

| 验证                                                                                           | 当前结果                               |
| ---------------------------------------------------------------------------------------------- | -------------------------------------- |
| `pnpm test:ts src/features/application/dataManagement/databaseRead.test.ts`                    | 原实现先失败，修复后两个非 UI 用例通过 |
| `pnpm test:ts`，显式选择数据库读取、编辑器加载及两个既有元数据生命周期文件                     | L1，4 文件 9 用例通过                  |
| `pnpm test:ts`，显式选择读取、转换、修改、导入、服务、表格选择、Details 与文档契约共 14 份文件 | L2，49 用例通过，含文档契约 6 用例     |
| `pnpm check:ts`、`pnpm lint:ts`                                                                | 均退出 0；仍为原有 4 条前端 lint 警告  |

12 份变更源码、测试与文档的 `pnpm format:check:ts`、文档契约最终复核 6 用例及
`git diff --check` 均通过。

上述检查覆盖前端已观测的资源版本和请求生命周期，不能证明多个 RPC 来自同一个后端快照。
首次项目加载、导入回执补全及独立详情缓存的完整版本关联仍在审查范围，继续保持开放。
本批没有改动 Rust 或 IPC wire，没有运行 Rust、完整 CI、桌面构建或真实交互验收。
没有保留架构规则偏离或作性能收益声明，不以测试耗时替代 benchmark；全项目审查尚未完成。

### 数据库查询贯通预期资源版本（2026-10-02）

本批把元数据、分页、列分布与图表列对的 `expectedRevision` 设为当前 IPC 契约的必填参数。
前端复用原读取依据捕获的 ResourceStore revision，元数据和分页沿用同一个值；图表预览把缓存所依赖的
数据库 revision 传给查询。没有资源版本时不发起数据查询，不增加版本计数器、平行索引或兼容入口。

Application 的共享数据库读取入口在开始和返回前复用 Project 按 ID 的资源版本校验，并将请求版本与
Runtime query basis 捕获的声明 observation revision 比较。Runtime 基线沿用现有 observation，重验时
同时比较声明 observation、runtime revision 与 schema revision。Project 已前进但前端发布尚未到达、
或 Project 与 Runtime 声明尚不一致时，不再把另一版本的结果当成本次请求的数据返回。

图表列对在 Arrow 物化前校验预期声明版本，Project 校验改为按数据库 ID 查询，删除为了一个数据库
构造并扫描完整项目索引的事实结构与重复捕获函数。Harness 资源检查把原资源检查得到的同一 revision
传给元数据、分页和编辑状态。复制沿用原会话及预期版本。Harness 的带版本导出还在创建临时文件前
核对 Runtime 声明版本，拒绝不匹配的导出并保留已有目标文件；普通 GUI 导出的当前版本语义不变。
Project 错误映射接收实际操作类别，读取与导出不再一律标记为元数据操作。

本批新增两个非 UI Rust 回归：一个覆盖后端版本前进后的旧请求、Project/Runtime 声明不一致、图表消费者
及导出目标保留；另一个在读取结果转换中推进 Project revision，验证最后的 Project 重验不能省略。
原会话替换回归继续保留。导出检查加入前，第一个回归实际失败；补齐 Runtime 版本检查后通过。
现有前端调用契约和生命周期夹具已迁移，无新增 UI 用例。消费者验证还修正了两处测试夹具：
重开项目的查询使用重开后索引的 revision；图表呈现测试的局部 `react-i18next` mock 保留模块其他导出，
避免在导入期间缺少 `initReactI18next` 而实际执行零用例。

| 验证                                                                                                                                            | 当前结果                                                                                                             |
| ----------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-application --lib database::query::tests`                                                                          | L1，3 用例通过，含新增 2 项                                                                                          |
| `pnpm test:rs:package -p yss-application -p yss-database-runtime --lib`                                                                         | L2，Application 143、Runtime 11 用例通过                                                                             |
| `pnpm test:rs:package -p yss-application --test database_test`，随后指定 `row_history_save_and_multiple_dataset_reopen_keep_names_and_contents` | 首次 3 项通过、1 项夹具版本错误；修正后该项通过，4 个不同用例均有通过结果                                            |
| `pnpm test:ts`，显式选择数据库读取/加载/导入/修改、Service、表格与 Details，以及 Chart Service/预览生命周期/错误/缓存/呈现和文档契约共 20 文件  | 首次 19 文件 88 用例通过；修复上述 mock 后单独复跑 `ChartPreview.selection.test.tsx`，5 用例通过；累计 93 个不同用例 |
| `pnpm check:ts`                                                                                                                                 | 当前源码和夹具通过                                                                                                   |

`pnpm test:rs:package -p yss-application --test numeric_execution project_dataset_graph_runs_through_application_authority_and_paged_results`
另有 1 用例通过，验证真实数据集、图执行与分页结果消费者。本批共覆盖 159 个不同 Rust 用例，重复运行不重复计数。
`pnpm lint:rs:package -p yss-application -p yss-database-runtime --lib --tests` 和 `pnpm lint:ts`
均退出 0，仍分别为原有 6 条 SCI 与 4 条前端警告。
两个受影响 Rust 包的 `pnpm format:rs:package -p yss-application -p yss-database-runtime -- --check`、
20 份变更前端文件和文档的 `pnpm format:check:ts` 及 `git diff --check` 已通过。
Application 库测试本次正常结束，先前原生进程终止的根因仍未确认，不因本次通过而核销。
当前契约同步在 Database Application、Database Runtime、Chart Application、IPC 与 Features 的 README。
没有独立性能测量，也没有保留规则偏离或提出 benchmark 豁免；减少全索引查询是代码路径变化，
不以测试耗时宣称性能收益。首次项目加载、导入元数据回执及 Details 已缓存元数据的版本关联仍待检查，
本批读取 RPC 的版本约束不自动完成这些路径或全项目审查。
本批未运行完整 CI、桌面构建或真实桌面/插件进程验收。

### 数据库初始加载、缓存与导入回执的版本关联（2026-10-02）

首次加载现在先取得项目索引，再以同一项目身份读取路径、以索引的 publication revision 查询数据库 schema。
Application 在查询前后复用 Project 的索引版本检查和原数据库快照的 authority generation，
同时保留 Runtime 声明观察、目录快照及原 Application 会话重验。插件列表沿用原捕获会话的当前快照查询，
不在异步流程中重新捕获会话。Project 只暴露快照已有的 generation，没有新版本计数器。
路径归一化由 Application 承担，IPC 删除重复的文件系统处理；两个 GUI 查询直接使用当前必填 wire 参数。

初始加载与后续发布共用 `prepareDatabaseIndexSnapshot`。列、行数和加载状态只在资源仍存在且 revision
匹配时复用；数据库版本前进会在原候选事务中清除旧缓存。初始加载只复用同一项目的记录与资源依据，
新 schema 不再搭配另一版本的旧行数。准备好的快照在提交前、以及等待旧面板关闭后均检查当前发布水位。
新增纯应用回归实际复现了已发布版本 5 被准备中的版本 4 回退，加入提交检查后通过。

所有元数据写入必须提供预期资源 revision，ResourceStore 在原更新入口拒绝失效写入。
导入用例从匹配数据库 ID 的创建 delta 取得 `toRevision`，等待原发布协调器后重新核对项目身份，
再核对索引与回执提供的资源路径并补全元数据；前端不拼接数据库路径，迟到回执不能修改新版本。
管理 UI 删除自己的重复补写。另一项新增非 UI 回归覆盖正常补全与发布已前进两种结果，并验证后者不通知。
现有导入 UI 夹具改为真实创建语义和任意资源路径，没有新增 UI 用例。

Data/Chart Details 共用原数据库读取模块中的版本订阅与取消入口。资源 revision 改变后，即使旧请求
尚未返回，也会重新读取；合法空 schema 与未知 schema 分开，未知行列数显示占位。列设置使用资源版本
作为重置依据，删除渲染中的整列 JSON 序列化。实际桌面显示、并发操作及取消仍需人工验收。

| 验证                                                                                                                             | 当前结果                                                                                                      |
| -------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-application --lib database::tests::project_database_query_rejects_an_unpublished_declaration`       | L1，1 用例通过；现有回归补充错误项目身份、错误 publication revision 和原会话失效检查                          |
| `pnpm test:rs:package -p yss-application -p yss-project --lib`                                                                   | L2，Application 143、Project 48 用例通过，共 191 项                                                           |
| `pnpm test:ts`，显式选择数据库、ResourceStore、项目加载/发布/关闭、编辑器操作、Details、Chart、Service、日志与文档契约共 34 文件 | 首次 149 项通过、2 项旧夹具失败；修正后相关 3 文件 12 项通过，累计 151 个不同用例通过，未变化输入复用首次结果 |
| `pnpm check:ts`、`pnpm lint:ts`                                                                                                  | 通过；lint 仍有原来的 4 条警告                                                                                |
| `pnpm lint:rs:package -p yss-application -p yss-project --lib --tests`                                                           | 通过；依赖仍有原来的 6 条 SCI 警告                                                                            |
| `pnpm format:rs:package -p yss-application -p yss-project -- --check`                                                            | 两包通过                                                                                                      |

两处旧夹具分别依赖上一用例留下的 index mock、把重命名 delta 当成导入创建；均修改输入而保留行为断言。
文档契约最终复核 6 用例及 27 份变更前端文件和文档的 `pnpm format:check:ts` 通过。
当前契约已同步到 Features、Project、Project Application 与 IPC README。
本批不保留规则偏离，不作性能收益声明；没有运行独立 benchmark、完整 CI、桌面构建或真实插件进程验收。
本轮 Application 完整库测试正常结束，仍不能解释此前原生进程终止的根因。

### 数据库元数据在 Services 边界一次校验（2026-10-02）

继续检查输入边界，发现单库元数据和导入聚合直接把 IPC 未知值断言为返回类型；项目 schema 虽标为
unknown，随后却由 Application 跳过坏列、补默认声明及沿用旧值。两个新增非 UI 回归分别实际复现了
错误数据库 ID 的元数据被接纳、项目响应中的 null 列未经拒绝。类型声明不能作为这些输入已验证的证据。

现由 `services/database/databaseWireParser` 的模块级 Zod schema 校验元数据、项目数据库映射及修改响应数据。
列结构共用，复用原列语义 guard；检查列名唯一、列数匹配、非负安全整数计数与响应身份。数据库修改
回执直接复用原 `parseResourceMutationResultDto`，不新增回执协议或校验框架。原来 Project 索引、资源
wire 形状和资源 delta 校验中的三份数据库引擎判断收敛到已有共享 Database 类型 owner。
修复后的两个新增回归通过；已有导入用例还验证损坏的资源回执被拒绝，调用夹具遵循真实列字段和编辑状态。

Application 现在接收边界推导出的已验证类型，并核对元数据映射与同版索引的成员完全一致。
原 `normalizeDatabaseRecord`、跳过坏列、声明默认值及不受验证的 engine 断言删除。
声明仍来自索引，匹配资源版本的行数缓存和合法空 schema 保持原语义；候选准备失败不发布 Store。
旧转换测试改为声明候选、缓存引用和缺失成员检查，输入形状与语义错误检查移到 Services。
没有新增 UI 单元测试、平行状态、兼容路径或规则例外。

| 验证                                                                                                                                                                        | 当前结果                                        |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| `pnpm test:ts src/services/database/databaseService.test.ts src/services/project/projectService.test.ts -t 'rejects invalid metadata\|validates project database metadata'` | 新增 2 项先失败，确认旧入口直接返回坏响应       |
| `pnpm test:ts`，指定上述两个 Service、`databaseRecords.test.ts` 和 `projectHydration.test.ts`                                                                               | L1，4 文件 36 用例通过                          |
| `pnpm test:ts`，显式选择前述 34 个数据库/项目/Details 消费者及资源回执、项目事件 parser、Chart Service、NodeSystem golden contracts                                         | L2，38 文件 202 用例全部通过，含文档契约 6 用例 |
| `pnpm check:ts`、`pnpm lint:ts`                                                                                                                                             | 通过；lint 仍为原有 4 条警告                    |

当前契约同步在 Features 与 IPC README。本批未改 Rust 代码或 wire，也未重跑 Rust、完整 CI、桌面构建
和真实插件进程验收。没有独立 benchmark，不以测试耗时宣称性能收益。
数据库分页的 cell/row ID 边界、列分布与外部数据源发现响应仍需逐项核对；已修复元数据不能代替这些检查。

### 数据库分页、分布与发现响应边界（2026-10-02）

本批核对了 Runtime 的分页投影、TabularScalar 标量 wire、Arrow 的精确显示编码、行 ID 分配以及
分布和外部发现的实际返回类型。发现 `i64` 行 ID 直接作为 JSON number 输出，而前端 `Number.isInteger`
不能发现 JavaScript 舍入；分页 cell、行宽和身份唯一性未校验，分布及发现列表仅有返回类型断言。

IPC 现在将行 ID 编码为规范十进制字符串，前端从 Service、页面状态到 Grid 原样保留；删除 number|string
并存和按页位置补造身份的回退。Rust Application、Runtime、Dataset Store 仍使用原 i64 owner，
未改变持久化、行序或编辑权限，也没有旧 numeric-ID wire 兼容路径。新增 Rust 回归先实际复现数字输出，
再验证 `9007199254740993` 与 `i64::MAX` 的精确字符串及原单元格值。

Services 复用既有数据库 Zod 模块，一次校验有限数值/字符串/布尔/null cell、统一行宽、i64 字符串形式
与范围、唯一 ID、行与 ID 数量对应，并按请求 limit 拒绝超长页面。Application 在原版本重验后核对已知
元数据列数，不在组件重复解析。数值和类别分布按现有变体校验字段、列名唯一性与非负安全整数计数；
SQLite、远程 SQL 和 Excel 共用字符串数组 schema，名称内容和顺序不变。

本批另增 1 项非 UI 分布回归，先复现负计数直接通过；已有分页和发现测试分别迁移并扩展，验证坏 cell、
宽度、身份范围/重复/缺失、请求上限，以及列表中的对象被拒绝。发现测试也先在原实现失败。
已有 Application 读取用例补充元数据宽度不符的拒绝，编辑器夹具按当前字符串 ID 契约更新，没有新增 UI 测试。

| 验证                                                                                                                   | 当前结果                                               |
| ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| `pnpm test:rs:package -p yss-application --lib ipc::commands::command_dataframe::tests`                                | L1，2 项通过，包含精确 ID 新回归                       |
| `pnpm test:rs:package -p yss-application --lib`                                                                        | L2，144 项全部通过；首次原生进程终止的根因仍未据此核销 |
| `pnpm test:ts`，指定 DatabaseService、databaseRead、useDataLoader、databaseGridModel                                   | L1，4 文件 21 项通过                                   |
| `pnpm test:ts`，显式选择数据库读取/导入/修改/Service/表格/Details、Chart Service/预览生命周期/错误/缓存/呈现及文档契约 | L2，20 文件 96 项全部通过                              |
| `pnpm check:ts`、`pnpm lint:ts`                                                                                        | 通过；原有前端警告仍为 4 条                            |
| `pnpm lint:rs:package -p yss-application --lib --tests`、`pnpm format:rs:package -p yss-application -- --check`        | 通过；依赖仍有原来的 6 条 SCI 警告                     |

当前契约同步到 Features 和 IPC README。本批没有保留规则偏离，没有独立 benchmark 或性能收益声明。
未运行完整 CI、桌面构建和真实桌面/插件验收；表格选中状态、分页和图表呈现仍需人工检查。
数据库 Service 的上述响应已核对，其他服务的未知输入、状态 owner 与全模块覆盖继续按原清单复查。

### Chart 读取边界与发布版本关联（2026-10-02）

Chart 文档和列对读取原先直接信任 IPC 的泛型返回类型；两个新增非 UI Service 回归分别复现旧格式
文档和无限坐标被接纳。当前格式版本由模块级 Zod envelope 校验，正文沿用资源回执已有的完整 guard，
该 guard 移到共享 Chart 类型 owner 后供读取和回执共用，删除两处重复判断。列对 schema 校验有限坐标、
轴格式、必填可空标签，并在显式请求上限存在时拒绝超长响应；模型转换仅处理 null 标签的呈现缺省值。
两项新增回归修复后通过，没有新增 UI 测试、兼容 wire 或平行数据 owner。

初次视图读取此前未传已有的 publication revision。现在初次读取与索引刷新都绑定捕获版本，
IPC 命令将该参数改为必填，并直接使用原 ProjectInstanceId 类型；Application 的原生/Harness 可选版本
入口不变。前端仅对索引中存在且尚未缓存正文的资源发起读取，继续保留草稿、项目/epoch、资源修订
和读取令牌接纳。已有生命周期用例先复现缺少版本参数，又复现新发布仍共享旧 pending promise；
在途请求键加入捕获版本后，同一路径的新发布发起独立请求，旧读取不能覆盖新读取。

| 验证                                                                                                                                                                | 当前结果                                             |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| `pnpm test:ts`，指定 ChartService、chartViewActions、toChartModel 和 resourceMutationValidation                                                                     | L1，4 文件 23 项通过                                 |
| `pnpm test:ts`，显式选择 Chart 预览/缓存/Store/Details、项目发布/快照/水合、文件保存/关闭、Project/Database Service、项目事件 parser 和 NodeSystem golden contracts | L2 新增覆盖 22 文件 159 项通过；前述未变 L1 结果复用 |
| `pnpm test:rs:package -p yss-application -p yss-project --lib`                                                                                                      | Application 144 项、Project 48 项全部通过            |
| `pnpm check:ts`、`pnpm lint:ts`                                                                                                                                     | 通过；原有前端警告仍为 4 条                          |
| `pnpm lint:rs:package -p yss-application --lib --tests`                                                                                                             | 通过；依赖仍有原来的 6 条 SCI 警告                   |

当前契约同步到 Features、Chart Application 和 IPC README。本批不保留规则偏离，未运行独立 benchmark，
不以测试耗时声明性能收益；未运行完整 CI、桌面构建和真实桌面/插件验收。该批只覆盖读取，
后续 Rust Chart 类型与文件修改回执关联的复查结果见下文，其他输入路径仍继续复核。
本次库测试正常结束仍不解释此前原生进程终止的根因。

### Rust Chart 类型统一归文档 owner（2026-10-02）

继续沿持久化、索引、资源变更和 Harness 检查，确认 Rust ChartDocument 接受任意字符串类型，
而前端和 Harness 只支持 histogram/scatter/line。扩展现有严格文档契约用例后，未知类型实际通过
反序列化，回归先失败。Harness 原有的三值枚举现移到 `yss-chart-document::ChartType`，
文档、Project 索引和资源变更状态直接持有该类型；Harness 契约重导出同一类型供工具 schema 使用，
Application 两处双向字符串映射删除。合法 wire 仍是原来的三个字符串，不增加旧格式兼容。

文档 owner 复用 workspace 已有的 schemars 提供该枚举的 JsonSchema，Harness Contract 依赖该低层 owner，
没有反向依赖 Harness、Application 或 Tauri。Cargo.lock 和应用内 crate 依赖快照同步更新。
现有用例同时检查三种合法 wire 的往返；原 Project 覆盖保存、Application 图会话保留和 Harness 图表
编辑持久化用例迁移为同一枚举，没有新增测试或 UI 用例。

| 验证                                                                                                                                        | 当前结果                                               |
| ------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| `pnpm test:rs:package -p yss-chart-document --lib chart_document_has_one_current_strict_wire_contract`                                      | 增加未知类型输入后先失败；修复后由以下 L1 通过         |
| `pnpm test:rs:package -p yss-chart-document -p yss-project-history -p yss-harness-contract --lib`                                           | L1，3 + 1 + 5，共 9 项通过                             |
| `pnpm test:rs:package -p yss-project-model -p yss-project -p yss-application --lib`                                                         | L2，5 + 48 + 144，共 197 项通过                        |
| `pnpm lint:rs:package -p yss-chart-document -p yss-harness-contract -p yss-project-history -p yss-project -p yss-application --lib --tests` | 通过；仍为原有 6 条 SCI 警告                           |
| `pnpm format:rs:package -p yss-chart-document -p yss-harness-contract -p yss-project-history -p yss-project -p yss-application -- --check`  | 5 个变更包通过                                         |
| `pnpm docs:crate-dependencies`、`pnpm docs:crate-dependencies:check`                                                                        | 71 个 workspace crates、247 个依赖声明，生成与检查通过 |

当前契约同步到 Project、Chart Application 和 Harness README；前端合法 wire 不变，复用前述 Chart
边界与调用方结果。不保留规则偏离，不作性能收益声明；完整 CI、桌面构建及真实进程验收未运行。

### Mind/Doc 快照与命令回执关联（2026-10-02）

文件读取已在 Services 校验正文和快照，并在 Application 核对项目与路径。继续检查修改流程发现：
`createFileActions` 只核对外层 mutation 的项目和 operation ID，就安装内层快照或清理已删除文档。
新增两项非 UI 回归分别复现异项目/异路径快照被发布，以及没有创建/删除 delta 也执行本地安装/清理。
这与前述两个 Chart Service 回归不同，针对文件命令聚合中的上下文关联，不能由单个 wire parser 覆盖。

接纳现复用已解析的生命周期和移动 delta，核对资源种类、操作、目标路径、前后版本及保留的编辑会话。
创建/复制要求对应创建 delta，重命名的目标取自实际 move；空快照仅能配合 Delete/Discard 的删除 delta。
核对在正文安装、输入缓冲迁移、删除清理和项目发布之前完成，失败仍走原权威读取恢复入口。
没有重复结构解析，没有推算资源路径，也不新增队列、Store、发布器或协议。

已有重命名用例改用真实 move/delta 夹具，继续检查同时输入的文本被保留并使用新路径提交；
新回归同时确认有效创建与删除仍成功。每批的新增回归均为非 UI；原有 UI 测试仅作为受影响调用方运行。

| 验证                                                                                                                                   | 当前结果                                        |
| -------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| `pnpm test:ts src/features/application/resource/createFileActions.test.ts -t 'rejects command snapshots\|requires matching lifecycle'` | 新增 2 项先失败，确认错误内容被发布和无授权清理 |
| `pnpm test:ts`，指定 createFileActions、docActions、mindActions 和 projectFilePublication                                              | L1，4 文件 15 项通过                            |
| `pnpm test:ts`，显式选择资源分派/文件管理/输入缓冲、Core 文件快照/文档查询、保存/关闭/打开入口及 Doc/Mind Service                      | L2，12 文件 43 项通过                           |
| `pnpm check:ts`、`pnpm lint:ts`                                                                                                        | 通过；原有前端警告仍为 4 条                     |

当前契约同步到 File operations 和 IPC README。本批未改变 Rust，复用前述 206 项相关库测试，
未重新运行完整 CI、桌面构建或真实桌面/插件验收。不保留规则偏离，没有独立 benchmark 或性能收益声明。

### Graph 与项目命令回复、进度的输入边界（2026-10-02）

继续核对 Services，发现 Graph 卸载回复仅由泛型断言为 boolean，字符串 `"false"` 会作为真值进入
前端释放流程；函数签名修改回复也未经过已存在的资源回执 parser。新增一项纯 Graph Service 回归，
并扩展原函数修改用例，分别实际复现字符串确认、负发布版本被直接返回。
卸载现使用模块级布尔 schema；函数修改直接复用 `parseResourceMutationResultDto`，没有另写回执规则。
项目身份、Graph lifecycle token、FIFO、签名请求/版本和发布协调仍由原 Application 入口负责。

项目事件已有激活、注册记录和生命周期回执 parser，命令回复却仍直接使用泛型返回类型。
这些原 parser 移到 `projectWireParser` 供事件和命令共用，删除事件模块中的旧定义；保留字段、可空值、
枚举与注册时间/路径字符串语义，不重新维护一套 Zod 合约。激活结果类型归共享 Project 类型 owner，
调用方改用该类型；进度类型从新增模块级 Zod schema 推导，删除 Service 中的平行手写类型。
新 schema 只负责原来没有校验的标量确认/路径、扫描/清理结果与进度，检查安全整数和跨字段数量关系。

另增两项非 UI Project Service 回归：第一项复现坏激活版本、注册字段、生命周期失效标记和扫描计数被接纳；
第二项复现非法进度以及命令完成后仍交付进度。这两项针对项目返回契约和 Channel 生命周期，区别于前述
Graph 卸载回归。现由原 Service 的共享进度适配入口过滤坏消息，并经既有 logger 留下诊断；辅助进度失败
不替代已提交的注册/清理结果。finally 复用既有 Channel 清理函数，成功、失败都撤销消息处理器后退出 HMR 登记。
没有新增业务状态、订阅 owner、通用校验框架或兼容 wire，也没有新增 UI 单元测试。

| 验证                                                                                                                                                       | 当前结果                                                                       |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `pnpm test:ts src/services/nodeSystem/functionMutationService.test.ts src/services/graph/graphService.test.ts`                                             | 两处入口回归先失败，修复后纳入以下 L1                                          |
| `pnpm test:ts`，指定上述两个 Service、functionSignatureCoordinator 和 graphDocumentUnload                                                                  | L1，4 文件 13 项通过                                                           |
| `pnpm test:ts src/services/project/projectService.test.ts -t 'validates activation\|ignores malformed picker'`                                             | 新增 2 项先失败，确认坏回复与迟到/非法进度被交付                               |
| `pnpm test:ts`，指定 ProjectService、projectEventParser 和 projectHydration                                                                                | L1，3 文件 23 项通过；logger API 名称经类型检查校正后 Service 的 19 项复跑通过 |
| `pnpm test:ts`，显式选择项目选择/切换/IOStore、事件流/恢复、生命周期回执、项目发布/快照、Graph 取消/关闭/保存/执行、资源契约与 NodeSystem golden contracts | L2，18 文件 140 项通过；未变化 L1 结果复用                                     |
| `pnpm check:ts`、`pnpm lint:ts`                                                                                                                            | 通过；原有前端警告仍为 4 条                                                    |

当前契约同步到 Project Application、Graph Application 和 IPC README。本批没有修改 Rust 源码或 wire，
未重跑 Rust、完整 CI、桌面构建和真实桌面/插件验收。不保留规则偏离，没有独立 benchmark 或性能收益声明。
插件服务的生成 schema 适配和标量回复由下一节继续覆盖；其他尚未覆盖的服务仍按原清单检查，不能由本批推定全模块合规。

### 插件回复边界与宿主动作归属（2026-10-02）

插件 Service 原有对象校验继续复用 Rust 生成 schema，集中到 `pluginWireParser`；没有另建生成类型的
Zod 副本。属性查询改为只认 schema 自有字段，修复 `constructor` 等原型属性绕过禁止额外字段的问题。
Service 为导出令牌、垃圾回收计数补充模块级 Zod，并将存储、清缓存、历史和诊断结果的 `pluginId`
与原请求关联。生成契约与 Rust wire 均未修改；原有对象引用、嵌套校验和容量限制继续保留。

原 Application 只按回复中出现的 `hostUi`、`openView` 执行宿主动作，保存的视图状态和插件业务结果
也会被误认。现由 Service 根据原请求方法解析保存文件、显示文件和打开视图的宿主回复，`PluginView`
仍使用生成 schema。内部判别值只用于 Service 到 Application 的分派，不进入插件 wire；普通结果原样
交给插件，不能因为携带同名字段而触发宿主动作。组件中的未知结果断言已删除。

保存对话框返回后，Application 复用视图 owner 的有效性判断，再申请导出令牌；关闭、导航或替换后的
迟到选择不再发起授权。会话释放、失败重试、端口撤销、打开视图时的插件身份检查及最终回复接纳继续
由既有 owner 负责，没有新增共享状态、兼容通路或 UI 单元测试。

新增四项非 UI 回归分别覆盖标量回复、生成字段/插件身份、跨方法宿主动作误认及迟到保存选择。
四项均先失败：数字令牌、未声明字段被接纳，普通视图状态触发显示文件，失效视图仍申请导出授权。
修复后通过，并覆盖对应合法返回；新增数量及各项风险已在实施前说明。

| 验证                                                                                                          | 当前结果                                 |
| ------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| `pnpm test:ts`，指定 pluginService、pluginActions、pluginViewActions、pluginViewSession                       | L1，4 文件 7 项通过                      |
| `pnpm test:ts src/features/application/plugins/usePluginView.test.tsx src/services/ipc/invokeCommand.test.ts` | L2，2 文件 15 项通过；既有视图用例未修改 |
| `pnpm check:ts`、`pnpm lint:ts`                                                                               | 通过；保留原有 4 条前端警告              |

当前契约更新到 IPC README。本批没有修改 Rust 源码、生成文件或外部插件 SDK，未重跑 Rust、完整 CI、
桌面构建和真实桌面/插件进程验收。不保留规则偏离，没有独立 benchmark 或性能收益声明。

### Rust 插件会话和授权的提交前重验（2026-10-02）

继续沿上一批的视图关闭路径核对 Rust：`grant_export` 在初次读取上下文和项目身份后才重新锁定
RuntimeState，期间发生 detach 会删除上下文，但旧请求仍可插入导出令牌；`attach_view` 在准备项目
上下文后也未重验捕获的进程，已退出的进程仍可发布一个新视图。两处均以真实插件进程和受控查询暂停
实际复现：预期拒绝的请求返回了会话或令牌。

运行状态 owner 现统一提供活动进程/上下文检查。附加在发布上下文前重验捕获的实例，授权在写入令牌前
重验仍登记的上下文和活动实例，检查与写入共同持有既有 RuntimeState 锁；detach 和进程故障撤销使用
同一把锁。原上下文读取入口复用该检查，文件读取和 HostServices 项目查询保持在锁外。
没有新增状态 owner、事务框架、协议字段、迁移或兼容通路。

本批实际构建了 `target/plugin-packages/` 中的开发包，消除了原生会话验收缺少产物这一前提障碍。
两项新增原生回归使用项目读取处的握手暂停来控制并发顺序，不依赖 sleep；修复后分别验证新进程可以
重新附加、新视图可以正常取得授权。原有并发激活、配额、跨窗口释放和卸载用例同时通过。
该范围启动真实插件进程，不准备 Julia 环境、不运行推断；完整计算链和桌面呈现验收仍未完成。

| 验证                                                                                                                                           | 当前结果                                                                    |
| ---------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `pnpm plugin:julia:package --dev`                                                                                                              | 开发包构建成功；网页保留既有 chunk 大小警告                                 |
| 指定 `YSSBI_PLUGIN_TEST_PACKAGE` 后运行 `pnpm test:rs:package -p yss-plugin-runtime --test native_extension view_ -- -- --ignored --nocapture` | L1，新增 2 项先失败；修复后 3 项真实进程用例通过，完整 Julia 用例被明确过滤 |
| `pnpm test:rs:package -p yss-plugin-runtime --lib --test installation`                                                                         | L2，5 项库测试和 4 项安装/存储消费者通过                                    |
| `pnpm lint:rs:package -p yss-plugin-runtime --lib --tests`                                                                                     | 通过，无警告                                                                |
| `pnpm format:rs:package -p yss-plugin-runtime -- -- --check`                                                                                   | 通过                                                                        |

当前契约同步到 Plugin Runtime 与 Julia 插件 README。前端、生成契约和外部插件实现源码没有改动，
未重复未变化的前端检查，也未运行完整 CI 或桌面构建。不保留规则偏离，没有独立 benchmark 或性能收益声明。

### Results 回复与查询身份关联（2026-10-02）

Results Service 已有结构 parser，但 descriptor、分页和 claim 未关联原请求，Pin 查询也未核对回复中的
输出地址。查询协调器只按请求身份判断是否过期，随后按请求键发布结果；因此另一个结果的合法 DTO 仍可
进入错误的缓存键。现由 Service 在原 IPC 边界核对完整结果引用、页面 ResultId/请求大小/偏移、Pin 图与
端口以及 claim 的预期租约 token。复用结果引用与端口键函数；value/page 未定义的响应身份字段没有新增。

既有 provenance parser 同时验证非空输出的图和节点与自身字段一致，保留契约允许的 null 输出。
分页检查经过 Rust 实现核对：物化数据与报告表对已知末尾使用 `min(offset, totalCount)`，未知总数的
关系页保留请求偏移。这两种合法空末页均保留，不把所有回复偏移强制等同于原请求。
面板 lease controller 的请求 token/结果身份确认、失败清理，以及独立窗口对目标结果的关联继续归
Application；没有在呈现层追加解析、建立新缓存或维护另一份结果 wire。

新增两项 Service 回归分别覆盖响应身份和分页范围，扩展原 parser 用例验证 provenance 字段一致性；
三项在修复前实际失败，修复后全部通过。没有新增 UI 单元测试。

| 验证                                                                                                                                                                           | 当前结果                    |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------- |
| `pnpm test:ts src/services/result/resultService.test.ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/results/resultLeases.test.ts` | L1，3 文件 14 项通过        |
| `pnpm test:ts`，显式选择 runtime、Pin 搜索、可检查结果、图结果呈现、分页、读取错误、独立窗口加载/生命周期、窗口打开、ResultPanel/ResultContent 及 NodeSystem golden contracts  | L2，12 文件 74 项通过       |
| `pnpm check:ts`、`pnpm lint:ts`                                                                                                                                                | 通过；原有前端警告仍为 4 条 |

当前契约更新到 Results Application README。Rust、wire 与生成文件未修改，未重跑 Rust、完整 CI、
桌面构建或真实交互验收。不保留规则偏离，没有独立 benchmark 或性能收益声明；全模块复核仍未完成。
文档检查同时发现新增节点分类草案缺少生命周期元数据，已补齐 Planned 标记、实现 owner 链接及路线图入口。
分类建议仍是待评估方案。文档契约 6 项复跑通过，不能据此声称分类方案已实现。

### 端口与图输出身份键的共享归属（2026-10-02）

端口键原先分别在 Domain、投影 parser 和连接候选 Service 实现，Results parser 另行组合图输出键。
现将既有 Domain 长度前缀编码移到 `shared/types/domain/portAddressKey.ts`，原 Domain 公开入口继续
导出同一函数；投影、Results 和连接候选的边界校验直接复用共享实现。移除重复实现和旧内部文件，
Service 不再为了生成端口键依赖完整的图投影 parser。实体、DOM 与结果查询使用的原有键格式保持一致，
没有引入状态、兼容转换或新的校验框架。当前职责更新到 Features README。

这是行为保持的归属整理，复用已有地址变体/分隔符、身份核对、重复项及投影上下文回归，没有新增测试。

| 验证                                                                                                                                                                               | 当前结果                    |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- |
| `pnpm test:ts src/features/domain/editorProjection/editorProjection.test.ts src/services/nodeSystem/connectionCandidatesService.test.ts src/services/result/resultService.test.ts` | L1，3 文件 31 项通过        |
| `pnpm test:ts`，显式选择 Graph 同步、投影 Service、NodeSystem golden、实体安装、Pin 目标、诊断、编辑命令和 Results runtime/查询/呈现/搜索                                          | L2，11 文件 110 项通过      |
| `pnpm check:ts`、`pnpm lint:ts`                                                                                                                                                    | 通过；原有前端警告仍为 4 条 |

没有修改 Rust、wire 或生成契约，没有独立 benchmark 或性能收益声明，也没有保留规则偏离。
本批未运行完整 CI、Rust 或真实桌面验收；全模块复核仍未完成。

### 图表同列双轴的物化归属（2026-10-02）

图表配置和调用方允许两个轴选择同一列，但 Runtime 原先把重复列名直接交给关系投影，实际返回
`ColumnMaterializationFailed`。新增一项 Runtime 回归已复现该失败。现由 `plot_query` 在原入口
按唯一列投影和转换，同列双轴共享已有 `Arc` 数值数组；异列继续在同一关系读取，空值位置、标签和
日/微秒坐标保持不变。关系引擎不增加图表特例，Application 继续负责有限值筛选、点数限制和会话/版本重验。
当前契约更新到 Database Runtime README。

扩展已有时间列测试覆盖空值位置，已有 Application 查询用例验证同列数值结果和原版本拒绝行为。
没有新建状态、通用物化框架或 UI 测试。

| 验证                                                                     | 当前结果                                                        |
| ------------------------------------------------------------------------ | --------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-database-runtime --lib plot_query::tests::` | 修复前 1 通过、1 失败；修复后 2 项通过                          |
| `pnpm test:rs:package -p yss-database-runtime --lib`                     | L2，12 项通过                                                   |
| `pnpm test:rs:package -p yss-application --lib database::query::tests::` | 3 项通过，包含同列图表、Project/Runtime revision 与会话替换检查 |
| `pnpm test:rs:package -p yss-application --lib chart::`                  | 1 项通过，覆盖有限值筛选、点数上限与轴格式                      |
| `pnpm lint:rs:package -p yss-database-runtime -p yss-application --lib`  | 通过；依赖中的既有 SCI 警告仍为 6 条                            |

上述 L2 共 16 项 Rust 测试通过；未重跑完整 CI、完整 Application 库或桌面交互。
这次修复遵守现有适配职责与不可变数据共享，没有独立 benchmark 或性能收益声明，不保留规则偏离。

### 视口同步与释放归回状态 owner（2026-10-02）

已提交视口的变化原先由每个读取订阅回写到实时状态：没有订阅时，快照已更新但 `getViewport`
仍返回旧的实时坐标。面板释放忽略实时 entry 的清空通知，按图释放又只遍历已提交表，导致尚未提交的
坐标漏掉通知或清理。两项新增 Core 回归已分别复现这些问题，没有新增 UI 单元测试。

现由现有 Viewport Store 通过一份 Zustand 订阅统一同步已提交变化；面板读取只订阅自身 scope，
无消费者时同步也生效。面板、图和项目释放都覆盖未提交 entry，先移除提交值，再通知读取恢复有效坐标。
项目加载复用 `clear()`，不再只重置提交表。实时 pointer 更新仍只写所属面板的 Zustand entry，
没有新增平行状态或自制订阅框架；Features README 同步当前职责。

| 验证                                                                                                                               | 当前结果                               |
| ---------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| `pnpm test:ts src/features/core/viewport/viewportSession.test.ts`                                                                  | 修复前 2 通过、2 失败；修复后 4 项通过 |
| `pnpm test:ts`，显式选择初始视口、memento、画布 session、项目加载/快照/关闭、面板关闭、图卸载、常量拖入及已有 GraphFlowCanvas 用例 | L2，12 文件 67 项通过                  |
| `pnpm check:ts`、`pnpm lint:ts`                                                                                                    | 通过；原有 4 条 lint 警告              |

该批遵守 Zustand 与既有生命周期边界，没有独立 benchmark、性能收益声明或规则豁免。
测试包含已有画布适配器用例，仍不替代真实桌面平移、取消和分屏验收；全模块复核继续开放。

### 画布取消回调的注册身份（2026-10-02）

手势清理注册被消费后，同一 graph/group/type 可以登记下一次手势。旧注册的注销函数原先只看
自己的 callback 集合已空就删除该 key，会误删后来登记的新集合，使新手势取消时漏掉清理。
现仅当注册表中仍是原集合时删除 entry，复用现有注册 owner；没有增加手势状态或改变取消 API。
这是不同于前述视口发布与释放的注册身份回归，因此额外新增一项 Core 测试；未新增 UI 测试。

`pnpm test:ts src/features/core/canvas/canvasInteractionCleanup.test.ts` 在修复前实际为
8 项通过、1 项失败。修复后显式选择该文件、GraphInteraction Store、现有 useCanvasInteraction、
useEditorKeyboard 及 graphDocumentUnload 消费者，共 5 文件 30 项通过（图卸载与前一批验证重叠）。
`pnpm check:ts`、`pnpm lint:ts` 通过，原有 4 条 lint 警告未变化；Features README 已更新生命周期契约。
没有新建 benchmark、规则例外或性能声明；真实桌面手势验收和全模块复核仍未完成。

### 文档并发提交与侧栏投放的原操作身份（2026-10-02）

`fileTextInput.flush` 原先只等待一次 `inFlight`：多个等待者在同一次编辑完成后醒来，会重复提交
期间新增的文字。新增回归通过延迟第一项编辑复现三次提交，而正确结果应为两次。现每次醒来都重验
原缓冲区的在途 Promise；只有一个调用方使用上次回复的版本提交下一批文字。保留编辑 generation、
失败传播和释放检查，不新增队列或缓冲区 owner。Resource README 已同步当前并发契约。

函数投放原先在激活面板前捕获处理器，激活完成后仍调用旧函数，且没有重验拖动开始时的项目身份。
现把现有拖动身份传入该用例，在激活前后校验，并在激活后读取当前画布注册。投放被拒绝后的资源打开
也重验原项目，避免项目切换后继续执行旧拖动。复用既有 Core 身份与注册表，Features README 更新契约。

本批新增三项非 UI 回归分别针对重复文字提交、激活期间处理器替换、激活期间项目替换；第三项具有
独立的跨项目副作用风险。三项最初均实际失败；项目替换用例随后扩展到完整拖放结束入口，检查既不调用
处理器也不进入资源打开 fallback，并确认没有通过错误捕获路径掩盖失败。没有新增 UI 单元测试。

| 验证                                                                                                                     | 当前结果                                         |
| ------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------ |
| `pnpm test:ts`，显式选择 fileTextInput、createFileActions、Mind actions、Doc actions                                     | 4 文件 16 项通过                                 |
| `pnpm test:ts`，显式选择函数投放、常量投放、侧栏投放策略和已有 useCanvasDrop 用例                                        | 4 文件 6 项通过                                  |
| `pnpm test:ts`，显式选择更新后的函数投放回归、文件管理/动作、面板/项目关闭、项目生命周期、DnD 输入契约、模板创建与打开图 | L2，11 文件 60 项通过；函数投放 2 项与前一批重叠 |
| `pnpm check:ts`、`pnpm lint:ts`                                                                                          | 通过；原有 4 条 lint 警告                        |

同轮读取核对确认 Chart Preview 缓存的结算同时比较 generation 和 Promise 身份，Graph activity
刷新任务的清理也比较原 entry；这些入口本轮没有修改。画布处理器注册与卸载的后续复核见下一节。
本批没有修改 Rust 或 wire，没有运行 Rust、完整 CI 或真实桌面验收；无独立 benchmark、性能声明
或拟保留规则偏离。全模块审查仍未完成。

### 画布投放注册的卸载归属（2026-10-02）

原画布卸载仅用 panel ID 调用 `setHandler(id, null)`，不能区分本次挂载与后来替换的注册。
现由原 Zustand Store 的 `registerHandler` 返回专属注销函数，以每次注册的对象身份确认所有权；
同一回调被下一次挂载复用时也不会被旧注销删除。读取仍使用原 `getHandler`，旧可空 setter 和
重复内部 setter 已删除，Application 直接交还该注销函数，不增加注册 owner 或订阅机制。

新增一项 Core 回归在保留原按 key 清理行为时实际失败，身份核对后通过；覆盖回调复用、迟到/重复
注销和其他面板隔离。已有 useCanvasDrop 用例只适配新的注册/注销接口，没有新增 UI 测试。
Features README 已更新当前生命周期契约。

`pnpm test:ts` 显式选择注册表、已有 useCanvasDrop、函数/常量投放、模板创建、投放策略和 DnD
契约，共 7 文件 20 项通过。`pnpm check:ts`、`pnpm lint:ts` 通过，仍为原有 4 条 lint 警告。
本批没有修改 Rust、wire 或持久化，没有新 benchmark、性能声明或规则豁免；桌面挂载/卸载与
分屏拖放验收仍开放，局部生命周期修复不代表全项目复核已完成。

### Graph Analysis 缓存依据复核（2026-10-02）

本轮读取核对 `resolution`、Schema 缓存、类型缓存及当前节点规则的调用链，没有修改 Rust 实现。
Schema 缓存同时检查计算指纹与资源依赖，命中时仍记录缺失资源读取；类型缓存先计算当前输入、
端口及泛型状态，再结合节点协议指纹判断复用。已检查的节点规则仅通过协议默认参数读取注册表，
该默认值已纳入协议指纹，未在这个范围内确认遗漏注册表变化的失效条件。

缓存只复用派生中间结果，最终仍组装完整的 `GraphSemanticSnapshot`；没有增加第二份语义事实源。
`pnpm test:rs:package -p yss-graph-analysis --lib cache` 实际运行 6 项并通过，覆盖常量类型、
表格单元格、资源缺失/恢复、重连、环、已删除输出与转换约束。再按完整测试名运行
`incremental_semantics_equal_full_resolution_and_stop_at_unchanged_output_types`，1 项通过，
核对同类型常量变化时的完整/增量求解等价与下游复用。

这 7 项已有回归支持本次缓存依据检查，不代表 Graph Analysis 全模块或全项目审查完成；
没有新增性能例外、benchmark 或性能结论。其余求解边界和消费者继续逐项核对。

### Problems / Output 呈现与定位归属（2026-10-02）

两模块的呈现文件及公开入口已读取核对：活动图来自 FlexLayout 的共享读取，Problems 按图读取
ResourceStore 的 canonical diagnostics，Output 只选择 Execution 读投影的当前失败；失败读取
核对语义 hash 与执行会话。呈现层没有另建诊断、日志或运行状态 owner，格式化使用既有领域入口。
未绑定输入判断中唯一残留的旧诊断别名已删除；Rust 当前定义和前端模板使用 `graph.input.unbound`，
不保留另一套旧代码语义。

共同调用的 `revealGraphProblem` 原先在 Details 等待结束后直接激活旧面板；项目切换虽被末尾
检查发现，错误激活已发生，同项目内面板改绑则可能继续成功。现在先确认原项目，再复用
`captureEditorCommandTarget` / `isEditorCommandTargetCurrent`，在 Details、激活与布局帧后
重验同一目标。已移除确认面板前重复发布的节点选择和 Details 写入，由原选择入口与 `revealDetails`
各写入一次。Editor README 已更新当前契约，没有新增目标 owner、状态机或 UI 测试。

新增两项 Application 编排回归分别覆盖项目切换和同项目面板改绑；补齐测试环境后，两项都在
旧实现上实际因错误激活失败，修复后通过。L2 显式运行定位、目标校验、Details、既有 Problems
组件、诊断、运行失败、运行事件及 Workbench 激活/面板动作，9 文件 50 项通过。删除旧诊断别名后
再运行领域诊断与既有 Pin 消费者，2 文件 8 项通过，其中领域诊断 5 项重叠；本批共 10 文件
53 项不同用例通过。`pnpm check:ts`、`pnpm lint:ts` 通过，原有 4 条 lint 警告未改变。

本次修复没有改 Rust、wire 或持久化，也不保留需要 benchmark 反驳的规则偏离。Problems / Output
的桌面定位、分屏和字段焦点验收仍开放，其他模块继续依照覆盖表复查。

### Schema 默认参数与数列输入缓存依据（2026-10-02）

继续复查 Graph Analysis 的派生端口、Schema 组合及变换时，确认两处此前未覆盖的偏差：
类型求解读取协议默认转换目标，列语义推导却只看文档显式参数；Schema 缓存仅为频数节点补充
数列意义，合表/设列等消费者在上游数列改变类型但仍无表 Schema 时会错误复用旧结果。
此前 7 项缓存回归没有覆盖这一输入类别，不能据此推定整个缓存边界完备。

现将原文本参数读取移回 `parameter_projection`，由列语义、正向类型与自动转换约束共同调用；
没有复制第二份默认值或新建参数模型。Schema 缓存将每个输入的列事实或解析问题与原地址、
表 Schema 一起纳入指纹，删除频数专属补丁。原 resolver 内按源端口复用本次解析的列事实，
避免不同消费者反复沿相同连接追溯；成功与失败结果都随本次 resolver 释放。持久可丢弃缓存和
最终 `GraphSemanticSnapshot` 的所有权保持不变，当前契约已写入 Analysis README。

新增两项回归分别确认默认值标签经过频数节点应为 Categorical，以及上游目标从 Categorical
改为 Ordinal 后合表和设列输出应更新。旧实现实际分别返回 Unknown 和旧 Schema；修复后字段
正确、增量/完整快照与资源依赖一致，并仍可复用未变输出。没有新增 UI 测试或 wire 变更。

| 定向命令与范围                                                                                                | 结果                                      |
| ------------------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| `pnpm test:rs:package -p yss-graph-analysis --lib`                                                            | 35 项通过，包含 2 项新增回归              |
| `pnpm test:rs:package -p yss-graph-editor --lib`                                                              | 13 项编辑、端口、常量及投影消费者通过     |
| `pnpm test:rs:package -p yss-graph-runtime --lib snapshot_`                                                   | 3 项资源缺失/恢复、诊断与传递函数缓存通过 |
| Runtime 按完整名称运行 `layout_reuses_semantics_and_projects_current_display_but_constant_metadata_refreshes` | 1 项通过                                  |
| `pnpm lint:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime --lib`                   | 通过，无警告                              |

本批共 52 项不同 Rust 用例通过；没有运行完整工作区或桌面验收。此次遵守既有 owner、共享规则
与可丢弃索引约束，没有拟保留偏离，也没有新增 benchmark 或性能结论。其余求解边界与模块
仍按覆盖表继续复核。

### 自动转换与下游列 Schema 的共同解析（2026-10-02）

继续沿 Schema / 类型边界复查，确认自动转换虽然能由下游约束得到精确类型，先执行的 Schema
阶段却看不到该结果。频数的值列仍为 Unknown，随后选列继续未解析；只改显式/default 参数读取
不能解决这个阶段次序问题。一项新增回归在旧实现上实际返回 Unknown 而非 Known(Numeric)。

`resolution` 现在协调已有 Schema、端口投影和反向类型约束。确定的自动输出类型发生变化时才
重新准备 Schema/端口；原约束求解器接收本次前一轮的结果并继续取交集，有限类型域只收窄。
未确定/冲突都对应未知列意义，不因候选集合变化而重复构造 Schema；无自动转换时仍为一轮。
正向类型阶段复用最后一轮约束，不另外运行一遍反向求解。中间节点和诊断保持局部，稳定后才
完成输入、语义与函数校验并组装唯一快照；约束不跨请求保存，既有索引及可丢弃缓存继续使用。
Graph 类型到数据类型的转换复用 `yss-graph-type-mapping`，没有另一套类型 ID 映射。

新增回归覆盖三段连续的自动转换→频数→选列，让前段列意义成为后段转换的约束；不能用固定
两轮替代收敛。Numeric 改为 Binary、增加冲突、再解除冲突后，类型、Schema、Ready、完整/增量
结果和资源依赖保持一致，文档中的自动参数没有被写成推断值。当前阶段契约已更新 Analysis README。

| 命令与范围                                                                                                         | 结果                                                |
| ------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------- |
| `pnpm test:rs:package -p yss-graph-analysis --lib`                                                                 | 36 项通过，新增 1 项                                |
| `pnpm test:rs:package -p yss-graph-editor --lib`                                                                   | 13 项通过                                           |
| `pnpm test:rs:package -p yss-graph-runtime --lib snapshot_` 及按完整名称运行布局/常量元数据复用用例                | 3 + 1 项通过                                        |
| `pnpm test:rs:package -p yss-graph-execution --lib graph_preparation`                                              | 6 项类型/coercion、输出、Ready 与计划缓存消费者通过 |
| `pnpm lint:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime -p yss-graph-execution --lib` | 通过，仍有 SCI 原有 6 条警告                        |

本轮共 59 项不同 Rust 用例通过；没有改前端或 wire，没有运行完整 CI、完整执行库或桌面交互。
没有新增规则豁免或性能结论；后续全模块审查仍保持开放，局部收敛正确性不代表所有求解边界已完成。

### 引用端口的批量补全与目录测试校正（2026-10-02）

继续复查参数投影、端口、语义及函数校验入口，发现 orphan 补全会为每个未知引用复制该节点完整端口表，
并反复扫描全部诊断。现在仍由 `port_projection` 准备本次候选事实，按节点收集后一次追加；原有端口直接移入
最终列表。借用引用地址去重，首次实际补全时才建立已有端口诊断索引；有效图不额外建立该索引。
节点内首次引用顺序、首次引用方向、连接计数与诊断顺序保持原行为，没有新增持久状态或发布入口。
当前归属与批量处理契约已同步 Analysis README。

Runtime 的现有目录测试实际失败：它仍认为 `yssbi.dataframe.labels` 没有端口，而当前 Catalog 已声明
数列输入与输出。这里只校正该项接口预期；测试模拟的 kernel 集合仍不包含此节点，因此不可用断言保留。
Catalog 生产实现没有改动，也不能将该模拟结果解读为实际会话中此节点始终不可用。

| 命令与范围                                                                                  | 结果                                            |
| ------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| `pnpm test:rs:package -p yss-graph-analysis --lib`                                          | 36 项通过                                       |
| `pnpm test:rs:package -p yss-graph-editor --lib`                                            | 13 项通过                                       |
| `pnpm test:rs:package -p yss-graph-runtime --lib`                                           | 13 项通过；原有 1 项手动 benchmark 保持 ignored |
| `pnpm lint:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime --lib` | 通过，无警告                                    |

本批共 62 项不同 Rust 用例通过。补全调整复用现有语义、端口和投影消费者覆盖，没有新增测试或 UI 单元测试。
没有改前端或 wire，也没有运行完整工作区或桌面验收。这是既有责任内的批量构造，不保留规则偏离，
没有新增 benchmark 或声称测得性能收益。上述入口的只读检查不代表 Graph Analysis 或全模块复查已经完成。

### 有效参数与后投影校验的一致性（2026-10-02）

经 Registry 注册的合法协议可声明参数默认值和条件显隐，但资源引用及 Schema 参数校验原先只读取
文档显式值。两项新增回归确认：默认过滤条件引用消失的列、默认资源不存在时，都未产生应有诊断；
两个用例完成合法注册后，分别在缺失诊断断言处实际失败。

现在 `node_projection` 先构造已有有效参数事实，再交给资源校验并装入同一个节点；Schema 参数校验
也直接消费这些事实，不再自行遍历全部声明后仅查显式值。编辑配置与校验共用 `parameter_projection`
的字面量读取，借用显式 JSON，默认值通过 Protocol 的现有转换处理。没有新增参数模型、状态 owner
或 wire。原始文档仍由 `validate_parameter_values` 校验，隐藏字段中的非法显式值继续产生
`ParameterInvalid`，但不会触发不适用的 Schema 校验或资源读取。Analysis README 已同步当前契约。

两项回归分别覆盖输入 Schema 改变与显式覆盖/隐藏，以及默认资源缺失、恢复、依赖记录与隐藏时不读取；
均通过公开 Resolve 入口运行，并核对增量/完整结果及默认值未写回。测试协议在现有注册边界内扩展，
没有改动 Catalog 生产声明，也没有新增 UI 测试。

| 命令与范围                                                                                                         | 结果                                            |
| ------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------- |
| `pnpm test:rs:package -p yss-graph-analysis --lib`                                                                 | 38 项通过，新增 2 项                            |
| `pnpm test:rs:package -p yss-graph-editor --lib`                                                                   | 13 项通过                                       |
| `pnpm test:rs:package -p yss-graph-runtime --lib`                                                                  | 13 项通过；原有 1 项手动 benchmark 保持 ignored |
| `pnpm test:rs:package -p yss-graph-execution --lib graph_preparation`                                              | 6 项通过                                        |
| `pnpm lint:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime -p yss-graph-execution --lib` | 通过；仍有 SCI 原有 6 条警告                    |

本批共 70 项不同 Rust 用例通过；没有运行完整工作区、完整执行库或桌面交互。当前没有保留规则偏离，
没有新增 benchmark 或声称测得性能收益。选列、重命名等 Schema 求解和 ColumnOutput 仍有直接读取
显式参数的入口，后续继续核对其有效值契约；本批只关闭上述后投影校验问题，不代表全模块审查完成。

### Schema 表达式与选列类型的协议参数读取（2026-10-02）

继续核对上一批列出的入口，确认数据库来源、投影/排除列、两种参数重命名及 ColumnOutput 仍绕过协议
默认值。经注册的合法默认声明实际得到未解析 Schema 或类型；两项新增回归在旧实现上均因无法 Ready 失败。
组装/聚合求解还直接读取显式参数，合表方式与右表后缀另存了一份默认字符串。

这些入口现在复用 `parameter_projection` 的有效 JSON/文本读取，并由 Protocol 的已有显隐规则筛选适用
声明。显式值存在但类型错误时仍返回无效值或无法读取，不使用默认值掩盖错误。原有效 JSON helper 改为
借用显式值、仅为协议默认值持有转换结果；变换求解的局部参数表也借用 key 和显式值，不复制完整参数内容。
聚合、合表及生成列继续由原 resolver 求解，删除重复默认字符串，没有引入第二参数模型、持久缓存或新生产接口。

新增回归分别验证默认数据库→投影→文本/对象重命名链，以及默认选列→频数的类型与列意义；覆盖显式覆盖、
无效显式值、隐藏/恢复和输入语义变化。完整/增量快照与依赖记录一致，默认值没有写入文档。
注册辅助逻辑归到原 crate 测试模块，供新旧回归共用；没有新增 UI 测试。Analysis README 已更新当前读取契约。

| 命令与范围                                                                                                         | 结果                                                         |
| ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ |
| `pnpm test:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime --lib`                        | 40 + 13 + 13 项通过；Runtime 原有手动 benchmark 仍为 ignored |
| `pnpm test:rs:package -p yss-graph-execution --lib graph_preparation`                                              | 6 项通过                                                     |
| `pnpm lint:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime -p yss-graph-execution --lib` | 通过；仍有 SCI 原有 6 条警告                                 |

本批共 72 项不同 Rust 用例通过。没有改前端、wire 或 Catalog 生产声明，未运行完整工作区、完整执行库或
桌面交互；没有规则豁免、独立 benchmark 或实测性能收益声明。常量引用、函数依赖捕获及其跨模块消费者
仍需继续核对，不能将上述 Schema/类型读取修复视为全模块完成。

### 常量类型事实的后续消费（2026-10-02）

沿常量和函数引用检查节点投影、类型规则、Schema、剪贴板、计划准备及 Application 资源捕获。
常量类型规则原先再次从文档读取参数、解析 ConstantId 并查询常量表；节点投影已经持有该常量的共享事实。
现在正向类型和反向约束直接传入该事实的类型，移除类型规则对完整 GraphDocument 的参数依赖及失效 lint 例外。
缺失选择与失效引用的原有类型状态、缓存身份及计划准备边界保持不变，没有新增事实源。

只读核对同时确认：当前内置常量 Get、函数 Call/Entry/Return 的引用参数均为无默认值的必需显式身份；
函数捕获与语义校验共用 `direct_function_dependencies`，Application 仍负责正文 I/O 和去重捕获。
扩展协议若允许默认或条件式图引用，其 Schema、类型、剪贴板及函数捕获支持边界仍需一并核对，不能只改局部 helper。
结果缓存的输入身份还包含 Registry 和 Kernel 指纹；因此仅看节点 `semantic_fingerprint` 不足以断言协议默认值漏失效。
本批没有据此增加重复指纹或修改 Application。

| 命令与范围                                                                                                         | 结果                                                             |
| ------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-graph-analysis --lib constant`                                                        | L1：2 项现有类型/表格常量缓存回归通过                            |
| `pnpm test:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime --lib`                        | L2：40 + 13 + 13 项通过；Runtime 原有手动 benchmark 仍为 ignored |
| `pnpm test:rs:package -p yss-graph-execution --lib graph_preparation`                                              | 6 项通过                                                         |
| `pnpm lint:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime -p yss-graph-execution --lib` | 通过；仍有 SCI 原有 6 条警告                                     |

本批共 72 项不同 Rust 用例通过，行为保持的归属整理复用现有覆盖，没有新增测试或 UI 测试。
Analysis README 已同步常量事实消费，并将变换 Schema 的参数描述校正为有效参数。没有改 wire、保留规则偏离
或新增 benchmark/实测性能结论；完整工作区、桌面验收和全模块复核仍未完成。

### 目录候选、创建和剪贴板的函数签名复用（2026-10-02）

Graph Editor 的目录查询与实际创建原先分别组装声明端口和函数动态成员；编辑目录还保存另一套字符串
函数签名，在连接校验中重复解析类型。现在复用 `yss-graph-resource-contract::FunctionSignature`，删除
`CatalogFunctionParameter` / `CatalogFunctionSignature`。Application 的既有 `ProjectCatalogResources`
将已验证的签名与资源 revision 组合为编辑校验输入；编辑与剪贴板导出共用此适配，导出仍只捕获项目声明，
不新增数据库 Schema 读取。项目索引版本和应用会话的最终重验继续保留。

目录查询与创建并连接共用声明端口和函数成员候选构造，每个成员只转换一次类型并复用至回退 metadata。
新增 Runtime 回归在旧实现上实际发现：函数返回端口的回退标签为 `Numeric`，而语义投影为 `Result`；
现已统一为语义标签。该回归还覆盖两个连接方向的成员身份、顺序、类型、过期 revision 拒绝和原子撤销。
第二项 Application 回归确认，数据库 runtime 声明/Schema 不可用时，函数和数据库节点仍可基于有效项目
声明导出剪贴板。两项都是非 UI 测试。

只读核对确认既有端口的连接预检优先消费语义快照。常量类型回查用于无快照的纯 Editor 调用及同一原子
补丁中新建端口，仍有实际消费者，未将其误删。Graph application README 已同步共享签名与导出读取范围。

| 命令与范围                                                                               | 结果                                                          |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-graph-editor -p yss-graph-runtime --lib`                    | 13 + 14 项通过；Runtime 原有手动 benchmark 保持 ignored       |
| `pnpm test:rs:package -p yss-application --lib graph::`                                  | 50 项通过，包含图编辑、目录、剪贴板、运行及 Automation 消费者 |
| `pnpm lint:rs:package -p yss-graph-editor -p yss-graph-runtime -p yss-application --lib` | 通过；SCI 原有 6 条警告                                       |
| `pnpm test:ts src/tests/documentationContract.test.ts`                                   | 6 项通过                                                      |

本批共 77 项不同 Rust 用例通过，新增 2 项回归；没有修改前端或 wire，没有新增事实 owner、持久缓存、
规则豁免或 benchmark/实测性能结论。未运行完整工作区、完整 Application 库或桌面交互。
创建候选与成员分组、派生 resolver 的其余契约仍需继续核对，本批不代表 Graph 或全模块审查完成。

### 初始成员分组与函数 resolver 的编辑一致性（2026-10-02）

继续对照 Protocol、Registry、实例化、语义投影和连接预检，复现并修复三处声明消费不一致：

- Protocol 要求组内模板的单独最小数量为 0，但允许组本身具有正的最小数量。创建已按组生成完整成员，
  候选筛选却只读模板数量，导致这类合法节点从兼容目录消失。现有初始候选构造改为通过
  `member_group_for_template` 读取组下限，未分组模板继续使用自身下限；没有增加数量状态或另一套分组规则。
- 函数候选原先按 `arguments` / `results` 名称找模板，而语义解析与绑定校验按 resolver 识别成员。
  经合法注册、保持 resolver 但更改模板名的协议同样从目录消失。候选现在遍历派生模板，按 resolver
  和方向选择成员，保留实际模板身份；既有 Runtime 回归扩展后同时覆盖内置与改名模板的两个连接方向。
- 无语义快照的连接回退只按成员 ID 读取函数类型，曾允许将返回成员绑定到参数端口后连线。
  新增回归实际得到错误的 InsertConnection 补丁。现在连接回退与剪贴板导入共用
  `function_member_data_type`，同时检查派生模板、resolver、方向和成员 ID；同型但归属错误的成员被拒绝。

本批新增 2 项 Editor 回归并扩展 1 项既有 Runtime 回归，三处问题都先在旧实现实际失败，再验证修复。
覆盖正/零组下限、完整分组实例、错误成员归属及修正后连接、稳定端口 metadata、过期资源 revision 和
原子撤销；未新增 UI 单元测试。Graph application README 已同步声明读取与回退校验契约。

| 命令与范围                                                                               | 结果                                                    |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| `pnpm test:rs:package -p yss-graph-editor -p yss-graph-runtime --lib`                    | 15 + 14 项通过；Runtime 原有手动 benchmark 保持 ignored |
| `pnpm test:rs:package -p yss-application --lib graph::catalog::tests`                    | 13 项目录、连接、剪贴板及生命周期消费者回归通过         |
| `pnpm lint:rs:package -p yss-graph-editor -p yss-graph-runtime -p yss-application --lib` | 通过；SCI 原有 6 条警告                                 |
| `pnpm test:ts src/tests/documentationContract.test.ts`                                   | 6 项通过                                                |

本批共 42 项不同 Rust 用例通过，没有改动公共 API、wire、前端或资源事实 owner，没有保留规则偏离或提出
未经测量的性能结论。未运行完整工作区、完整 Application 库或桌面验收。扩展结构节点的函数正文捕获、
ABI 与 Project 引用管理仍有按固定节点 ID 识别的入口，需要跨模块继续核对；接口创建回归不证明扩展函数
执行链已完成，也不代表全项目架构审查完成。

### 成员分组数量在节点组装中的共享读取（2026-10-02）

Application 组装节点时原先只读取模板的 `PortCardinality`，忽略成员分组的上下限。合法分组内模板必须
声明为 `UserCreated { min: 0, max: None }`，因此要求 2～3 个成员的协议会被当成 0～无限个成员，错误拒绝
范围完全匹配的内核。新增一项非 UI 回归先在旧实现复现拒绝，再验证匹配契约通过，以及过窄输入或输出契约仍被拒绝。

Protocol 的 `NodeInterfaceProtocol::port_instance_bounds` 现在统一读取模板或所属分组的数量范围；
Application 将其用于逐输入模板和输出总数校验，Editor 初始候选筛选也复用此方法。声明端口仍为 1，
派生端口范围保持开放；没有新增实例状态、缓存、数量模型或依赖，也没有替代完整分组的创建、删除和共享实例身份。
Node Catalog 与 Application 的 canonical README 已同步此公开只读契约及消费者。

| 命令与范围                                                                                                                                               | 结果                                                                  |
| -------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-node-protocol -p yss-node-registry -p yss-node-catalog -p yss-graph-editor -p yss-graph-runtime --lib -- -- --format terse` | 30 + 4 + 14 + 15 + 14 项通过；Runtime 原有手动 benchmark 保持 ignored |
| `pnpm test:rs:package -p yss-application --lib session::components::tests`                                                                               | 3 项通过，包含新增分组数量回归及已有真实数值执行/能力校验             |
| `pnpm test:rs:package -p yss-application --lib graph::catalog::tests`                                                                                    | 13 项目录、连接、剪贴板及生命周期消费者回归通过                       |
| `pnpm lint:rs:package -p yss-node-protocol -p yss-graph-editor -p yss-graph-runtime -p yss-application --lib`                                            | 通过；SCI 原有 6 条警告                                               |

本批共 93 项不同 Rust 用例通过，新增 1 项测试。未运行完整工作区、完整 Application 库或桌面交互，
没有修改 wire、前端或资源事实 owner，没有规则豁免、独立 benchmark 或实测性能结论。
结构性函数节点的 Registry 注册角色与 Analysis/Project 中固定节点 ID 的消费仍需跨模块核对，
本批没有收窄扩展支持契约，也没有声称函数执行链或全模块复核完成。

### 函数注册角色与项目引用的跨模块消费（2026-10-02）

Registry 原先允许注册扩展 Call/Entry/Return，Analysis 的依赖遍历、ABI 和 Project 引用重写却按内置
节点 ID 识别。新增聚焦回归在旧实现实际失败：扩展调用的函数没有进入解析结果。现在这些消费者读取冻结
Registry 的结构角色；角色的 `target` / `function` 引用字段约定由 Registry 唯一提供。
Protocol 的 `Parameters::effective_text` 复用原有显隐和默认值契约，Analysis 文本参数、函数依赖、
派生端口和 Editor 资源绑定共用该借用读取，显式错误值不回退到默认值。

函数 resolver 仍可供只消费签名的叶节点使用，不把这种端口声明误判为函数调用。ABI 的 Entry/Return
选择、owner 校验和实际端口匹配均保留；没有以禁止扩展类型 ID 的方式绕开消费者差异。
Application 捕获函数正文、复制/重命名图及修改函数签名时，复用当前 Graph Runtime 的冻结 Registry。
Project 新增对独立 Node Registry 的依赖，但只按操作借用配置，不保存第二份注册状态，也不依赖
Catalog、Analysis 或 Runtime；Catalog 仅为 Project 测试依赖。

重命名与复制共用注册声明来处理当前文档、磁盘正文和可逆历史，适用的默认引用写成新路径的显式覆盖。
显式历史引用继续重写，隐藏的默认声明不写入文档。普通文本、输入字面量和重命名时的端口身份保持原语义。
`GraphResourceRenameRequest` 绑定重命名操作数据；历史重写从原路径及资源 move 推导目标路径，删除多余参数。
文件 lease、版本重验和事务回滚继续由既有 Project 入口负责。

本批新增 2 项非 UI 回归：Analysis 覆盖扩展三角色、默认 owner、输入/返回 ABI、错误显式引用，以及叶节点
独立使用 resolver；Application 覆盖扩展调用的默认目标、传递正文捕获与直接调用图的签名失效回执。
两项既有 Project 回归同时扩展为内置/扩展调用、默认引用、未加载文件、复制和撤销/重做覆盖。

| 命令与范围                                                                                                                                                                                    | 结果                                                                            |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-node-protocol -p yss-node-registry -p yss-node-catalog -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime -p yss-project --lib -- -- --format terse` | 30 + 4 + 14 + 41 + 15 + 14 + 48 项通过；Runtime 原有手动 benchmark 保持 ignored |
| `pnpm test:rs:package -p yss-project --lib -- -- --format terse`                                                                                                                              | 最终请求接口和引用重写修改后，48 项复验通过                                     |
| `pnpm test:rs:package -p yss-application --lib graph:: -- -- --format terse`                                                                                                                  | 51 项通过，包含图编辑、目录、剪贴板、运行及 Automation 消费者                   |
| `pnpm test:rs:package -p yss-graph-execution --lib graph_preparation`                                                                                                                         | 6 项计划准备、缓存及真实数值执行消费者回归通过                                  |
| `pnpm lint:rs:package -p yss-node-protocol -p yss-node-registry -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime -p yss-project -p yss-application --lib`                       | 编译检查通过；本批新出现的 2 条参数过多警告随后修复                             |
| `pnpm lint:rs:package -p yss-project -p yss-application --lib`                                                                                                                                | 最终接口复验通过，仅余 SCI 原有 6 条警告                                        |
| `pnpm docs:crate-dependencies` / `pnpm docs:crate-dependencies:check`                                                                                                                         | 生成并检查通过；本次生成只改变 Project 的依赖项                                 |

最终重命名请求接口调整后，`pnpm test:rs:package -p yss-application --lib graph::catalog::tests -- -- --format terse`
的 14 项目录消费者复验通过，未重复计入总数。

本批共 223 项不同 Rust 用例通过。相关 canonical README 与生成依赖图同步；没有改 wire、增加兼容层、
保留规则偏离或宣称未经测量的性能收益。未运行完整工作区、完整 Application 库或桌面交互。
函数 lowering/执行链仍遵循 Execution README 的现有未接通边界，本批不将解析和引用支持等同于可执行函数。

下一步需核对签名回执中的驻留直接调用图与传递依赖结果失效、活动发布之间的关系，尤其是中间函数未驻留时；
当前实现不足以证明这条链全部完成。Project 的整图复制仍按内置 Constant Get ID 重写常量引用，也需对照
注册的 `NodeTypingSpec::ConstantOutput` 及剪贴板消费者继续检查。全模块复核仍保持开放。

### 函数签名变更的传递失效与发布边界（2026-10-02）

继续检查上一批列出的签名发布链：驻留 Caller 调用未驻留 Custom，Custom 再调用 Nested 时，修改 Nested
签名的旧回执漏掉 Caller。先扩展既有 Application 回归，在原实现上实际得到 Caller 不在 affected 集合的失败。
结果查询原本会重验所读资源版本，因此这里确认的是主动失效和活动通知遗漏，不据此断言旧结果会作为当前结果返回。

Project 现在从原 WriterSnapshot 的驻留正文出发，按冻结 Registry 的 Call 角色遍历可达函数；
未驻留正文在既有 filesystem lease 内临时读取，不安装进 ProjectData。路径索引首次需要时扫描一次，后续读取复用；
路径去重处理重复调用和循环，不读取未被驻留图调用链触达的函数正文。直接签名消费者同时识别注册角色、函数 resolver
及函数资源参数，读取当前适用的显式值或默认值；只有实际 Call 角色形成正文依赖边。
受影响集合沿正文调用边反向传播，包含未驻留的中间函数；仅读取外层签名的节点不被内层签名变化误伤。
临时反向边随事务准备结束释放，没有新增持久依赖缓存或第二份注册状态。

Catalog 和 Analysis 原先分别声明的四个函数 resolver ID 迁到 Registry，resolver 到引用角色的映射也由它提供；
Editor、Analysis 与 Project 直接消费该声明，删除旧定义和直接调用专用的 affected helper。
签名最终提交重验原 WriterSnapshot 的 authority generation，不重新捕获较新的 authority 来接受旧依赖集合。
所有版本和发布检查成功后在锁内只更新目标函数及其 revision，移除提交阶段对整份 ProjectData 和 revision 表的重复复制；
文件提交失败、发布失败后的回滚及 operation reservation 继续由原事务入口负责。

本批新增 1 项 Project 回归：依赖捕获后发生真实图编辑时，旧 snapshot 的签名发布被拒绝，当前正文、签名和发布版本保留。
扩展的既有 Application 回归覆盖默认调用引用、未驻留中间函数、循环、独立的叶节点签名消费者、外层签名消费者排除，
并通过真实活动订阅核对驻留图收到 Changed；无关且不可读的未调用函数文件不影响事务，中间函数始终未驻留。
未新增 UI 测试。原有 Application 发布与 Execution 失效入口直接消费修正后的回执，没有新增并行通知链。

| 命令与范围                                                                                                                                                                       | 结果                                                                       |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-node-registry -p yss-node-catalog -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime -p yss-project --lib -- -- --format terse`         | 4 + 14 + 41 + 15 + 14 + 49 项通过；Runtime 原有手动 benchmark 保持 ignored |
| `pnpm test:rs:package -p yss-application --lib graph:: -- -- --format terse`                                                                                                     | 51 项通过，包含扩展后的签名与活动发布回归                                  |
| `pnpm test:rs:package -p yss-application --lib automation::resources::tests::function_signatures_and_graph_history_share_the_current_project_editing_state -- -- --format terse` | 1 项自动化签名入口回归通过                                                 |
| `pnpm lint:rs:package -p yss-node-registry -p yss-node-catalog -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime -p yss-project -p yss-application --lib`           | 通过；仅有 SCI 原有 6 条警告                                               |

本批共 189 项不同 Rust 用例通过，相关 canonical README 已同步。没有更改依赖清单、wire 或前端状态，
没有保留规则偏离或声称未经测量的性能收益；未运行完整工作区、完整 Application 库或桌面交互。
函数 lowering/执行仍遵循既有未接通边界。下一步继续核对 Project 整图复制中按内置 Constant Get ID 重写引用的逻辑，
对照 `NodeTypingSpec::ConstantOutput`、剪贴板及默认值/条件适用规则；本批检查不代表全模块审查完成。

### 整图与剪贴板复制的常量引用声明（2026-10-02）

继续核对上一批列出的常量复制入口，确认 Project 整图复制、Editor 剪贴板导出和导入各自按
`yssbi.constant.get` / `constant` 判断引用。合法注册的扩展 ConstantOutput 节点使用其他参数名时，
复制后的磁盘图仍保留原常量 ID，剪贴板也遗漏其常量。新增 Project 回归实际复现磁盘副本引用旧 ID；
扩展既有 Editor 回归实际复现导出常量数为 0，而预期为 1。

常量身份复制规则现由既有 Document Edit 层提供，按协议的 `GraphConstant` 参数声明读取引用。
Registry 原有校验已保证 ConstantOutput 指向这种参数，因此复制边界不另维护节点 ID 或类型规则表。
Project 和剪贴板共用该读取及重映射：显式隐藏引用继续保留，只有适用的默认引用会被捕获，
错误显式值不回退，普通文本不解释为常量身份。重映射先按源参数求出全部替换项，避免先写入的引用改变后续条件求值。
没有新增 crate 依赖、注册状态或文档 authority。

剪贴板导出把默认引用固化到副本参数，并按常量 ID 去重携带实际常量；源节点仍保持原参数。
参数字节预算在副本完成后检查。导入仍使用已有相同身份/内容复用、冲突换 ID、名称避重和原子撤销规则，
只是将全部已声明引用交给同一重映射；整图复制仍为所有常量分配新身份。

本批新增 1 项 Project 测试，通过公开复制事务和文件重新读取验证扩展显式/默认引用、隐藏显式值、
隐藏默认值不物化、非法与缺失引用保留、普通文本不变及源图不变。既有剪贴板用例扩展为内置及扩展节点，
验证显式/默认引用导出、源参数保持、跨图身份与名称冲突、共享常量、完整撤销和同图复制复用。
没有新增 UI 测试。

| 命令与范围                                                                                                                           | 结果                                                                 |
| ------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-project -p yss-graph-editor --lib constant -- -- --format terse`                                        | L1，1 + 2 项通过                                                     |
| `pnpm test:rs:package -p yss-graph-document-edit -p yss-graph-editor -p yss-graph-runtime -p yss-project --lib -- -- --format terse` | L2，1 + 15 + 14 + 50 项通过；Runtime 原有手动 benchmark 保持 ignored |
| `pnpm test:rs:package -p yss-application --lib graph:: -- -- --format terse`                                                         | 51 项消费者回归通过                                                  |
| `pnpm lint:rs:package -p yss-graph-document-edit -p yss-graph-editor -p yss-graph-runtime -p yss-project -p yss-application --lib`   | 通过；仅有 SCI 原有 6 条警告                                         |

本批共 131 项不同 Rust 用例通过，Graph 与 Project canonical README 已同步。
未运行完整工作区、完整 Application 库或桌面交互；没有规则豁免、benchmark 或未经测量的性能收益声明。
检查同时发现 Analysis 的 `referenced_constant` 和 Editor 的无快照常量类型回退仍只读取显式参数；
下一步需核对它们与 Schema、节点常量事实及条件适用规则的关系，不能把复制引用支持等同于整条默认常量解析链已完成。

### 默认常量的当前语义与计划物化（2026-10-02）

继续核对上一批的默认常量读取边界，确认节点常量事实、表格/组装 Schema 和 Editor 无快照类型回退仍直接读取
文档显式参数；常量类型规则也以显式字段存在性判断缺参。新增 Analysis 回归在原实现上因默认表格和标量无法完成
类型/Schema 解析而失败；扩展的既有 Editor 回归也实际复现默认文本常量被当作泛型、错误允许连接数值输入。

Analysis 现将已有 `referenced_constant(document, node, protocol)` 作为共享借用读取公开，按 ConstantOutput 声明
使用 Protocol 的 `Parameters::effective_text`。节点事实、Schema 输入指纹、表格 Schema、组装列含义和 Editor
无快照连接预检复用它，不再各自解释引用值。适用的默认值不写入文档，隐藏的显式/默认引用均不构成当前语义，
非法显式值不回退。类型规则优先使用已投影的常量类型，缺少事实时再按有效参数区分缺少选择和资源缺失。
复制时保留隐藏显式引用的 Document Edit 规则保持独立；它负责存储转换，不能替代当前语义读取。

执行准备继续消费节点常量事实。本轮同时确认它原先为每个引用节点重复转换相同常量、再覆盖同一参数句柄；
现在通过现有参数表的 vacant entry 只物化一次，节点继续绑定同一句柄，不新增常量缓存或改变计划身份。
这只是去掉明确的重复转换，没有新增 benchmark 或声称测得时间、分配量收益。

新增 1 项非 UI Analysis 回归覆盖默认表格/标量、类型与标题、下游列 Schema、常量类型变化、显式覆盖、
隐藏与恢复、非法显式值不回退、默认目标缺失与恢复。每一步都比较完整/增量语义及依赖记录，并确认默认值未写回。
既有 Editor 回归保留内置显式引用覆盖，加入扩展默认引用、隐藏的存储引用及重新显示后的连接预检。
计划准备复用既有缓存、准入和真实数值执行测试；没有新增测试来镜像 BTreeMap entry 的实现。

| 命令与范围                                                                                                                            | 结果                                                             |
| ------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| `pnpm test:rs:package -p yss-graph-analysis --lib constant_defaults_and_visibility_drive_facts_types_and_schema -- -- --format terse` | L1，先复现失败，修复后 1 项通过                                  |
| `pnpm test:rs:package -p yss-graph-editor --lib connect_rejects_a_known_type_outside_the_input_class -- -- --format terse`            | L1，先复现错误准入，修复后 1 项通过                              |
| `pnpm test:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime --lib -- -- --format terse`                      | L2，42 + 15 + 14 项通过；Runtime 原有手动 benchmark 保持 ignored |
| `pnpm test:rs:package -p yss-graph-execution --lib graph_preparation -- -- --format terse`                                            | 6 项通过；最终物化修改后复验通过，不重复计数                     |
| `pnpm test:rs:package -p yss-application --lib graph:: -- -- --format terse`                                                          | 51 项消费者回归通过                                              |
| `pnpm lint:rs:package -p yss-graph-analysis -p yss-graph-editor -p yss-graph-runtime -p yss-graph-execution -p yss-application --lib` | 通过；仅有 SCI 原有 6 条警告                                     |

本批共 128 项不同 Rust 用例通过，Analysis、Execution 与 Graph canonical README 已同步；未增加依赖、wire 或并行状态。
剩余 transform 参数读取已确认来自按协议显隐与默认值构建的临时参数集合，不能仅因局部 `get` 调用判为绕过协议。
未运行完整工作区、完整 Application 库或桌面交互，没有规则豁免。全模块复查仍需继续覆盖表中的其余入口，
下一轮转向 data-explorer 导入步骤、其 Application 用例和对话框 owner 的失败/取消生命周期；当前只读取了局部文件，尚不作完成结论。

### 导入与项目操作的进度所有权（2026-10-02）

沿 data-explorer 导入步骤核对 Application 回调与全局 UIStore，确认两类业务共用无操作身份的进度更新/结束入口。
新增两项纯 Application 编排回归均先在原实现上失败：较早的项目任务覆盖较新的导入进度；取消回调启动后继导入时，
旧取消动作立即清掉新进度。它们只调用真实用例与 owner、读取发布状态，不挂载 React 或增加 UI 渲染测试。

UIStore 的 `startProgress` 现返回操作专属句柄，更新和结束均校验当前身份。身份在开始发布前安装、在结束发布前释放，
同步订阅启动后继操作也不会被旧调用覆盖。取消动作捕获当前操作，在 `finally` 中只结束被捕获的句柄；
回调启动新任务或抛出错误均不把清理转移到其他操作。可见正文继续由原 Zustand store 单独持有，
没有增加进度队列、并行数据副本或兼容更新入口。替换进度只影响显示，旧业务任务仍按原有完成/取消合同执行。

数据导入和项目选择用例均持有自己的句柄；导入的绘制等待移入已有 `try/finally`，准备失败也会释放本次进度。
现有消费者测试改为观察公开发布状态，收尾验证进度已释放，不再依赖全局强制结束方法。
两项回归同时验证旧任务完成不影响后继进度、最新任务完成正常清理，以及项目取消结果保持一致。

已逐文件检查导入主弹窗、SQL 连接、Excel/SQLite/远程表选择、示例数据、局部步骤 hook 和 public 入口。
选择回调由 Application 将预期失败转换为错误文本；提交仍重验项目身份，弹窗由既有父子 owner 管理。
据此没有给局部 UI hook 另加通用异常流程，也没有将独立关闭子弹窗误当作撤销已提交导入。

| 命令与范围                                                                                                                                                                                                                                                                      | 结果                           |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------ |
| `pnpm test:ts src/features/application/ui/progressLifecycle.test.ts`                                                                                                                                                                                                            | L1，两项先复现失败，修复后通过 |
| `pnpm test:ts src/features/application/ui/progressLifecycle.test.ts src/features/application/dataManagement/useDatabaseManagement.test.tsx src/features/application/dataManagement/databaseImport.test.ts src/features/application/project/projectLifecycleOperations.test.tsx` | L2，4 个文件、23 项通过        |
| `pnpm check:ts`                                                                                                                                                                                                                                                                 | 通过                           |
| `pnpm lint:ts`                                                                                                                                                                                                                                                                  | 通过，仅有原有 4 条警告        |

本批没有规则豁免、benchmark 或未经测量的性能声明；未改变 Rust、wire 或持久化数据。
Features canonical README 已同步操作身份合同。自动化验证覆盖进度编排，导入对话框嵌套、真实绘制和桌面取消验收仍独立开放。
文档契约 6 项及本批文件格式检查通过。

### 命令可用性订阅与目录搜索复用（2026-10-02）

继续逐文件检查 commands 与 node-catalog 的公开入口和生产呈现，追踪菜单、活动面板、目录请求/缓存、
搜索投影及创建描述符消费者。命令可用性原先订阅整个图会话，节点正文或坐标更新也替换该引用；
现使用已有 ResourceStore 的 `useShallow` selector，只发布活动图路径和撤销、重做、保存中的标量投影。
工作台原生选择仍决定活动图，保存屏蔽规则不变，没有增加命令状态副本。

节点文档界面原先复制了 Domain 中相同的 Unicode 文本规范化流程，现直接复用 `normalizeCatalogSearchText`。
只匹配当前语言标题和别名的既有合同保留；完整创建目录的技术词、资源名及拼音索引继续由原 owner 管理。
未因为实现复用而扩大文档搜索范围。目录行的 `interactionDisabled` 实际用于搜索期间禁止折叠分类，
不能据此把可选择的搜索结果错误禁用。

修改前的历史可用性、文档界面及领域搜索 21 项基线通过；修改后 L2 选择相同三份测试及工作台菜单、
侧栏空状态、Nodes 活动面板、目录索引，共 7 个文件、32 项通过。没有新增或扩展 UI 测试。
`pnpm check:ts` 与 `pnpm lint:ts` 通过，lint 仍仅有原有 4 条警告。Features canonical README 已同步。
这两处按既有状态和规则 owner 收敛；没有规则例外、benchmark 或桌面性能测量结论。

### 常量 Details 编辑值的引用边界（2026-10-02）

沿 GraphConstantsPanel、常量字段/JSON 弹窗、转换 helper、Application 修改队列及拖拽创建入口复核，
确认父面板原先在任一常量变化后重新转换整张常量表。所有行因此得到新的编辑值对象，
而 JSON 弹窗的草稿初始化 effect 依赖该对象；无关常量更新也会走到已打开弹窗的草稿重置路径。
这是源码调用链确认的问题，本批没有把既有测试通过记作该交互的失败复现。

转换现移到按常量 ID 挂载的行，只依赖该行的类型、序列化值和表格内容引用。
Application helper 的输入也缩小为实际读取的三个字段。原 ResourceStore 安装继续用 `shareProjection`
保留未变分支，因此其他常量修改、保存状态或名称变化不重建该值；值或类型真正改变时继续重新转换。
没有新增共享缓存、可写常量副本或另一个提交入口，行卸载即释放派生值。

修改前 3 文件、4 项基线通过；修改后选择常量提交、画布投放、JSON 弹窗、函数 Details 和节点参数编辑，
共 5 个文件、26 项既有用例通过。`pnpm check:ts` 与 `pnpm lint:ts` 通过，仍仅有原有 4 条 lint 警告。
未新增或扩展 UI 测试；已打开 JSON 草稿在无关常量更新时保持、切换图后的重置，仍需桌面验收。
Features canonical README 已同步。下一步继续核对常量数字/JSON 转换失败的错误归属及其余 Details 入口，
不能据此把整个 Details 模块记为完成；本批没有规则例外或 benchmark 性能结论。

### 常量输入转换与表格单元格边界（2026-10-02）

继续核对上一批的输入错误归属，将界面中原有解析流程移到纯 Domain 入口后，新增两项非 UI 回归实际失败：
非法数字由共享转换抛出异常，而表格 JSON 中的 `1e400` 被接纳并重新序列化成 `null`。
前者绕过了原错误结果，后者静默改变输入；原来仅检查列数组及长度，未检查单元格内容。

`domain/graphConstants/valueInput` 现在统一接收字段值或 JSON 文本，复用既有 DataValue 转换并返回有限错误码。
表格新增的单元格检查采用模块级 Zod schema，保留列名、等长列、单列数列及空对象清空约定；
有限数字、布尔、文本和空值可进入提交，非有限数及嵌套对象/数组在输入边界拒绝。
没有改变 Rust wire 或重写共享数字转换；带引号的完整宽度整数继续按原转换保留精度。

四个旧界面解析入口已删除，展示文件重命名为 `constantValuePresentation` 并收回内部 helper 的导出。
标量字段和 JSON 弹窗只保存局部草稿及错误码，显示本地化错误，失败不调用保存/更新；成功仍进入原 Application 队列。
共享 Details 输入组件仅透传实际使用的错误关联属性。未增加 UI 单元测试或翻译键清单测试。

L1 两项回归先失败、后通过。L2 明确选择输入解析、共享字面量 wire、常量提交、画布投放、常量弹窗、
函数 Details 和节点参数编辑共 7 文件、30 项通过；`pnpm check:ts` 通过，lint 仍仅有原有 4 条警告。
本批遵守默认校验方案，没有规则豁免或 benchmark 性能声明；无效输入的实际错误显示与草稿保留仍需桌面验收。

### 节点端口 Details 的索引读取（2026-10-02）

逐项检查端口呈现和 Application 调用，确认面板原先在 selector 中物化全图端口、连线及节点标题，
每个端口行再筛选全量连线，并两次遍历全部端口构造选项。现有 Core 已拥有按端口维护的邻接索引，
这些扫描没有独立的状态或正确性职责。

面板现按端口 ID 挂载行，直接读取相应实体和 `pinConnections` 指向的连线，保留端点方向校验。
候选按后端决定筛选 ID，再读取这些端口与节点标题；查询关闭或刷新期间只需读取当前已连接项。
连线顺序跟随已有邻接索引，候选顺序跟随 Rust 的节点/端口顺序。旧全图扫描 helper 与透传参数已删除。
节点正文同时改为所需字段的浅订阅，坐标不会经整个 NodeData 引用更新详情。

已追踪 `useConnectionCandidates` 的项目/图版本/语义身份重验和空闲订阅，以及 `executeGraphEdit` 的失败结果转换；
没有在界面重复推导类型兼容性、构造另一个拓扑索引或添加额外异常编排。
修改前后两个 Details 文件的 8 项既有用例通过。L2 选择这两个文件、Core 图投影、候选 Service 和边操作，
共 5 文件、30 项通过，包含既有邻接重排、重定向、删除及引用保持覆盖。
类型检查和 lint 通过，仍仅有原有 4 条警告；未新增或扩展 UI 测试，也未声称测得桌面性能收益。

### 参数草稿同步与常量订阅范围（2026-10-02）

列表参数原先每次重绘序列化整个已发布数组，筛选参数则序列化包含全部列选项的编辑投影来判断是否重置草稿。
两者现在使用原发布对象/数组身份；Core 安装已有结构共享保留同值分支，实际值或筛选依据变化时仍恢复权威投影。
列表在提交时直接比较标量值，避免为了输入重绘维护序列化指纹。SemanticDomainEditor 的已有规范化及序列化
只随其投影依据变化计算，本批没有按字符串搜索结果把它一并改写。

常量参数选择器独立为原文件中的子组件，只有实际显示该编辑器时才订阅常量表；普通数字、文本、列表、
关系式和语义域参数不再因此订阅所有常量变化。未增加共享状态、缓存 owner 或新的命令入口。

两个既有参数编辑测试文件 22 项通过；随后参数 Application 用例及 Details 入口 2 文件、5 项通过。
类型检查与 lint 通过，仅有原有 4 条警告。没有新增 UI 测试；跨发布的真实草稿保持及连接选择仍需桌面验收。
Features canonical README 已同步上述契约。下一步继续核对 Details 总入口对完整资源目录的订阅，以及尚未完成的其他模块。

### Details 总入口的目标读取（2026-10-02）

原总入口无条件订阅完整 Event、Function、Database 集合及当前日志，纯解析器实际只使用当前目标的一条记录。
现在先解析目标和节点删除后的所属图回退，再按资源键、图路径、数据库 ID 读取所需字段。
函数只浅订阅输入/输出数组，名称和签名复用原 `buildFunctionResourceView`；无关资源版本和日志选择
不再通过上述宽订阅更新详情。Chart 正文仍沿用已合并的 ResourceStore 发布引用。

移除最后已无生产消费者的 `useEditorCollections`、`useFunctionCatalog`、集合类型、全目录组装和枚举入口，
同时收回对应 barrel 导出；没有构造单元素目录来兼容旧签名。两个只覆盖已删除 Core 入口的测试同步移除，
详情解析用例仅迁移输入夹具，原 Chart 订阅用例去掉过时 mock；未新增或扩展 UI 测试。

修改前详情两个文件的 6 项基线通过；修改后详情解析/订阅、单个函数组装和资源候选 4 文件、9 项通过，
函数签名提交/发布、项目元数据、详情选择政策及已有函数/日志/Chart 面板 7 文件、17 项通过。
`pnpm check:ts`、`pnpm lint:ts` 通过，仍仅有原有 4 条警告。节点删除回退和真实目标切换仍需桌面验收。

### Mind 与 Chart Details 的派生读取（2026-10-02）

继续逐文件核对剩余 Details 入口。Mind 表单原先在每次输入缓冲、busy/error 或折叠变化时重建子节点表、
后代集合和兄弟列表，并在每次添加子节点 ID 时复制已有数组。现按已发布节点数组、选中节点及语言缓存
父节点选项与兄弟顺序；临时子节点分组用本次计算独占的数组追加，保留后代排除和兄弟顺序。
这只是表单生命周期内的派生值，Mind 文档、输入缓冲、结构提交和分屏选择继续归原 owner。

Chart 数据集选择器只浅订阅数据库 ID/名称；列按当前数据库 ID 读取，编码选项只随该列引用重建。
删除对已验证记录的局部类型断言，展示列组件直接接受只读数组。空列数组仍表示已知 schema，
未读取列继续触发原元数据用例，版本与取消判断不变。没有新增可写索引、缓存框架或类型兼容性推导。

显式选择既有 Chart Details、数据库读取、图表 Store、Mind 动作、文件输入缓冲和文件快照共 6 文件、19 项通过。
类型检查与 lint 通过，仍仅有原有 4 条警告；未添加 UI 单元测试。Features 与 Document editors 的 canonical
README 已同步。此次已核对 Details 当前 37 个生产文件，包括剩余日志、文件、节点说明、数据库及共享呈现；
覆盖表把 Details 源码复核与待做的桌面验收分开记录，其他模块及全局复查仍然开放。
上述各批均遵循默认规则，没有保留规则偏离或宣称 benchmark 性能收益。

### Chart 与 Database Editor 的选择及读取边界（2026-10-02）

逐入口检查 Chart 的 4 个、Database Editor 的 12 个生产文件，并追踪预览读取/转换、分页加载、窗口初始化、
选择与导出入口。Chart 呈现继续消费原 Application 预览生命周期和共享 ChartRenderer；转换只组装显示字段，
没有复制绘图点或重复解析 wire，因此保留该实现。两个模块的源码复核与桌面验收在覆盖表中分开记录。

数据库窗口原先订阅完整元数据表，正文也订阅整条数据库声明。现通过 Core 的 `selectDatabaseNames`
只浅订阅 ID/名称，与 Chart Details 共用同一选择器；正文只浅订阅本库列和计数。URL 初选、删除后回退、
资源 revision 触发重载和分页身份检查保留。未知 schema 的空值不在每次渲染时产生新列数组。

原选择形状分别存在于 React hook 与表格 model，范围归属又在剪贴板重写。现由纯
`domain/databaseEditor/gridSelection` 统一类型和计算；React hook 只管理局部状态，AG Grid adapter
只负责事件与框架身份。已经无调用方的删除行选择转换及其导出直接删除，既有全选、键盘锚点和单元格
预览覆盖迁移到相应纯入口，删除的断言仅属于已移除功能；没有增加 UI 测试或新的共享状态 owner。

原 adapter 在每次单元格拖选时遍历整页行，并逐行调用选择更新。已核对安装中的 AG Grid 类型及实现：
使用既有 `getRowNode` 行 ID 索引和 `getSelectedNodes`，通过 `setNodesSelected` 分别批量更新选中/取消集合。
行同步 effect 只依赖行选择及页数据，可见格与表头仍调用框架刷新；没有另写渲染差分框架或行节点缓存。
鼠标释放和窗口失焦终止局部拖选，避免旧拖选跨越一次外部点击；卸载释放监听。原快捷键 hook 只认 Ctrl+a，
而表格抑制键盘行为已包含 Cmd 和大写 A；两处现复用同一个判断，文本输入排除同样合并到原键盘入口。

修改前 7 文件、17 项基线通过；修改后 L1 8 文件、19 项通过。首次类型检查指出当前 TS lib 不包含
`Object.hasOwn`/`findLast`，已使用自有属性判断和逆序遍历，未改变构建目标。最终 L2 用
`pnpm test:ts` 显式选择选择规则、预览文本、分页加载、窗口/Chart/Details 消费者、数据库读取和 Service、
ResourceStore 共 11 文件、38 项通过；`pnpm check:ts` 与 `pnpm lint:ts` 通过，仍仅有原有 4 条警告。
现有 UI 文件的用例只用于回归，其 mock/引用随接口机械调整，没有新增 UI 单元测试。
Features canonical README 已同步。此批没有规则豁免或 benchmark 性能声明；实际表格手势、TSV 复制、
全选快捷键、失焦清理、分页和图表预览仍需桌面验收，不能用上述聚焦用例替代它们。

### Project Explorer 的按资源读取与弹窗归属（2026-10-02）

逐文件检查原 21 个生产入口与呈现文件，追踪活动文档、资源选择、目录请求/拖拽、文件操作、数据库操作和
项目选择页生命周期；`useProjectActivityActions` 移至既有 Application/sidebar 后，模块保留 20 个生产文件。
没有新增共享 UI Store、资源列表或目录请求 owner。

原侧栏为每次图实体发布遍历全部图、重建诊断数量表，并把完整活动资源与该表传入每一行；
控制器的动作对象也随重命名输入重建。现在文件行按图 ID 读取一个数量，活动资源只浅订阅 ID/种类，
行接收自己的选中布尔值。动作引用与行边界复用未变 Activity 条目，删除旧数量集合 hook。
数据库行原先逐行扫描完整目录；Core 的资源目录查询按不可变目录项数组共享路径索引，
画布函数投放共用该查询，保留可用性、完整路径、种类、节点类型、候选顺序及原描述引用。
既有非 UI 投放用例补充了目录替换后读取新修订且旧目录不被污染的断言，没有新增用例或 UI 测试。

侧栏创建、重命名、删除与打开编排移回 Application，继续使用原文件/数据库动作。重命名捕获打开表单时的
项目身份，提交前重验；数据库删除在确认后重验原项目。Workbench 输入 owner 为每次打开保留独立回调身份，
旧提交的成功和失败不能关闭或写入后来打开的表单。

项目选择页原新建/删除组件跨关闭复用 busy/error 状态，旧请求可以关闭重开的弹窗；默认目录返回还会重置
已经输入的名称和路径。现把表单状态绑定到本次挂载，关闭及删除目标切换释放原表单，异步回写检查原表单仍在。
新建草稿以一次局部状态更新维护相关字段，默认目录使用最新名称，并在手动路径/目录选择后停止写回。
关闭仍只结束呈现，已提交操作继续沿既有 Application 生命周期结算；没有创建第二份项目操作状态。

基线和首轮修改各显式运行 5 文件、21 项。最终 L2 使用 `pnpm test:ts` 选择侧栏两文件、项目选择页反馈/关闭
两文件、投放模板、目录 Store、画布投放、共享目录请求、Activity 文档、文件管理/动作、数据库管理和
项目生命周期操作共 13 文件、62 项，全部通过。`pnpm check:ts`、`pnpm lint:ts` 通过，仍仅有原有 4 条警告。
相关 canonical README 已同步；呈现行为只复用已有测试，没有添加或扩展 UI 单元测试。
`pnpm test:ts src/tests/documentationContract.test.ts` 的 6 项通过；`pnpm format:check:ts` 显式选择本批
16 个现存文件，格式检查通过；`git -c core.safecrlf=false diff --check` 通过。
未运行 Rust、完整 CI、桌面构建或真实交互验收。

本批遵守两份规则，没有申请 benchmark 豁免或宣称实测加速。真实诊断指示、分屏选中、目录拖拽、
重命名迟到结果、确认期间切换项目，以及弹窗关闭/重开和默认目录慢响应仍待桌面验收。
Project Application 的列表刷新/操作并发与其余模块仍继续复核，全项目目标保持开放。

### Project Picker 操作准入与列表响应归属（2026-10-02）

继续追踪 `useProjectPicker`、Project Service、生命周期回执和进度 owner。原选择页只有部分操作设置 busy，
列表刷新、收藏、移除和删除可绕过这一状态，扫描/导入又在系统文件选择之后才设置 busy；不同操作结束时
无条件清为空闲。初次查询与当前路径注册独立运行，列表响应可以覆盖其他操作的结果。

现保留原 hook 内的局部项目列表与呈现状态，以一个同步操作句柄统一本页面实例的准入。普通动作共用
开始、错误归属、取消后刷新和结束逻辑；创建/删除保留既有回执恢复与结算流程，但复用同一个准入入口。
重复请求返回 `project_picker_busy`，不派发命令、不替换在途操作的页面错误或忙碌状态。
初次查询完成后才自动注册当前路径；等待中的 effect 在路径变化/卸载后停止，返回结果同样重验。
本地句柄不替换后端账本、Rust 注册表或全局进度句柄，也不扩展成跨窗口锁或新的业务状态 Store。

系统文件选择也纳入原操作生命周期，页面关闭后的迟到选择不再启动扫描/导入。已经提交的生命周期命令
继续通过原 owner 校验和结算；失效的页面实例不能再安装列表、错误或触发导航。页面列表、重试入口及
项目右键菜单使用同一忙碌状态，保留选择和筛选能力。Project 侧栏控制器另按项目实例清理旧菜单和重命名草稿，
原 Application 身份重验仍保留。

新增两个 Application 动作用例，测试宿主不渲染产品 UI：分别覆盖未完成刷新期间的注册表修改准入，以及
系统文件选择的忙碌归属和卸载后不派发命令。两项先在原实现实际失败：移除请求被执行、文件选择期间 busy 仍为 idle；
修复后通过，并验证操作结束后正常移除、旧请求不能污染当前错误/忙碌状态。未新增或扩展 UI 单元测试。

基线用 `pnpm test:ts` 显式选择项目生命周期操作与回执 2 文件、35 项通过。完成准入和共同执行入口后，
同范围 37 项通过；消费者补查项目选择页反馈/关闭、侧栏选择、项目加载/事件、进度和 Project Service
7 文件、30 项通过。本批共 9 文件、67 项不同用例，复用未变化的既有结果，不重复计算重跑数量。
`pnpm check:ts` 与 `pnpm lint:ts` 通过，仍仅有原有 4 条警告。Features canonical README 已同步；
文档契约 6 项、9 份变更文件格式及最终差异检查均通过。未运行 Rust、完整 CI、桌面构建或真实桌面验收。

当前页的串行准入符合原忙碌界面意图；本批没有保留规则偏离或申请 benchmark 豁免。
真实慢查询、扫描取消、文件选择期间关闭页面、按钮/菜单禁用及项目切换仍需桌面验收；
全模块源码核对和最终验收保持开放。

### Settings 消费字段与重置确认归属（2026-10-02）

逐文件读取 Settings 的 2 个呈现入口及 CSS、3 个原 Application 文件和 3 个 Core 文件，
并追踪存储/事件解析、窗口启动副作用、图表主题、菜单切换、插件主题上下文及 UIHost。
新增重置用例 owner 后上述模块共 10 个生产文件。原 Store 的分支共享、单次安装、待保存补丁、
跨窗口字段合并和原有 parser 继续保留；同步协调器按实际挂载创建，不引入无调用依据的多重启动机制。

原 `useApplicationSettings` 订阅完整设置，图表和插件只取主题也会接收 AI 或无关外观设置变化；
菜单只显示亮暗状态，却为点击动作订阅整个外观分支。现在图表/插件只订阅原只读主题预设，
菜单只读取模式，Application 动作执行时才取得当前记忆预设。启动副作用只读配色、语言和平滑滚动，
设置页移除重复的直接语言切换，统一由现有 `SettingsEffectsProvider` 应用状态变化。
没有新增主题模型、Store、通用 selector 框架或深比较。

原设置页重复编排全部/分区重置，在等待确认时没有操作准入，确认后的延续也不验证页面是否仍存在。
两条流程合入 `application/settings/useSettingsReset`；局部 ref 同步接纳操作，React state 仅表达确认/保存阶段和反馈。
确认复用 UIStore 的父子弹窗机制；UIHost 把真实设置弹窗 ID 传至用例，父弹窗关闭会取消子确认。
同一次重置从确认到持久化结束前拒绝重复提交，取消后释放；确认后已卸载的页面不再发起重置。
已提交保存仍由原 Settings Store 结算，页面卸载只停止反馈写回；未增加跨窗口重置队列。

基线显式选择设置状态、既有 Settings UI、插件、主题副作用和菜单 5 文件、21 项通过。
抽出原流程后新增 2 项 Application 操作回归，以返回 null 的 harness 调用动作、不渲染产品界面：
重复调用实际打开 2 个确认框，已确认操作在页面卸载后仍实际调用重置，两项均先失败；
准入与挂载保护补齐后通过，并覆盖取消后重试及保存期间仍保持准入。没有新增 UI 单元测试。

```powershell
pnpm test:ts src/features/application/settings/useSettingsReset.test.tsx src/modules/settings/internal/ui/SettingsView.test.tsx
pnpm test:ts src/features/core/settings/settingsStore.test.ts src/shared/types/settings/parseSettings.test.ts src/features/application/plugins/usePluginView.test.tsx src/app/providers/SettingsEffectsProvider.test.tsx src/app/windows/workbench/menuContributionRegistry.test.tsx src/app/ui/UIHost.test.tsx src/app/windows/workbench/WorkbenchComposition.test.tsx src/shared/theme/colorThemePresets.test.ts src/shared/theme/themeTokens.test.ts
pnpm test:ts src/app/ui/UIHost.test.tsx src/app/windows/workbench/WorkbenchComposition.test.tsx
```

第一组 2 文件、10 项通过；第二组首次 7 文件、20 项通过，另 2 文件、3 项暴露原有夹具失配。
UIHost 夹具补齐当前项目身份，并按现有交互确认丢弃连接草稿；窗口关闭夹具改为部分 mock，
保留真实 `collectEditorFiles` 供已存在的文件收尾流程使用。没有放宽生产 currentness 或删除原断言。
这 2 文件最终 3 项全部通过，本批累计 11 文件、33 项不同用例；复用其余未变化范围的成功输出。
`pnpm check:ts` 与 `pnpm lint:ts` 通过，后者仍为原有 4 条警告；文档契约 6 项、
15 份变更文件格式检查及 `git -c core.safecrlf=false diff --check` 均通过。

本批符合默认规则，没有 benchmark 豁免或实测加速声明。未运行 Rust、完整 CI、桌面构建或真实桌面验收。
主题/语言快速切换、跨窗口设置更新、嵌套确认焦点、设置页关闭/重开和重置反馈仍需桌面验收。
其余模块源码核对与全项目最终验收保持开放。

### Plugins 注册表投影与维护操作身份（2026-10-02）

逐文件读取 Plugins 的全部 4 个生产呈现入口、原 4 个 Application 文件、Service 与 Parser，
并追踪 App Provider、根面板注册以及 Workbench 贡献同步。新增注册表用例 owner 后 Application 为 5 文件。
复核原生成 schema 解析入口、按方法区分的宿主回复、MessagePort 绑定、视图 lease 串行释放和 backend currentness，
没有在本批重新实现 parser、传输协议、安装 authority 或后台任务账本。

原 App Provider 同时持有注册表 React state 和快照 ref，并直接查询 Service、编排安装/卸载及布局发布，
通过 Context 广播整份状态。插件页面和列表每行又分别扫描完整插件数组，任意状态变化都会扩大消费者更新范围。
现在这些用例移入 `application/plugins/pluginRegistry`，每个挂载实例只拥有一个内部 Zustand 投影；
Provider 只负责启动/停止和提供稳定接口。列表按需订阅状态，页面和条目读取同一只读 ID 索引。
查询安装时复用已有结构共享和冻结能力，按 ID 保留未变条目，一次交付列表、索引与查询状态；
同值列表不重新分配索引，也不重新触发 Activity 失效。原 Provider 用例增加同值响应的引用核对。
没有在 render/selector 中深比较、序列化或冻结整份数据。

查询、动作和排队布局回调绑定每次挂载身份。安装流程在原生选取、包检查、签名变更确认和信任确认后重验身份；
卸载确认还检查同一安装代际。已确认提交的卸载继续先移除本地该代际投影，再查询后端，
后续读取失败或旧响应不能复活该插件；不同代际的当前条目不会被旧回执直接删除。
后台已提交事务不因页面关闭而回滚，下一次挂载从 Rust 重读。语言变化通过呈现层翻译稳定错误 key，
不再因为翻译函数引用变化拆掉并重启注册表流程。

维护 hook 原来依赖整个插件对象，列表刷新后即使 ID/安装代际相同也会清空已翻页结果，
清理与翻页仅检查闭包中的 React busy，连续调用可在一次渲染内同时进入。
现在维护请求按插件 ID、安装代际和挂载绑定；页面局部 ref 同步准入，查询结果仍由局部 React state 拥有。
清理及其后续重查共用一次准入，初读/刷新/翻页都不能插入第二次操作，历史、游标、用量和诊断整体安装。
维护弹窗只为当前选中的同一安装代际挂载，关闭/替换后旧结果不回写，也不从旧清理继续发起查询。
插件 View 只在 project scope 订阅项目切换；原端口和 lease 生命周期保留。

本批新增 3 个不同的 Application 操作回归，分别保护过期信任确认、维护操作重复进入、同一安装身份的分页保留。
第三项专门检查请求身份，并在代际替换时核对旧查询不能覆盖新结果，不是前两项的重复矩阵。
三项在旧实现上分别实际观察到第二次确认、额外清理提交和额外历史查询，修复后全部通过。
维护测试的 harness 返回 null，不渲染产品界面；没有新增 UI 单元测试。

```powershell
pnpm test:ts src/app/windows/workbench/integrations/PluginProvider.test.tsx src/features/application/plugins/pluginActions.test.ts src/features/application/plugins/pluginViewActions.test.ts src/features/application/plugins/pluginViewSession.test.ts src/features/application/plugins/usePluginView.test.tsx src/services/plugins/pluginService.test.ts
pnpm test:ts src/features/application/plugins/pluginActions.test.ts src/features/application/plugins/usePluginMaintenance.test.tsx
pnpm test:ts src/app/windows/workbench/integrations/PluginProvider.test.tsx src/features/application/plugins/pluginActions.test.ts src/features/application/plugins/pluginViewActions.test.ts src/features/application/plugins/pluginViewSession.test.ts src/features/application/plugins/usePluginView.test.tsx src/features/application/plugins/usePluginMaintenance.test.tsx src/services/plugins/pluginService.test.ts src/modules/workbench/internal/application/workbenchLayoutActions.test.ts src/features/application/sidebar/useActivityPanelDocument.test.tsx src/app/windows/workbench/WorkbenchComposition.test.tsx
pnpm check:ts
pnpm lint:ts
```

基线 6 文件、9 项通过；新回归阶段 3 项先失败、1 项既有测试通过；最终 L2 为 10 文件、27 项全部通过。
TypeScript 检查和 lint 通过，仍只有原有 4 条警告。Features 当前契约已同步，文档契约 6 项、
14 份变更文件格式检查及 `git -c core.safecrlf=false diff --check` 均通过。
没有保留规则偏离、申请 benchmark 豁免或宣称实测交互加速。未运行 Rust、完整 CI、桌面构建或真实桌面验收。
真实安装/卸载、确认期间离开、维护分页与重开、插件显示/隐藏、原生文件对话框和完整计算链仍保持验收开放；
本批前端源码覆盖不证明 Plugin protocol/SDK 全模块或整个项目已完成。

### Results 展示校验与分页查询身份（2026-10-02）

逐文件读取 Results 呈现模块的 16 个生产文件，追踪 Application 加载器、分页/value/analysis 读取、
报告补选和现有 parser。原分页 hook 在 effect 中重置页码；part 或结果身份切换时，另一读取 effect
先带旧页码发出一次新查询，再读首页。页码现在绑定执行会话、结果、part、页大小和调用方总数，
在订阅及读取前重置。同值身份重建不重置，未知总数的最终页继续使用后端 `hasMore`。
查询协调器、共享 Zustand 投影和 payload lease 仍由原 owner 持有，没有第二份数据状态。

原 ResultContent 每次重绘都解析 Plot，解析后的新对象还会触发图表内部页码重置；独立窗口有自己的解析路径。
报告先在 ReportView 校验，再在 StructuredResult 重复解析展示元数据、查找路径及校验目标。
现在统一在 `loadPresentationWindow` 交付时复用现有 Plot/report parser；视图接收有类别的解析结果、
稳定图形对象及原有失败诊断。结构化章节同时携带已校验的方程文本或表引用，不在 renderer 再找路径。
原始 scalar 仍用于数值查看；数组仍为按需分页引用。移除无调用方的 Application Plot parser 转发文件，
线性报告仍检查完整结果引用，错误字段诊断和记录保留，未增加 schema、存储、迁移或兼容层。

结构化表格和稳定性根的原守卫移至现有 `structuredReportDisplay`，Application 的
`useStructuredReportTable` 在页面或章节变化时执行校验，视图只呈现已校验行。
原稳定性圆图先过滤非对象行，可能静默省略坏数据；现在整页显式失败，保留有限根及声明列约束。
报告绑定按其数据/引用复用，数值网格接收只读列和行，仅在 AG Grid 适配入口按页面引用物化外层数组，
不再由每个调用方复制全部行。假设检验公式的失败回退改为 React 文本节点，原公式不再直接插入 HTML。

本批新增 3 项不同风险的非 UI 回归：查询切换不能请求旧偏移；加载器交付已解析的 Plot 且保留无效图形状态；
结构化页中的坏行不能被过滤。第三项在添加前单独说明其风险。前两项在原实现分别观察到额外 `offset: 6`
请求和未解析载荷；第三项先抽取原过滤逻辑，实际观察到坏行被忽略，修复后通过。
分页 harness 返回 null，不渲染产品界面；已有 UI 用例只调整输入契约，没有新增 UI 单元测试。
既有报告加载用例也确认跨结果引用被拒绝、结构化对象保持引用，并继续拒绝用数列第一页拼成完整报告。

```powershell
pnpm test:ts src/features/application/results/usePagedResultRows.test.tsx src/features/application/presentation/loadPresentationWindow.test.ts src/shared/types/report/report.test.ts src/shared/types/domain/structuredReportDisplay.test.ts src/modules/results/internal/ui/info/LinearRegressionComponent.test.tsx src/modules/results/internal/ui/info/ReportView.test.tsx src/modules/results/internal/ui/panel/ResultContent.test.tsx src/features/application/presentation/usePresentationWindow.test.tsx
pnpm test:ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/results/resultLeases.test.ts src/features/application/results/resultReadErrors.test.tsx src/features/application/results/runtime.test.ts src/features/application/results/addLinearSummaryContents.test.ts src/features/application/results/components/renderers/ResultRenderers.test.tsx src/modules/results/internal/ui/panel/ResultPanel.test.tsx src/features/application/presentation/PlotResultView.test.tsx src/features/application/presentation/toResultChartModel.test.ts src/services/result/resultService.test.ts src/shared/types/dto/plotPayload.test.ts
pnpm test:ts src/modules/results/internal/ui/info/LinearRegressionComponent.test.tsx
pnpm check:ts
pnpm lint:ts
```

L1 基线 4 文件、12 项通过；完成变更后的第一组 8 文件、25 项，第二组 11 文件、50 项均通过，
累计 19 文件、75 项不同用例。公式文本回退调整后仅重跑直接消费者 1 项通过，复用其余未变化范围。
TypeScript 和 lint 检查通过，仍只有原有 4 条警告。两份 Results 当前契约已同步；
文档契约 6 项、29 份变更文件格式检查通过，交付前另行执行 `git -c core.safecrlf=false diff --check`。
本批遵守默认规则，没有 benchmark 豁免或实测绘制加速声明；未运行 Rust、完整 CI、桌面构建或真实桌面验收。
图表分页与展开失败、报告数值切换/补选、坏页提示和独立窗口租约仍需桌面验收。
本批发现的 ACF/PACF 置信带计算归属已由下批沿 SCI 和结果查询契约修复；
不能据呈现源码读取核销所有统计边界，全模块复查和最终验收继续开放。

### Results 置信带归属与租约对账确认（2026-10-02）

沿 ACF/PACF 的 SCI、Runtime、Kernel 保留结果、Application 查询、IPC、前端 parser 和视图核对数据流。
原报告组件使用 `1.96 / sqrt(n)`，SCI 的 correlogram 另算更精确的同一参考带，形成两个计算入口。
现由 `yss-sci::time_series::acf_pacf` 一次计算既有的双侧 95% 白噪声参考带，
中立 `AcfPacfResult` 增加 `ci_half_width`，SCI 相关图直接取该值；Runtime 检查有限且为正。
Kernel 和 Application 继续保留、查询同一结果；IPC 仅映射为 `ciHalfWidth`，没有新增计算服务或 backend 注入。
前端复用原 parser 校验必需的有限正数，相关图 parser 同时拒绝零和负宽度，缺失字段显式失败。
报告直接绘制这一值，按 ACF/PACF 数组引用复用滞后柱条；没有旧格式转换、猜测或兼容回退。
计算样本、滞后预算、相关系数和原有取消语义未改变。

继续读取 Results 租约控制器和面板/独立窗口调用方时，确认原对账缓存没有随 retain/release 失效。
已确认空集合后，申请租约、安装面板又在下次对账前关闭，会恢复同一空集合；旧确认使对账被跳过，
留下刚申请的后端租约。现复用原有所有权串行队列，在申请、释放和实际对账请求前撤销旧确认，
仅成功对账建立新确认。申请失效操作放在队列内部，避免被更早的在途对账回执覆盖。
FlexLayout 仍拥有面板集合，pending 仅保护正在安装的租约，没有第二份面板模型、结果存储或新队列。
后端 `ResultStore::reconcile` 明确保留待交接租约；既有测试确认主窗口对账不会提前回收独立窗口交接，
认领后按新 owner 管理，销毁任一尚未交接的端点也能释放结果。本批未修改这项后端规则。

新增 3 项不同风险的非 UI 回归：parser 必须保留后端给定宽度、IPC 必须原样交付且使用正确字段名、
快速关闭面板必须回收刚申请的租约。第三项在添加前已说明独立风险；三项均实际先失败后通过。
parser 和 wire 用 `n = 16`、宽度 `0.123` 证明透传，避免用恰好匹配公式的值掩盖重复计算。
已有 SCI/Runtime/图形测试补充数值来源及非法宽度断言；已有 UI 输入仅适配必需字段，没有新增 UI 单元测试。

```powershell
pnpm test:ts src/shared/types/report/report.test.ts src/services/result/resultService.test.ts src/shared/types/dto/plotPayload.test.ts
pnpm test:ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/results/addLinearSummaryContents.test.ts src/features/application/results/runtime.test.ts src/features/application/presentation/loadPresentationWindow.test.ts src/features/application/presentation/toResultChartModel.test.ts src/features/application/presentation/PlotResultView.test.tsx src/modules/results/internal/ui/info/LinearRegressionComponent.test.tsx src/modules/results/internal/ui/info/ReportView.test.tsx
pnpm test:ts src/features/application/results/resultLeases.test.ts src/features/application/execution/openInspectableResult.test.ts src/features/application/window/openWindowHelpers.test.ts src/features/application/results/addLinearSummaryContents.test.ts src/modules/workbench/internal/application/workbenchLayoutActions.test.ts src/features/application/presentation/usePresentationWindow.test.tsx
pnpm test:rs:package -p yss-sci -p yss-sci-runtime --lib acf_pacf
pnpm test:rs:package -p yss-application --lib ipc::schema::result::tests
pnpm test:rs:package -p yss-application --lib graph::results::report::tests
pnpm test:rs:package -p yss-sci --lib visualization::tests
pnpm test:rs:package -p yss-sci-runtime --test time_series_acf_pacf_golden
pnpm test:rs:package -p yss-node-kernel --lib acf_pacf
pnpm test:rs:package -p yss-node-kernel --lib linear_summary::tests
pnpm test:rs:package -p yss-graph-execution --lib window_handoffs_and_owner_reconciliation_do_not_leak_or_drop_claimed_results
pnpm check:ts
pnpm lint:ts
pnpm lint:rs:package -p yss-sci-contract -p yss-sci -p yss-sci-runtime -p yss-node-kernel -p yss-application --lib
```

ACF/PACF 的前端 L2 两组分别为 3 文件 28 项、8 文件 33 项；租约及消费者为 6 文件 28 项，
其中报告补选 1 项与前组重复，累计 16 文件、88 项不同用例全部通过。
Rust 上述目标合计 33 项通过，包含 53,940 观测的完整拟合结果读取、共享时间序列 golden 和窗口交接。
TypeScript 检查、前端 lint、上述 5 个 Rust package 的库 clippy 均通过，仍为原有前端 4 条、SCI 6 条警告。
五份当前模块契约已同步；文档契约 6 项、17 份前端/文档文件格式和 4 个变更 Rust package 的格式检查通过。
本批未保留规则偏离或申请 benchmark 豁免，没有实测帧率或桌面提速声明。
未运行完整 CI、桌面构建或真实原生交互；报告置信带、快速关闭/移动面板及独立窗口交接仍待桌面验收。
本批只关闭这两项明确边界，全模块源码复查和最终验收继续开放。

### Workbench 面板状态的变化检测归属（2026-10-02）

继续读取 Workbench 原生 Model 绑定、操作/投影/事务/持久化、面板 metadata 与放置规则、
RootLayoutHost、Logs 嵌套布局及 state/选择桥接入口。已读路径中，布局写入继续由 FlexLayout Model 承担，
Zustand 只发布已提交的冻结记录；中间几何动作只推进原生 mutation revision，未新增可写布局镜像。
候选 Model 的复制和验证位于复合提交或恢复边界，没有为了本批修改引入新的订阅、状态或布局抽象。
上述读取仍不代表 Workbench 全部菜单、呈现入口、默认/恢复和所有复合原生命令通知已复核完成。

`EditorPaneState` 原先对同值选择、相同折叠状态和重复清理都创建新对象并发布。
Graph 与 Mind 画布分别维护选择比较，其他命令、Details 和关闭入口绕过这些局部判断。
现由原 store 统一去重 ID、维护节点/连线互斥并比较有序 ID 数组；不变时保留整份状态和引用。
折叠只在成员关系改变时写入；面板释放在同一次 Immer 更新中移除选择和折叠，
不存在的面板及空 store 的重复清理不发布。其他面板和未变字段继续结构共享。
画布已删除自己的同值判断，仍按原流程同步 Details，避免把“状态未变”误当成“无需同步查看上下文”。
布局位置、活动分组、Graph 文档和撤销历史都不进入此 store。

本轮第四项新增回归专门保护状态通知和引用稳定性，与置信带 parser/wire 及租约泄漏三个风险不同，
添加前已说明其范围。纯状态测试在旧实现实际因等值状态被替换而失败；修复后检查未变写入不通知、
切换为连线选择清空节点、其他面板保留引用、复合释放只发布一次，以及重复释放/reset 不重复通知。
没有新增 UI 单元测试。Workbench README 同步当前选择契约，并删除 Settings 为布局持久化标签的过期描述；
当前 Settings 仍使用应用 singleton Dialog。Editor Application 与 Document editors 的 owner 文档同步对应边界。

```powershell
pnpm test:ts src/modules/workbench/internal/layout/editorPaneStateStore.test.ts src/modules/workbench/internal/state/workbenchUiStore.test.ts
pnpm test:ts src/modules/workbench/internal/layout/editorPaneStateStore.test.ts
pnpm test:ts src/modules/workbench/internal/layout/editorPaneStateStore.test.ts src/features/application/editor/revealGraphProblem.test.ts src/features/application/editor/useCanvasInteraction.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx src/features/application/editor/useEditorOperations.capabilities.test.tsx src/features/application/editor/workbenchPanelClose.test.ts src/features/application/editor/rightSidebarActions.test.ts src/features/application/editor/editorDetailPolicy.test.ts
pnpm check:ts
pnpm lint:ts
```

基线 2 文件 2 项通过；新增回归实际先失败后通过，最终 L2 为 8 文件 56 项全部通过。
类型检查和前端 lint 通过，仍为原有 4 条警告。与前述 Results 范围合计 24 文件、144 项不同前端用例，
Rust 33 项的结果保持有效；本项没有修改 Rust。没有新增依赖、规则偏离或 benchmark 豁免。
最终文档契约 6 项及本轮 24 份前端/文档变更文件格式检查通过，交付前另行执行 `git -c core.safecrlf=false diff --check`。
未运行完整 CI、桌面构建或真实交互；Graph/Mind 框选与取消恢复、Details 折叠、分屏状态隔离、
面板关闭和项目重置仍待桌面验收。全模块循环审查保持开放。

### Workbench 原生命令的完成发布与浮窗激活（2026-10-02）

本批继续完成默认布局、恢复/持久化协调器、布局命令、面板渲染器、侧栏、状态栏、拖放和架构菜单读取，
合并前批范围已逐文件检查当前 Workbench 全部 68 个生产 TS/TSX 文件。
架构图仍由静态节点、已生成的 Cargo 依赖数据及 URL 导航驱动；所列前端代表路径全部存在。
宿主、业务 contribution 与应用协调器继续沿现有入口组合，没有新增平行布局或跨领域状态 owner。
原生 Model 绑定仅供 RootLayoutHost 内部使用，删除没有消费者的公共重导出。
源码读取覆盖不等于全部桌面行为已经验收，其他模块的完整内部调用链仍在各自审查范围内。

原普通命令分别提交面板添加/标题修改和激活，订阅者会先看到新成员配旧活动目标的中间状态。
现复用 FlexLayout 原生 `Actions.group`，把同次打开、复用、移动、分屏、停靠及资源重映射的动作一次提交；
激活动作在原 Model adapter 内统一生成，原生新浮窗的选择和层级继续由库处理。
该分组只控制原生动作的发布边界，异步布局候选仍使用既有事务、基线 revision 和提交前重验。

另两处问题由实际原生模型复现：`MOVE_NODE` 的来源字段是 `fromNode`，旧逻辑误读 `node`，
移入浮窗后仍可能把旧分组的另一个面板作为活动目标；活动分组补全的嵌套修正又产生重复发布。
现读取正确来源，补全动作沿现有中间状态标记执行，仅外层动作发布最终投影，每个动作仍推进 mutation revision。
激活被最大化分组遮住的浮窗面板时，解除最大化明确传入目标浮窗 layout ID，修复原先默认作用于主布局的问题。
没有修改第三方依赖、增加另一套批处理或抑制真正的布局变化。

新增 3 项不同风险的纯 Model/状态回归均实际先失败再通过：打开或重命名复用只观察到一次完整状态、
原生跨布局移动只发布被移动面板为活动目标、最大化浮窗中的隐藏编辑器可再次显示。
第三项在添加前已说明独立可见性风险，没有新增 UI 单元测试。
Workbench README 已同步分组、原生移动字段、内部补全、浮窗激活和公共边界的当前契约。

```powershell
pnpm test:ts src/modules/workbench/internal/layout/workbenchNotifications.test.ts src/modules/workbench/internal/layout/workbenchActivation.test.ts src/modules/workbench/internal/layout/workbenchFloatingLayout.test.ts
pnpm test:ts src/modules/workbench/internal/layout/workbenchPanelModel.test.ts src/modules/workbench/internal/layout/workbenchLayoutPersistence.test.ts src/modules/workbench/internal/application/workbenchLayoutActions.test.ts src/features/application/editor/editorPanelActivation.layout.test.ts src/features/application/editor/editorPanelCloseCommands.test.ts src/features/application/editor/editorCommandFocus.test.ts src/features/application/editor/editorOpenTarget.test.ts src/features/application/editor/openFileInEditor.test.ts src/features/application/editor/openGraphInEditor.test.ts src/features/application/editor/revealGraphProblem.test.ts src/features/application/editor/pruneEditorPanels.test.ts src/features/application/editor/workbenchPanelClose.test.ts src/features/application/results/resultLeases.test.ts src/features/application/execution/openInspectableResult.test.ts src/features/application/results/addLinearSummaryContents.test.ts src/features/application/editorMutation/projectSnapshotResources.test.ts src/features/application/plugins/pluginViewActions.test.ts
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts
git -c core.safecrlf=false diff --check
```

基线 3 文件 8 项通过；修复后的聚焦检查为 3 文件 11 项，直接消费者为 17 文件 79 项，
合计 20 文件、90 项不同用例全部通过，覆盖活动编辑器、关闭、定位、项目快照、结果租约和插件面板入口。
删除公共重导出后重新执行类型检查和 lint 通过，仍只有原有 4 条警告。
文档契约 6 项通过；8 份本批源码、测试与文档文件已按根格式入口处理，交付前另行检查格式及 Git 差异。
本批遵守默认规则，没有规则偏离或 benchmark 豁免，也没有实测绘制速度声明。
本批未修改 Rust，未运行完整 CI、桌面构建或真实原生交互；浮窗拖动/分屏/停靠、最大化切换、
恢复和项目切换时的内容/焦点保留继续等待桌面验收，全模块循环审查保持开放。

### Logs 领域读取与订阅范围（2026-10-02）

已逐文件读取 Logs 原有全部 16 个生产呈现入口，以及 Application log、Service、领域定义、DTO parser、
record receiver/subscription 和配置入口；控制器归回 Application 后，当前呈现模块为 15 个、Application 为 6 个。
对照日志插件 README 与命令适配器，确认记录提交、recent/live 订阅和后端历史接口的边界。
此项不等于插件 collector、SQLite 和平台生命周期的全部内部源码已读完，相关范围继续开放。

原 `LogWorkspaceContext` 广播整批记录、筛选、选择和订阅状态；新记录使工具栏等无关消费者更新，
列表和标题栏各自扫描同一记录数组。现由原 recent buffer 的 Zustand owner 同批发布记录和领域索引，
`all` 直接引用原数组，其他领域只持有该有界缓冲中的记录引用，没有增加可写日志模型。
同一 stream 追加时仅统计被裁剪前缀和新增记录，更新受影响领域，未变领域直接保留引用；
完整快照替换与清空同步重建所需索引，不留下旧 stream 或已被容量裁掉的记录。
截断、去重、排序、水位与后端订阅恢复的原有规则未改变。

`useLiveLogs` 复用既有 Zustand 选择订阅桥接，列表、计数、首次加载和截断状态各取实际所需字段。
领域列表读取自身索引；原 `logStore` 的筛选函数按不可变数组、筛选对象与领域缓存，
列表和计数消费者共享输出，输入变动使对应查询重新计算。弱引用键避免缓存延长旧输入生命周期，
选择、搜索和自动滚动的同值写入不发布。未添加手写订阅框架或另一个查询 Store。

原 workspace controller 移至 Application，保留订阅挂载/释放、刷新、清空和选择日志联动 Details 的职责；
操作时从原 owner 读取当前自动滚动设置与查看上下文。Context 仅提供稳定操作、低频连接状态和刷新 token，
不再携带日志数组、筛选或选择；标题计数独立订阅，工具栏不接收日志批次。主窗口和独立窗口复用既有虚拟列表，
删除一路透传但从未被使用的 presentation 参数；既有 UI 用例只移除这一输入，没有新增 UI 单元测试。
Application 公共入口移除已经没有外部消费者的缓冲写入、内部订阅及筛选重导出。

两项新增非 UI 检查在旧实现实际失败：领域索引原先不存在，相同筛选输入每次产生不同数组。
修复后验证批次的一次完整发布、未变化领域引用、容量裁剪、大批次替换、换 stream、清空保留水位，
以及列表/计数共享结果和筛选/输入变化后的正确失效。Logs README 同步当前 owner 和订阅契约；
插件 README 修正前端 Service 已封装历史查询/统计的过期说明，Rust 命令及实现没有修改。

```powershell
pnpm test:ts src/features/application/log/logBuffer.test.ts src/features/application/log/logStore.test.ts
pnpm test:ts src/features/application/log/logBuffer.test.ts src/features/application/log/logStore.test.ts src/services/log/logService.test.ts src/services/ipc/recordBatchReceiver.test.ts src/modules/logs/internal/ui/logPanelScroll.test.ts src/modules/logs/internal/ui/logPanelViewport.test.ts src/modules/logs/internal/ui/useLogPanelVirtualList.test.tsx src/modules/logs/internal/ui/LogItemRow.test.tsx src/modules/details/internal/ui/panels/LogDetailPanel.test.tsx src/modules/details/internal/ui/useDetailPanelModel.test.tsx src/features/application/editor/rightSidebarActions.test.ts
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts
git -c core.safecrlf=false diff --check
```

基线为 2 文件 7 项，修复后聚焦 2 文件 9 项及直接消费者 9 文件 23 项均通过；最终合并复验 11 文件 32 项全部通过。
类型检查、前端 lint 及文档契约 6 项通过，lint 仍只有原有 4 条警告；本批 20 份变更文件的格式与 Git 差异另行检查。
本批未保留规则偏离或申请 benchmark 豁免，没有实测帧率提升声明。
未运行 Rust、完整 CI、桌面构建或真实原生交互；多领域分屏、高频日志下的筛选/选择、Details 联动、
滚动锁定/恢复、断流重订阅和主/独立窗口仍需桌面验收，全模块循环审查保持开放。

### 日志插件的终止交付与订阅释放（2026-10-02）

继续逐文件读取日志插件全部 17 个生产 Rust 文件、两个独立测试文件、build/权限和 Cargo 入口，
覆盖 collector 捕获/脱敏/console、SQLite、dispatcher、订阅 worker、运行时和命令适配器。
插件不依赖其他仓库 crate；桌面仅通过 `init` 安装，前端消费者沿 LogService、原 parser、receiver、
subscription 和 Application hook 接收。记录与存储仍由插件唯一拥有，业务诊断不依赖这条允许丢失的观测链。

原普通队列满时直接移除订阅，没有告知前端；写入失败又把终止通知投向同一条可能已满的队列，
即使成功交付也保留 worker 与 sink。现由原 `BoundedWorker` 持有一个独立、只写一次的终止位置，
dispatcher 非阻塞标记结束并移除订阅。当前成功回调返回后丢弃积压数据，交付一次终止通知并退出；
空闲线程沿原队列唤醒，拒收、panic 或已经断开的 sink 仍独立结束，不阻塞派发线程。
关闭或永久阻塞的 sink 无法保证送达，未将允许丢失的日志通道改称可靠交付。

`LogStreamFailure` 统一表达 `storage_unavailable` 和 `subscriber_lagged`，Rust 枚举拥有 wire 值，
前端共享类型与既有 parser 同步接纳且只允许空记录终止批次。receiver 保留明确原因，
subscription 沿原入口释放 Channel，Application 继续原有有界重订阅和 recent snapshot 恢复。
未提交记录仍不进入 ring 或普通批次；没有增加并行日志模型、重试框架、迁移格式或扩大普通队列。
插件和 Logs README 已同步交付边界。

扩展两项现有 Rust 回归，旧实现分别因缺少终止消息、存储失败后仍持有 sink 而实际失败；
新的一项服务层回归先复现 `subscriber_lagged` 被误认作无效批次，再验证原因、单次清理、
迟到旧 Channel 消息丢弃和新快照后的继续接收。满队列回归同时确认健康订阅继续推进、
慢订阅不交付积压数据且可从水位 3 的快照恢复。没有新增 UI 单元测试。

```powershell
pnpm test:rs:package -p tauri-plugin-tracing --lib
pnpm lint:rs:package -p tauri-plugin-tracing --lib
pnpm check:rs:package -p yssbi --lib
pnpm test:ts src/services/log/logService.test.ts src/services/ipc/recordBatchReceiver.test.ts
pnpm test:ts src/features/application/log/logBuffer.test.ts src/features/application/log/logStore.test.ts src/tests/documentationContract.test.ts
pnpm check:ts
pnpm lint:ts
```

修复后 Rust 23 项、前端 Service/receiver 9 项、buffer/store 9 项和文档契约 6 项全部通过；
类型检查及前端 lint 通过，仍只有原有 4 条警告。Rust clippy 通过；首次附加 `-D warnings`
因脚本转发未保留参数分隔符而未执行检查，改用上列根脚本命令后正常完成。
桌面组装消费者 `yssbi --lib` 编译检查通过；插件 Rust 格式及本批 8 份前端/文档文件格式通过，
交付前单独检查 Git 差异。本项编译检查不代表运行了桌面应用。
本批遵守现有规则，没有申请 benchmark 豁免。完整 CI、桌面构建和真实 Webview 慢消费者/存储故障恢复未运行，
不能用本批聚焦验证替代原生验收或全模块复核。

### Assistant 呈现订阅、初始化重放与 Channel 清理（2026-10-02）

逐文件读取 Assistant 全部 5 个生产呈现 TS/TSX 文件、CSS，及 5 个 Application 文件和 Service/Parser。
投影仍由原内部 Zustand owner 统一发布；消息 parts 是工具、引用、任务和计划的只读呈现来源，
Context 继续传稳定动作与读取接口。工具分组改为一次扫描自身范围，只选择调用/运行/异常计数及
首个活动工具名，复用 `useShallow` 保留未变摘要，避免正文 text delta 使整组因完整内容数组而更新。
工具卡片本身继续接收原位置的参数与终态，未新增平行工具表或 Store。

Assistant 删除本地的重复链接组件，直接使用共享 Markdown 组件中的 `MarkdownLink`，
通过原 `MarkdownLinkContext` 传稳定的系统浏览器打开动作，保留原失败弹窗。
地址解析、页面片段定位、点击与中键默认导航处理现在共用原 owner；Doc 仍由 Workbench 提供参考页策略。
这项改动没有新增 UI 单元测试，实际中键、片段、窄面板和流式 Markdown 仍需桌面验收。

会话初始化直接订阅并回放持久事件；订阅接通前不开放发送，
同一 generation 贯穿回放与订阅确认，迟到回调不能覆盖当前会话状态。
重连中的替换订阅再次缺号或解析失败时，原 session owner 使该 stream generation 失效，
进入错误状态并释放持有/迟到的订阅；发送还要求当前订阅存在。显式重载沿原入口恢复，未新建重试框架。

Service 的 HMR 与显式退订共用同一幂等清理入口，关闭后的 Channel 回调立即忽略；
HMR 先于订阅应答时，迟到 ID 被退订且请求被拒绝，不再交付一个已关闭的订阅。
远端退订命令留在 Service 内部，外部仅持有订阅自身的释放操作。

纯 Application 回归覆盖订阅和回放完成前禁止发送，以及缺号的重放不能恢复 ready。
Service 回归验证拒绝 HMR 关闭后的迟到应答、单次远端清理及关闭后回调失效，
没有 UI 测试或新依赖。

```powershell
pnpm test:ts src/services/assistant/harnessService.test.ts src/services/assistant/harnessContract.test.ts src/features/application/assistant/assistantHarnessSession.test.ts src/features/application/assistant/assistantHarnessProjection.test.ts src/features/application/assistant/assistantMessageContent.test.ts src/features/application/assistant/assistantHarnessRuntime.test.tsx
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts
git -c core.safecrlf=false diff --check
```

最终 6 文件 15 项通过，覆盖 parser、Service、session、消息归约及真实 assistant-ui ExternalStore 适配。
类型检查和 lint 通过，保留原有 4 条警告；检查中发现测试 helper 使用了当前 TS lib 未提供的
`Promise.withResolvers`，已改为本地 Promise 构造并重新验证通过，没有扩大编译目标。
Harness Core 与 Document Editor 的 README 同步当前契约；文档契约 6 项及本批 10 份变更文件格式检查通过，
交付前单独完成 Git 差异检查。
本项未改变 Rust 实现，沿用本轮日志插件和桌面组装检查的有效结果，没有再运行完整 CI 或桌面构建。
未声明性能 benchmark 豁免或实测帧率改善。

同期完整读取 Document Editor 的 6 个生产 TS/TSX 入口、6 个直接输入/命令 Application 文件、
共享 Markdown 链与 PDF Service，确认 React Flow 的视口、测量和手势留在单个面板，
Rust 文档、原输入缓冲及 ResourceStore 保持各自职责。其 Mind 节点数组在选择/测量变化时仍重建元素，
引用复用在下一批中修复并验证；没有把源码已读完计为全部桌面交互已完成。

### Mind 节点引用、尺寸生命周期与画布视口（2026-10-02）

Mind 原先在文档/折叠变化时重新创建所有布局节点和连线，选择或测量变化再对整个节点数组
叠加新对象；独立尺寸表保留已折叠/删除节点的尺寸。视口每帧写入 React state 后通过受控
属性回送 React Flow，连带重新创建画布回调。

复用 Graph 已有按 ID 更新的节点视图投影器，将其移至 `shared/ui/flowNodeViewProjector.ts`，
两种画布各自持有实例。Mind 的基础树投影继续归 `mindProjection.ts`，按布局、标签和拓扑的
实际变化复用节点、位置、data、连线、数组及可见 ID 集合；选择和测量不重新计算树布局。
尺寸保留在原视图条目中，折叠或删除时随条目释放，迟到测量不恢复已移除条目。
Mind 通过原 React Flow `setNodes` action 一批发布，未引入第二份文档或 Zustand store。

视口使用 React Flow 自身 transform，手势 ref 保留接受值和起点，不再经 React state 逐帧发布。
将 Graph 已有的 transform/D3 恢复步骤提至共享 `synchronizeFlowViewport`，Mind 导航、取消和
失效手势复用它；事件回调及画布配置保持稳定。已核对当前安装库的回调顺序：`onViewportChange`
先于非受控 transform 写入，故 Mind 在后续 `onMove` 处理接受/恢复，避免恢复被同帧库写入覆盖。
Graph 的 viewport session、手势 lease 和拖动提交行为保持原 owner。Mind 框选后续回调及松键重验
捕获的编辑器目标，项目、面板或分组变化后走原取消入口，避免继续接受旧手势事件。

新增两项纯模型测试：标签修改保持未变分支/拓扑引用；Mind 与共享视图的选择、测量及折叠释放。
第一项在旧布局上实际失败于未变根节点引用，修复后通过；第二项直接验证复用投影器的集成，
没有新增 UI 测试。提取前 Graph 原有 2 文件 10 项检查通过，提取后继续通过。

```powershell
pnpm test:ts src/modules/document-editor/internal/mindProjection.test.ts src/modules/graph-editor/internal/ui/Canvas/core/graphFlowModel.test.ts src/modules/graph-editor/internal/ui/Canvas/core/GraphFlowCanvas.test.tsx src/modules/workbench/internal/layout/editorPaneStateStore.test.ts src/features/application/resource/mindActions.test.ts src/features/application/editor/rightSidebarActions.test.ts src/features/application/editor/editorCommandFocus.test.ts
pnpm check:ts
pnpm lint:ts
```

最终 7 文件 36 项通过，覆盖两种画布、Pane 选择、Mind 命令、Details 同步与焦点准入；类型检查
通过，lint 保留原有 4 条警告。规则默认方案得到复用，没有申请 benchmark 豁免，也没有将模型
引用断言计为实测帧率提升。Graph 内部文件的后续复核记录在下一节；桌面缩放、框选、折叠测量、快捷键和
Escape/切换面板取消验收仍开放。

### Graph 呈现订阅、覆盖层引用与按图结果搜索（2026-10-02）

完整读取当前 Graph Editor 的 32 个生产 TS/TSX/CSS 文件，覆盖公开入口、画布适配、节点/端口、
连线、菜单、目录与覆盖层；同时读取 `useEditorCanvas`、结果搜索和原 Results 读取入口。
节点/端口继续按 ID 使用原实体、诊断、结果及手势投影，React Flow 暂态不写回文档。

编辑器外壳原先订阅完整文档标记，工具栏订阅完整运行展示记录；现在用 `useShallow` 只交付
实际消费的可用/冲突状态、运行状态和按钮能力。`useEditorCanvas` 的图身份与选择分开复用，
选择改变不再重建未变化的 `activeGraph`；覆盖层通过既有 React memo 边界隔离，自己的结果、
运行和语言订阅保持生效。没有改变操作准入、保存锁或执行流程。

原结果搜索在每个面板读取全部图的当前 descriptor，并因任意本图实体变化重建所有搜索条目。
现在 Results 的原只读发布入口在校验当前绑定后派生按图索引，按节点/端口 ID 读取现有标签，
统一发布稳定的搜索条目；同图分屏共享同一数组，消费者只过滤自己的查询文本。搜索缓存无独立
写入接口，失效绑定随原发布立即移除，没有有效绑定的图释放缓存；已持有报告继续沿原查询/租约
生命周期。旧的全局 descriptor hook 和无缓存收集入口已由实际使用的新读取路径取代。

本批第三项新增纯模型回归对应标签/位置变化的引用复用及移除风险，旧派生方式实际失败于未变
数组身份，修复后通过。第四项新增 Application 回归在添加前单独说明按图隔离和原子失效风险：
通过真实 owner、查询协调器和图发布，验证另一图更新保留本图数组，本图失效在一次发布中清空
搜索读取，另一图及先前快照不变。测试仅将 React 订阅边界替换为同步 snapshot 读取，没有挂载 UI。

```powershell
pnpm test:ts src/features/core/execution/graphRunArtifacts.test.ts src/features/application/editor/useVisibleGraphPanel.test.tsx src/features/application/editor/synchronizeVisibleGraphPanel.test.ts src/features/application/editor/useCanvasInteraction.test.tsx src/features/application/editor/useCanvasMutationHandlers.test.ts
pnpm test:ts src/features/application/results/pinResultSearch.test.ts src/features/application/results/runtime.test.ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/results/graphPresentation.test.ts
pnpm check:ts
pnpm lint:ts
```

画布命令/运行能力 5 文件 12 项通过；Results 4 文件最终 24 项通过（新增 owner 回归后，单独
复跑 runtime 的 13 项，其余新鲜结果复用）。与 Mind 批次合计 16 文件 72 项，未把重复运行的
用例重复计数。类型检查通过，lint 沿用原有 4 条警告。没有新增 UI 测试、依赖或规则偏离。
仍需桌面验证框选、取消、目录菜单、输入、执行工具栏、标签/结果失效和同图/跨图分屏搜索。
本批没有 Rust 代码变更，未运行完整 CI、桌面构建或实测帧率 benchmark；其余 Core/Application、
Rust 内部模块和完整原生验收继续按总清单推进。

本批收尾的 `pnpm check:ts`、`pnpm lint:ts`、文档契约 6 项、23 份变更文件格式检查和单独的
Git 差异检查均通过。文档与格式检查命令如下；共享发布测试的通过不替代桌面人工验收。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts src/shared/ui/flowNodeViewProjector.ts src/shared/ui/flowCanvasInteraction.ts src/modules/document-editor/internal/mindProjection.ts src/modules/document-editor/internal/mindProjection.test.ts src/modules/document-editor/internal/MindEditor.tsx src/modules/graph-editor/internal/ui/Canvas/core/graphFlowModel.ts src/modules/graph-editor/internal/ui/Canvas/core/useGraphFlowNodes.ts src/modules/graph-editor/internal/ui/Canvas/core/GraphFlowCanvas.tsx src/modules/graph-editor/internal/ui/Canvas/core/GraphDocumentEditor.tsx src/modules/graph-editor/internal/ui/Canvas/overlays/CanvasExecutionToolbar.tsx src/modules/graph-editor/internal/ui/Canvas/overlays/CanvasOverlays.tsx src/features/application/editor/useEditorCanvas.ts src/features/application/execution/usePinResultSearch.ts src/features/application/results/pinResultSearch.ts src/features/application/results/pinResultSearch.test.ts src/features/application/results/resultProjection.ts src/features/application/results/runtime.ts src/features/application/results/runtime.test.ts src/modules/document-editor/README.md src/modules/graph-editor/README.md src/features/application/editor/README.md src/features/application/results/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git -c core.safecrlf=false diff --check
```

### Execution 读取与 Results 展示的结构共享（2026-10-02）

本批完整读取 Core/state 的 2 个、Core/execution 的 7 个生产文件，以及 Graph 当前运行身份读取、
Results 展示汇总和按图读取入口，追踪全部 `shareProjection` 调用方。内部读取已经消费不可变
owner 引用，却仍用通用递归合并重新寻找相等分支；生命周期辅助写入还会在重复 unknown/清空时
创建新图与 Store 对象。新增的重复写入回归在原实现上实际失败，修复后通过。

Execution 的生命周期字段现在复用原 owner 的一次 Immer 更新；相同字段不会再次通知，提交新
请求仍创建新身份并撤销旧回调。读取投影按图比较状态、运行 ID 和经过当前语义身份过滤的失败
引用，新增、变化和删除用一次 Immer 更新完成。只更新 outputRuns 时保留公开图记录及整个读取
快照；无关图和已发布快照不变，失败信息直接共享 owner 的不可变对象。

Results 继续按原 outputs/connections 与 pending 范围派生有效性汇总；节点行和输出连线组用浅
比较，所有变化用一次 Immer 更新。运行节点集合与最终组合字段只做浅比较，不再递归重建失败
对象或汇总分支。已有宽标量表的发布冻结、按图缓存、语义/会话/revision 过滤和租约规则保持。
复用当前 Zustand、Immer 和冻结入口，没有新增业务 Store、通用更新引擎或依赖。

本批共新增 2 项纯状态/读取回归：重复生命周期写入不发布，以及输出归属变化、语义失效与图
释放后的读取稳定性。后者只替换 React 订阅边界为同步快照读取，实际 owner 和投影仍参与执行，
没有挂载 UI。既有 Results 汇总、终态、输出失效和查询协调器检查继续通过。

```powershell
pnpm test:ts src/features/core/execution/useExecutionStore.lifecycle.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/application/results/graphPresentation.test.ts src/features/application/results/runtime.test.ts
pnpm test:ts src/features/core/execution/read.test.ts src/features/core/execution/graphRunArtifacts.test.ts src/features/application/editor/observeGraphRunEvent.test.ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/results/graphPresentation.test.ts
pnpm check:ts
pnpm lint:ts
```

功能检查共 8 个不同文件、34 项通过，未重复计数两次执行的 graphPresentation 用例；类型检查
通过，lint 为原有 4 条警告。Features 和 Results canonical README 已同步当前共享契约。
本批没有保留规则偏离，也未执行或声称性能 benchmark、完整 CI、Rust 检查和桌面验收。
Graph 外部快照与资源/目录候选中的剩余 `shareProjection` 路径继续开放复核；Workbench 提交见下节。
Results 按图运行输出扫描的输入范围也仍需检查，不能据此宣称共享读取层或全部模块已合规。

### Workbench 提交投影的有限字段共享（2026-10-02）

继续读取当前 Workbench 规则、README、提交投影、Model 的面板/分组/边栏查询、metadata 类型与
校验，以及通知测试。原递归合并已移除，现有提交入口在一次 Immer 中按面板 ID 更新、删除记录，
按 group ID 复用分组、位置及成员 ID 数组；边栏只比较公开的标量状态。分组顺序仍来自 Model。
Result metadata 仅对结果引用与展示类型这两个对象作浅比较，其余 metadata 和面板字段同样用
浅比较。整个模型替换为相同内容时保留读取引用，真实 metadata 变化继续触发成员订阅。

原生 FlexLayout 仍是布局唯一写入 owner；没有新增布局状态、动作影响清单或订阅框架。持久化
继续接收每次提交，尺寸与中间手势的通知边界、hydration、关闭提交和异常隔离沿用原实现。
第三项新增纯 Model 回归在添加前说明独立风险：相同模型替换不得刷新结果面板；结果引用/租约
变更及移除必须通知成员消费者，并保留未变的展示类型、位置、分组和先前快照。没有新增 UI 测试。

```powershell
pnpm test:ts src/modules/workbench/internal/layout/workbenchNotifications.test.ts src/modules/workbench/internal/layout/workbenchActivation.test.ts src/modules/workbench/internal/layout/workbenchFloatingLayout.test.ts
pnpm test:ts src/modules/workbench/internal/application/workbenchLayoutActions.test.ts src/modules/workbench/internal/layout/workbenchLayoutPersistence.test.ts src/features/application/editor/workbenchPanelClose.test.ts src/features/application/editor/editorPanelActivation.layout.test.ts src/features/application/project/projectWorkbenchLifecycle.test.ts src/features/application/results/resultLeases.test.ts src/features/application/editor/synchronizeVisibleGraphPanel.test.ts
pnpm check:ts
pnpm lint:ts
```

变更前 3 文件 11 项基线通过；变更后同一范围新增回归后 12 项通过，直接消费者另 7 文件 44 项
通过，共 10 文件 56 项。与本轮 Execution/Results 合计 18 文件 90 项，类型检查通过，lint 仍为
原有 4 条警告。Workbench canonical README 已同步。没有保留规则例外或声明性能收益。
实际桌面拖动、浮窗、布局恢复和结果面板租约验收仍开放；Logs 子布局的快照比较以及其他外部
安装路径需继续复核，本批不将仍存在的递归比较默认为合规。

本轮收尾验证：文档契约 6 项、11 份变更文件格式检查和单独的 Git 差异检查。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts src/features/core/execution/read.ts src/features/core/execution/read.test.ts src/features/core/execution/useExecutionStore.ts src/features/core/execution/useExecutionStore.lifecycle.test.ts src/features/application/results/graphPresentation.ts src/features/README.md src/features/application/results/README.md src/modules/workbench/internal/layout/workbenchLayoutProjection.ts src/modules/workbench/internal/layout/workbenchNotifications.test.ts src/modules/workbench/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git -c core.safecrlf=false diff --check
```

### 并行复核资源安装、数据库、插件和 Graph 固定字段（2026-10-02）

本批由主代理处理 ResourceStore 的 Chart/Mind/Doc 安装，三个子代理分别处理数据库候选、插件
注册表和 Graph 共享路径，再统一集成与交叉审查。所有写入范围分开，未覆盖其他工作树改动。

Chart 与文件快照的有限结构共享归 `core/resource/resourceDocumentProjection`，复用原 Chart
保存内容比较，按 encodings、文件头和版本浅比较；Mind 按节点 ID 与 reference 字段共享。
新增纯状态回归实际复现旧实现按数组位置比较导致重排后的未变节点丢失引用，修复后通过。
完整文件表的变化和删除仍在原 ResourceStore 的一次 Immer 中发布，版本、重命名、读取令牌及
资源标记由原 owner 处理。交叉审查确认 reference 删除不会被旧值补回，没有新增文件模型或写入 owner。

数据库声明候选改用一次 Immer 增删与更新记录，移除逐记录再整表的两层递归合并。
`core/database/databaseProjection.updateDatabaseColumns` 在调用方已有的事务中按具体列及语义字段更新，
ResourceStore 元数据入口共用此函数；新增/变化的外部分支隔离复制，未变列、numeric 和语义值共享。
两项输入隔离回归均先在旧实现上失败：首次接纳的 columns 仍受外部数组修改影响，部分更新的语义
值也可被调用方继续改写。当前已验证 engine 只有空 dataset 配置，首次复制后可共享；资源存在性、
revision、索引成员检查和元数据失效规则保持。列目前仍按数组位置共享，重排按列身份共享留待复核。

插件注册表移除通用递归合并。同一 packageDigest 复用已签名的完整 manifest，依据为 Rust
`package::inspect` 对 manifest、keyId 与 files 的共同摘要，以及 registry 的原子安装和 list 读取。
enable/disable 可以推进安装代际，运行状态和有效 grant 也能独立变化，因而这些字段继续接纳。
换包按具体协议、预算、列表和贡献行浅比较，列表/索引/状态仍一次发布。新增两个纯 Application
用例分别保护运行/授权/代际变化，以及换包字段/贡献/可选字段删除；未新增 UI 测试。

Graph 的 7 处固定字段路径复用原 `sameFields`：参数显示、节点位置/显示/能力、编辑版本、
阻断连线标量索引，以及端点已按端口键共享的连线记录。没有新增 helper 或绕过完整投影校验。
Pin、参数与递归值仍需内容共享，不能直接改成借用输入：外层数组按位置比较，内层按 ID/key
恢复重排后的实体引用。普通位置 delta 的未变分支已有 Object.is 短路，当前重复成本包含顶层扫描，
不能表述为每次深递归全部节点。成功验证且完成发布冻结的缓存仍要求候选级跨实体检查。

定向验证命令与结果如下，各范围有重叠，不相加冒充不同用例总数：

```powershell
pnpm test:ts src/features/core/resource/fileSnapshots.test.ts src/features/core/chart/chartDocumentStore.test.ts src/features/application/editorMutation/projectSnapshotResources.test.ts
pnpm test:ts src/features/application/resource/mindActions.test.ts src/features/application/resource/docActions.test.ts src/features/application/resource/fileManagement.test.ts src/features/application/resource/fileTextInput.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/core/state/readonlySnapshot.test.ts
pnpm test:ts src/features/core/resource/resourceStore.test.ts src/features/application/dataManagement/databaseRead.test.ts src/features/application/editorMutation/projectSnapshotMetadata.test.ts src/features/core/dataStore/graphProjection.test.ts src/features/application/editor/observeGraphRunEvent.test.ts
pnpm test:ts src/features/application/resource/createFileActions.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/chart/chartViewActions.test.ts src/features/core/resource/resourceStore.test.ts src/features/application/dataManagement/databaseImport.test.ts src/features/application/dataManagement/databaseRead.test.ts
pnpm test:ts src/features/application/dataManagement/databaseRecords.test.ts src/features/application/editorMutation/projectSnapshotMetadata.test.ts src/features/application/project/projectHydration.test.ts
pnpm test:ts src/features/application/dataManagement/databaseRead.test.ts src/features/application/dataManagement/databaseMutation.test.ts src/features/core/resource/resourceStore.test.ts
pnpm test:ts src/features/application/plugins/pluginRegistry.test.ts src/app/windows/workbench/integrations/PluginProvider.test.tsx src/features/application/plugins/pluginViewSession.test.ts src/features/application/plugins/usePluginView.test.tsx src/features/application/plugins/usePluginMaintenance.test.tsx src/services/plugins/pluginService.test.ts
pnpm test:ts src/features/core/dataStore/graphProjection.test.ts src/features/core/dataStore/nodeView.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectSnapshotMetadata.test.ts
pnpm check:ts
pnpm lint:ts
```

上列功能范围依次为 3 文件 11 项、7 文件 25 项、5 文件 19 项、6 文件 21 项、3 文件 10 项、
3 文件 9 项、6 文件 10 项、4 文件 15 项，均通过。集成检查最初还传入两个已不存在的测试路径，
实际只匹配第三行这 5 个文件，未将不存在路径计为通过。数据库 helper 接入时发现 readonly 值数组
需要声明真实 Immer Draft；修正 recipe 类型后统一 TypeScript 检查通过。lint 仍为原有 4 条警告。
数据库分支的早期 ResourceStore 结果已由接入 helper 后的消费者检查覆盖；未重复运行未变化的范围。

Features README 已同步当前契约。没有保留规则偏离或声明性能收益；本批未运行 Rust、完整 CI 或
真实桌面验收。下一步需要测量已有图的同值/单节点变化/重排完整快照接纳；现有 snapshot benchmark
每次先删除图，因此不能证明这些路径。GraphMeta 的 share → clone → share 也未被当前 Chart
资源候选基准覆盖。其余 Graph 深层共享、GraphMeta、Logs 子布局及全部未核销模块继续开放。

上一批收尾的文档契约 6 项、11 文件格式检查及独立 `git diff --check` 均已通过；
以下为随后继续并行处理的批次，不将两批验证混为完整项目验收。

### 并行复核 GraphMeta、结果摘要、日志恢复及完整快照测量（2026-10-02）

三个子代理分别负责 GraphMeta、Graph 基准与日志，主代理处理结果摘要并统一集成；完成实现后
交换只读审查，再由主代理执行共同消费者和类型检查。相互独立的写入范围没有交叉覆盖。

GraphMeta 移除整个条目的 `share → clone → share`。原有一次 Immer 直接处理图类型、签名的
标量参数、输入输出端口，以及 ValueType 的 Array、DataSeries、OneOf 容器；新增或变体变化的
外部内容在接收边界复制，其余叶对象浅比较。没有新增通用不可变引擎，也不按 functionRevision
跳过内容。新增一项纯候选行为保护在旧实现也通过，覆盖同 revision 的递归内容更新、未变子分支
共享、参数/输出删除及 function→event 字段清理；原有输入隔离、改名和项目提交检查继续通过。
参数和端口数组仍按位置更新；按 ID 共享前还需确认 ID 唯一契约，当前 guard 未验证此条件。

图结果摘要由 `core/dataStore/graphResultProjection` 按当前严格协议的有限字段共享，完整图回执
和 ResourceStore 独立结果查询复用同一入口。输出使用已有 graphOutputKey，连接使用完整两端
身份，状态及 resultId 决定记录变化，重排仍保留原对象；根状态继续接纳执行会话、语义 hash 和
revision。新增纯状态回归先在旧实现复现重排丢失引用，修复后检查完整回执与查询更新、删除、
旧快照、冻结及通知次数。交叉审查确认 declared/instance 键完整、结果消费者仍核对会话及语义身份，
原版本拒绝、校验、冻结和原子发布边界没有绕过。测试未直接新增实例端口或跨会话组合矩阵。

日志重订阅依据当前 stream 和 sequence 复用缓冲内已提交记录，保留未变领域数组及同值快照。
身份依据为 tracing 插件 `store.rs` 的 sequence 主键、持久化 stream identity 和只追加 INSERT，
不是未经证明地忽略任意响应内容。新增两个纯 Application 回归分别保护同值/追加恢复的引用和
通知，以及 gap 恢复、clear 后历史恢复、换 stream 不复用旧记录；两项均在旧实现先失败。
排序、去重、容量和 gap 检查仍归原 buffer，没有增加历史缓存或平行 owner。

本批新增四项测试各有上述不同风险，没有新增 UI 单测。定向命令及实际结果如下，重叠范围不相加：

```powershell
pnpm test:ts src/features/application/editorMutation/projectSnapshotMetadata.test.ts src/features/application/project/projectHydration.test.ts
pnpm test:ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts
pnpm test:ts src/features/core/dataStore/graphProjection.test.ts src/features/application/results/runtime.test.ts src/features/application/results/resultQueryCoordinator.test.ts
pnpm test:ts src/features/application/log/logBuffer.test.ts src/features/application/log/logStore.test.ts src/services/log/logService.test.ts src/services/ipc/recordBatchReceiver.test.ts
pnpm test:ts src/services/nodeSystem/graphEditorSync.test.ts src/services/nodeSystem/graphEditorChanges.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editor/observeGraphRunEvent.test.ts src/features/core/state/readonlySnapshot.test.ts
pnpm check:ts
pnpm lint:ts
```

依次为 2 文件 5 项、2 文件 11 项、3 文件 27 项、4 文件 20 项、6 文件 31 项，均通过。
日志命令原来还传入不存在的 `src/shared/types/dto/logParser.test.ts`，上面仅列实际命中的路径，
未将缺失文件计为通过。类型检查发现 benchmark 安装回调把 boolean 返回值转交给只接受 void 的
BenchFunction；改为执行相同安装并丢弃返回值后通过。lint 仍为原有 4 条警告。

Graph 基准在原文件增加已有图接收同值、单节点移动、节点及端口重排的完整快照；每节点两个端口，
三场景分别测 sync、adoption、sync and adoption。每次样本恢复同一已安装状态并接收全新对象，
冻结、校验和模拟 JSON 接收在 adoption 计时外；完整场景验证及样本引用断言也不计时。
100 节点 9 组先做行为预检，再单独测 5000 节点 9 组，各 30 次，正式用时 160.132 秒。
主代理在正式测量期间暂停统一测试、类型和 lint，未宣称操作系统或 GC 已完全隔离。

```powershell
pnpm bench:graph --testNamePattern='^100 nodes: existing graph'
pnpm bench:graph --testNamePattern='^5000 nodes: existing graph'
```

机器为 Windows 11、i9-13900K，Node 24.19.0、Vitest 4.1.10。均值单位 ms，括号内为 RME：

| 已有图完整快照 |              sync |          adoption | sync and adoption |
| -------------- | ----------------: | ----------------: | ----------------: |
| 同值           |  179.64（±8.74%） | 93.5311（±6.03%） |  260.05（±2.24%） |
| 单节点移动     | 207.68（±31.01%） | 94.7564（±6.18%） |  220.14（±3.50%） |
| 节点及端口重排 | 236.45（±73.90%） |  240.07（±2.72%） | 460.84（±35.72%） |

[原始输出与环境](../benchmark/probes/graph-existing-snapshot-2026-10-02.txt) 包含完整命令、stdout、
HEAD 和 dirty tree 说明。测量已包含新的结果摘要共享；测量后只修正 benchmark 回调返回类型，
未为此重跑。部分 sync 和 combined 样本长尾明显，各阶段独立测量的均值不能相减作精确归因。
约 94–95 ms 的同值/单节点变化安装成本与约 240 ms 的重排成本说明完整安装仍需审查；
没有旧版本或 Immer 对照，**不能据此批准规则例外或声称此次修复提速**，也不代表桌面帧率。

Workbench `logsRuntime` 的递归比较已确认是七个 domain tabs 的原生布局持久化边界，与日志记录
无关。它仍需进一步复核：不能只比较 selected/order 而丢弃合法布局字段；Model.toJson() 会调用
节点 save 事件，binding revision 不当然代表全部序列化内容。同 revision capture 跳过策略还需
处理同步订阅者重入 restore/replace，当前未修改此路径或批准例外。
并行只读排查还确认 Project 与 ResourceDelta 的函数签名 guard 可收拢：后者当前由标量校验及
精确字段形状两段合成，不能单删其中一段；下一批应复用同一严格 guard 并验证真实函数 delta。

本批更新 Features、Logs 和 benchmark README，相关 Rust 代码只读核对；未运行 Rust、完整 CI
或桌面验收。文档契约、13 文件格式及独立差异检查作为本批收尾，不替代全模块审查。

### 并行修复函数签名边界、Logs 重入与 Harness Channel（2026-10-02）

本批继续以三个子代理分别处理签名边界、Logs runtime 和 Graph 完整快照；签名完成后接续
Harness Channel，Logs 完成后对 Channel 作独立只读审查。主代理核对真实调用方、简化项目进度
关闭状态并执行消费者与共同检查。Graph 正式基准期间暂停测试及 Cargo，避免主动制造 CPU 争用。

Project 索引与 ResourceMutation 的函数签名共用类型旁的 `isFunctionSignatureDto`：签名只接受
parameters/return_type，参数只接受 id/name/type_name，字段类型和 nullable return 保持原契约。
type_name 仍为不透明字符串，语义属于 Rust `yss-project-history::FunctionSignature`，不能与
Catalog 的 ValueType 签名混为一套协议。原 ResourceMutation 的值校验与精确形状校验共同构成
同样的严格规则；现移除两处重复子树检查，保留补丁 before/after 外壳及回执关联校验。
新增两项实际边界保护，在旧实现也通过，分别覆盖项目索引与真实 function delta 的严格字段、
不透明类型名和输入隔离；没有为纯代码移动增加同构测试。

Logs 持久化 runtime 先比较原生 JSON，有变化才复制；已经隔离的 restore/reset 候选直接交给
原 Zustand 快照，LayoutModelBinding.replace 继续拥有进入原生 Model 的隔离复制。导出仍返回副本。
每次 capture 调用 toJson，保留节点 save 事件；没有凭 revision 跳过序列化或漏掉合法布局字段。
同步观察者可以在旧绑定的最后 capture 中重绑：原实现的 finally 会清掉后继。现在 bind、capture、
restore 和清理核对本次 BoundFlexLayout 对象身份，外层 bind 也不覆盖回调建立的新绑定；同一 API
重新绑定后，旧恢复异常不能清理新生命周期。两项纯 runtime 用例覆盖输入/输出隔离和 save 事件，
以及卸载、外层 bind、同 API 恢复失败期间的重入；后一项初始场景先在旧实现失败，扩展场景在
最终实现验证。保留的通用布局比较仍待进一步规则审查，本批未批准其性能例外。

主代理逐文件读取 IPC Channel 的三个生产文件、IPC Event 的一个生产文件及 Cargo/README，
并核对 Application command/runtime、Harness EventWriter 和前端订阅/恢复入口。发现 Harness
原来持全局订阅锁调用 Channel.send，回调重入 publish/unsubscribe 会死锁，慢发送也占用注册锁。
现只在锁内准入、领取顺序项和占有单个 drainer，发送移到锁外；领取时推进序号，避免在途重复。
历史逐项与同一实时队列合流，长历史不会直接塞满 256 项队列。积压超限立即关闭准入，仅保留
最高序号作为最终 gap 通知，由原 drainer 在在途项之后发送并移除订阅；发送失败同样移除。
unsubscribe 阻止后续领取，在途发送可能完成。交叉审查进一步发现删除订阅时可能在锁内释放
最后一个 Channel 并调用外部 on_drop；现先取出订阅、解锁后释放，Rust 5 项与格式检查再次通过。
发送仍由原调用者同步执行，未新增线程或独立
worker，也不宣称不同订阅的发送已完全隔离。业务持久化、wire 与恢复事实仍归原 owner。

两个新增 Rust 回归分别保护重入发布/去重/顺序/退订，以及阻塞发送时并发积压、单 gap 终止和
发送失败清理；try_lock 检查避免对旧实现测试时无限挂起。本次未执行旧 Rust 红例，不将源码
发现冒充运行证据。原有回放大于队列容量、乱序补齐和重复抑制用例继续通过。ProjectProgress
删除与 sender Option 重复的 closed 状态，close 在锁内 take、锁外 drop，enqueue 仍在同一锁内
nonblocking try_send；原有容量、所有 Arc 克隆关闭及剩余队列排空回归通过。

Graph 完整快照先按 nodeId 共享完整节点，仅当端口引用仍变化时按完整地址对齐并再次共享该节点，
恢复重排后 session projection 内未变节点及端口的引用，不修改旧数组。根比较两侧共用处理后的
节点数组，避免按数组位置再共享而撤销身份匹配；对已经共享的增量保留输入节点、数组和根的
身份及其既有校验凭据。没有新增公开 parser 接口或缓存 owner，同值和位置变化不再预扫描端口。
既有重排用例改用完整全新对象，旧实现先在引用断言失败；原拒绝用例扩展重复节点和端口。
原增量用例增加 Immer 节点/端口重排，在中间实现复现丢失输入根身份，修正后通过，未新增测试项。
完整候选仍检查图身份、重复 ID、端点与诊断，未将节点引用复用当作整图校验凭据。
通用 shareProjection 的剩余边界审查继续开放。

Graph 测量保持上一批的样本、fixtures、setup 和计时体，仅选择 5000 节点的三组 adoption，
每组 30 次。早期端口预扫描方案未达到预期，因此没有止于第一轮重排均值下降；经过旧策略
定位控制、局部探针后，最终改为上述逐节点方案，删除不再需要的公开缓存访问器。
以下均值单位为 ms，括号为各进程内部样本的 RME，不是跨版本差异的置信区间：

| 测量方案                 |               同值 |        单节点移动 |    节点及端口重排 |
| ------------------------ | -----------------: | ----------------: | ----------------: |
| 端口预扫描，直接计算键   | 85.9341（±15.17%） |  117.89（±3.62%） |  198.50（±2.99%） |
| 端口预扫描，复用校验键   | 91.7065（±14.07%） |  124.07（±2.65%） | 176.12（±10.23%） |
| 相同筛选的旧策略定位控制 |  74.9392（±7.18%） | 99.1925（±5.47%） |  235.49（±3.34%） |
| 端口预扫描，改用索引循环 |  121.84（±21.43%） |  119.16（±2.22%） |  216.95（±1.86%） |
| 最终逐节点共享           |  89.6332（±3.73%） |  101.94（±3.60%） |  223.81（±2.75%） |

命令为 `pnpm bench:graph --testNamePattern='^5000 nodes: existing graph .* snapshot adoption$'`。
[完整原始记录](../benchmark/probes/graph-existing-snapshot-aligned-2026-10-02.txt) 保留各次 stdout、
环境、dirty tree、临时切换/插桩脚本和前后 SHA256；临时操作均已恢复，benchmark 无永久改动。
另一次只选 move 的插桩探针记录 helper 30 个计入样本的调用均值 5.7416 ms、中位 4.7698 ms；
整体 adoption 为 108.22 ms（±27.42%，最大 515.01 ms）。首条额外 hook 属于 Tinybench 同步性
探测，不计入 30 个 samples，原始输出仍完整保留。局部计时不能排除分配后的延迟 GC，也不能
替代无插桩、相同筛选的完整路径测量。

最终版本恢复了必要的实体和增量引用契约，单节点移动的观测均值回到接近旧策略控制；同值
快照仍高于该次旧控制，不能声称全部路径提速或性能问题已经完成。旧控制本身无法通过新增
投影重排身份回归，内容可比不等于引用行为完全同语义，也不是默认 Immer 对照。各次不是随机
交替实验，JIT、GC 与进程状态仍会影响结果；**没有据此批准任何规则例外或推断桌面帧率**。

最终定向检查如下，重叠范围分别报告，不相加冒充不同用例总数：

```powershell
pnpm test:ts src/services/project/projectService.test.ts src/shared/types/domain/resourceMutationValidation.test.ts src/services/nodeSystem/functionMutationService.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/services/project/projectEventParser.test.ts
pnpm test:ts src/modules/workbench/internal/layout/logsRuntime.test.ts src/modules/workbench/internal/layout/workbenchNotifications.test.ts src/modules/workbench/internal/layout/workbenchLayoutPersistence.test.ts src/modules/workbench/internal/layout/workbenchActivation.test.ts src/modules/workbench/internal/layout/workbenchFloatingLayout.test.ts
pnpm test:ts src/features/core/dataStore/graphProjection.test.ts src/features/domain/editorProjection/editorProjection.test.ts src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphEditorSync.test.ts
pnpm test:ts src/features/application/results/runtime.test.ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editor/observeGraphRunEvent.test.ts src/features/core/state/readonlySnapshot.test.ts
pnpm test:ts src/services/assistant/harnessService.test.ts src/services/assistant/harnessContract.test.ts src/features/application/assistant/assistantHarnessSession.test.ts src/features/application/assistant/assistantHarnessProjection.test.ts src/features/application/assistant/assistantHarnessRuntime.test.tsx
pnpm test:rs:package -p yss-ipc-channel --lib
pnpm check:rs:package -p yss-application --lib
pnpm lint:rs:package -p yss-ipc-channel --lib
pnpm check:ts
pnpm lint:ts
```

上列功能检查依次为 5 文件 33 项、5 文件 15 项、4 文件 57 项、5 文件 33 项、5 文件 13 项，Rust Channel 5 项，
均通过。Rust Application 消费者编译、Channel Clippy 及 TypeScript 检查通过，前端 lint 为原有 4 条警告。
上述范围新增六项分别覆盖独立风险的纯边界/runtime/Rust 用例；接续取消回归另列，未新增 UI 测试。
当前契约同步到 Features、Workbench、Application IPC 与 IPC Channel README；全模块核销、
真实桌面与完整 Julia 计算链仍未完成，局部检查不替代它们。

接续只读预审完整核对 Harness Core 的 events/ports/tools、Host 的 mod/session/turn/workflow、
orchestration/mod，SQLite 的 events/lib、Rig 的 stream/driver，以及 Application 的 Harness
用例、runtime/harness、IPC runtime 和 command_harness。Contract 的取消与持久化契约及相关
测试只读了对应片段，不能据此核销完整 crate。SQLite append 在返回前已提交；EventWriter 的
Host 级异步锁覆盖 append→sink.publish，以保持已有实时顺序，慢 sink 会延迟其他会话及取消
收尾。这是需继续评估的串行传播边界，未发现生产反向调用闭环，不能直接认定生产死锁或将
publish 任意搬到锁外。Host 取消方法此前持 active_turns 锁同步唤醒任意 Waker；自定义 Waker
重入 Host 的闭环已获独立交叉确认，生产 Tokio 的 wake 只入队唤醒，尚无实际重入故障证据。
现沿用 WorkflowControl 的局部模式，在锁内克隆当前 token，锁外执行 cancel/wake，User 入口
复用既有 cancel_active_turn。取消只作用于捕获的 token，不影响后继 turn；首个 reason 仍由
token CAS 决定，但并发取消不承诺按 active-turn 取锁顺序获胜，bool 也不等于业务已经停止。

新增一项纯 Host 回归先 try_lock 断言再同步重入，同一用例覆盖 User 与 ProjectReplaced 入口、
首次原因及 turn 准入释放后的 false；Wake 只持 Weak<Host>，不形成强引用环。旧实现实际运行 1 项失败，
修复后 Core 库 26 项全部通过，包含既有父子任务取消、项目核对、workflow 迟到结果和连续事件流。

```powershell
pnpm test:rs:package -p yss-harness-core --lib host::tests::cancellation_releases_active_turns_before_waking_reentrant_waiters
pnpm format:rs:package -p yss-harness-core
pnpm test:rs:package -p yss-harness-core --lib
pnpm lint:rs:package -p yss-harness-core --lib
```

Core 格式、Clippy、三文件差异检查及独立审查通过，Harness Core README 已更新捕获与取消语义。
首次带额外 --exact 的尝试因参数转发失败未运行测试，未算作红例或通过结果。EventWriter 保持原契约。

本批最终收尾：文档契约 6 项、17 份 TypeScript/Markdown 文件格式、两个 Rust crate 格式、
TypeScript 检查及独立 `git diff --check` 均通过。最终 Graph 源码 SHA256 与测量后记录一致，
未留下临时计时探针或旧策略切换。未运行完整 CI 或真实桌面验收，剩余覆盖与性能问题仍按下列清单继续。

### 并行接续：缩减 Graph 镜像与生命周期边界（2026-10-02）

本轮继续三个子代理分别负责 Graph、前端状态/平台和 Harness，主代理审查公共 IPC、交叉复核并整合。
实现与定向检查并行，性能测量单独运行，不让编译或其他测试污染同机样本。没有新增状态 owner、
协议、依赖或迁移逻辑，也没有据此宣布所有模块合规。

Graph 会话 Store 删除完整 GraphDocument 镜像，仅保留实际使用的 constants；其余编辑事实继续
来自既有 projection、实体表和 Rust 回执。常量编辑读取 constants，插入节点判断复用原有 nodes
索引，结果报告的 summary 修改在 FIFO 真正执行时读取同图实体的节点类型。Rust mapper 逐个
document 节点生成投影节点，因此无需再建节点 ID 集合。完整 document 仍在 IPC 入口校验、冻结
并可能由同步基线持有，未改后端协议或宣称消除同步解析成本。也未用 revision/hash 代替内容判断；
同版本常量变化仍接纳，未变常量与 delta 身份保留，等值快照不发布。既有回归直接扩展，未新增测试项。
首次 TypeScript 检查找出一个 mutation 方法简写消费者及两处旧 fixture，已一并修正；再次类型检查通过。

前端新读 Core UI 三文件、Application 进度三文件和 Platform 全部九个生产文件，并沿设置同步、
窗口创建、项目/数据进度及原生关闭入口核对消费者。ProgressHandle 同值更新复用 Zustand shallow，
保持原 progress 引用且不通知。窗口几何模块沿用原队列和监听清理表，增加该模块生命周期的 disposed
检查：已注册监听立即释放，异步注册迟到后自行释放，旧排队恢复和 await 后续操作停止。两项新纯
Application/Service 回归在旧实现实际失败，修复后连同项目和数据管理消费者共 4 文件 27 项通过。
未新增 UI 单元测试；窗口关闭确认期间卸载的独立边界与真实窗口/HMR 验收继续开放。

公共 IPC 新读五个入口、HMR、ExecutionDrain、ResultSessionChannel 与 tauriWebview，并核对日志、
项目、Harness 和 Workbench 直接消费。Record receiver 复用同一个 pending 队列，回调重入的新批次
排在已接纳批次之后，每项交付前检查 dispose/断流；默认 64 容量仅计算尚未交付项，活跃溢出进入
既有 subscriber-lagged 恢复。prepare/入队仍拥有序列水位，排空不再推进它。HMR 先取登记快照并清空，
释放全部捕获 Channel 后传播首个异常，回调创建的新实例留给下一生命周期。ExecutionDrain 沿用 settled
事实，结算后停止解析和交付消息。新增两项纯边界回归，另在既有执行用例补迟到消息断言：旧实现
实际为 5 失败/10 通过，最终 15 项通过；旧红例附带的两个未处理 rejection 来自断言早于 await 的
测试顺序，已修正，不记为额外生产故障。L2 共 10 个不同文件 69 项通过。当前生产无同一 Channel
对象在清理中重新登记的入口，本批不增加代次框架；terminal 回调同步重入也不伪称由 settled 早返覆盖。

Harness SQLite 十个生产文件及持久化 ports 已逐读；Core/Contract 只核销本批实际读取的文件与片段，
不将整个 crate 标为完成。SQLite 原先将文件路径转为 URL，SQLx 会把合法目录名中的 `%20` 解码为
空格，可能打开另一目录的数据库。现直接使用 SqliteConnectOptions.filename，保留文件系统路径，
不改变 schema 或内存库。新真实文件回归旧实现 1 项失败；修复后 SQLite 8 项及 Application 项目/库
重开隔离 1 项通过，SQLite all-targets Clippy 通过。临时目录由测试唯一创建，关闭 pool 后只清理该目录。

Contract 取消 Future 原先在 waiters 锁内克隆或释放 Waker。poll 现锁外 clone 一次，锁内 insert 并将
旧 Waker 带出后 drop；Future Drop 同样锁外释放 remove 返回值。注册前后原因检查、首原因 CAS 和
waiter 身份不变。新安全 Wake 回归用 try_lock 观察释放边界、成功后重入 cancel，避免旧实现测试挂死；
旧实现 1 项失败，修复后 Contract 6、Core 26、Rig 21 项通过，Contract all-targets Clippy 通过。
这验证任意 Waker 的生命周期契约，不等于证明 Tokio 生产路径已经发生死锁。EventWriter 的跨会话
append→publish 串行传播仍待评估；不能在没有顺序证据时直接把 publish 移出锁。

本轮主要验证命令如下；有重叠的 L1/L2 范围分别记录，不累计成不同测试总数：

```powershell
pnpm test:ts src/features/core/dataStore/graphProjection.test.ts src/features/domain/editorProjection/editorProjection.test.ts src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphEditorSync.test.ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/graphEditing/graphConstantActions.test.ts src/features/application/editor/canvasDrop/dropGraphConstant.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts
pnpm test:ts src/features/application/results/runtime.test.ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editor/observeGraphRunEvent.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/application/results/addLinearSummaryContents.test.ts src/features/application/editor/useEditorHistoryAvailability.test.ts
pnpm test:ts src/features/application/ui/progressLifecycle.test.ts src/services/platform/mainWindowGeometry.test.ts src/features/application/project/projectLifecycleOperations.test.tsx src/features/application/dataManagement/useDatabaseManagement.test.tsx
pnpm test:ts src/services/ipc/recordBatchReceiver.test.ts src/services/devHmrIpc.test.ts src/services/project/executionChannelDrain.test.ts src/services/log/logService.test.ts src/features/application/log/logStore.test.ts src/features/application/log/logBuffer.test.ts src/services/project/projectService.execution.test.ts src/services/assistant/harnessService.test.ts src/services/workbench/presentationService.test.ts
pnpm test:ts src/services/project/projectService.test.ts
pnpm test:rs:package -p yss-harness-sqlite --lib
pnpm test:rs:package -p yss-application --lib harness::tests::conversations_restore_after_project_and_database_reopen_and_remain_isolated
pnpm lint:rs:package -p yss-harness-sqlite --all-targets
pnpm test:rs:package -p yss-harness-contract --lib
pnpm test:rs:package -p yss-harness-core -p yss-harness-rig --lib
pnpm lint:rs:package -p yss-harness-contract --all-targets
pnpm check:ts
pnpm lint:ts
```

Graph 两组分别为 8 文件 69 项、7 文件 35 项；IPC 两组分别为 9 文件 49 项、1 文件 20 项。
TypeScript 通过，lint 仍为原有四条警告。新增六项分别对应不同边界的回归，未增加 UI 用例。
完整 CI、真实桌面和完整 Julia 计算链仍未运行，不能由上述局部验证替代。

所有 Cargo 和前端测试退出后，运行原三组 5000 节点 adoption，每组 30 样本，计时体和输入场景不变。
bench 仅在 setup 保存原始 document 引用，以延续“输入确为新快照”的既有断言；不再从已删除的
Store.document 读取它。无插桩或旧策略切换。命令与 [原始输出、环境及源码哈希](../benchmark/probes/graph-constants-mirror-2026-10-02.txt)：

```powershell
pnpm bench:graph --testNamePattern='^5000 nodes: existing graph .* snapshot adoption$'
```

| 当前路径       | 上轮逐节点共享，仍保存 document（ms） | 本轮仅保存 constants（ms） |
| -------------- | ------------------------------------: | -------------------------: |
| 同值快照       |                     89.6332（±3.73%） |          48.5472（±1.83%） |
| 单节点移动     |                      101.94（±3.60%） |          54.9135（±2.64%） |
| 节点及端口重排 |                      223.81（±2.75%） |           120.06（±0.64%） |

当前基准文件运行 34.383 秒；括号为各进程样本 RME。这是同机、同输入和筛选的先后进程观察，
不是随机交替实验或默认 Immer 对照，不能将差异全部归因于单段代码。场景 constants 为空，
不能外推大型常量的共享成本，也不包含 Rust、真实 IPC、DOM 或完整交互。三组观测均值下降，
支持移除无消费镜像和遍历的方向；通用 constants/projection 递归仍继续审查，**没有批准规则例外**。

本轮收尾：文档契约 6 项、25 个 TypeScript 文件及 7 份 Markdown 格式检查通过；SQLite 和 Contract
包格式、相关 Rust Clippy、TypeScript 检查通过，前端 lint 保留既有四条警告。独立全工作树
`git -c core.safecrlf=false diff --check` 通过。基准六份关键源码运行前后哈希一致，没有残留临时探针。

### 并行接续：共享边界、关闭许可与错误观察（2026-10-02）

上一轮有实际修复与验证进展，本轮继续主代理加三个子代理并行；目标和剩余核销范围未缩减。
新增读取 lib/utils、shared/utils 原四个文件、共享画布投影/交互、overlay、Markdown 与 Modal 边界，
并追踪真实 Application 消费。公共事件监听复用原生 add/remove 清理，删除内部 AbortController
及其兼容分支，调用方提供的 signal 不再被覆盖；已有键盘和画布消费者通过，未为平台原生行为
新增重复测试。其余共享 UI、统计呈现及基础类型仍不能由这些局部读取替代。

Graph 的通用 shareProjection 现在只有 graphProjection.ts 一个生产调用文件；另外 13 个读取投影
消费者复用 Zustand 和 shallow，没有各自执行通用深合并。实体安装已先获得按节点/端口身份共享的
投影，Pin 再递归比较一次、Node 再递归比较诊断和端口添加项均无必要。本轮删除 shareEntity，Pin
只浅比较自身字段，Node 的 position/display/capabilities/diagnostics/portInstanceAdditions 直接借用
规范投影引用。原 bucket Immer、完整候选校验和参数组/参数按 key 共享保留；既有测试补实际连接
变化与 canonical 引用断言。L1 4 文件 57 项、消费者 5 文件 31 项通过，没有新增测试或性能例外。
此前空 constants 的 Graph 基准不用于外推任意 JSON 内容，本轮也未重复运行不受影响的基准。

仍需审查外部 JSON 内容共享与参数键匹配。可检验的默认方案候选是以 next 为 base，在一次 Immer
produce 中把内容相等的分支指向 previous；内容变化、删除、数组顺序和已共享 delta 的 next 身份必须
同时保留。constants map 可独立比较，但完整投影不能用逐节点额外 produce scope 冒充一次批次默认
方案。本轮只有设计与适用边界，未实现或测量该参考，不能作为保留当前通用递归的豁免证据。

关闭流程原来只在注册回执时检查卸载，确认对话框返回后仍可继续保存，flush 期间项目换代仍可继续
关闭。现复用既有项目生命周期身份与调用方 isActive，在确认、保存批次和 flush 的 await 后重验；
已提交保存自行结算，失效后不启动下一个文件或弹出旧错误。原布尔关闭放行改为同一个一次性许可，
第二次原生关闭回调消费许可时再次核对挂载与项目；旧失败回执不能清除新项目的后继许可。实际本地
Tauri close invoke 与 WINDOW_CLOSE_REQUESTED 处理不是原子步骤，因此最后这次重验有真实调用依据。
两个新增纯 Application 回归旧实现实际 2 失败，修复后通过；既有关闭 hook 四项通过，没有新增 UI
测试。真实确认期间卸载/换项目与原生窗口关闭仍需人工验收，原生已真正关闭不能撤回。

原 shared formatErrorMessage 将未知异常文本用于 Graph 日志；另外 Editor/Project、数据读取、初始化、
Graph 卸载和呈现加载也直接记录 String(error)/message。本轮删除旧 formatter，沿 Application 现有
IPC 归一化增加一个格式化入口，只保留受识别 code/incidentId，未识别异常仍用已有 transport/malformed
分类；typed mutation outcome 继续直接记录其已知 status。一个新纯 Graph 刷新回归实际旧红 1 失败/6
通过，最终 7 项通过，覆盖不读取原始 message、保留后台 incident、拒绝非 branded 任意 code。
另将 console 捕获的 Error 归为固定类型摘要，原生 console 仍接收原参数；新 transport 回归旧红
1 失败/4 通过，修复后 logger/batcher/LogService 三文件 13 项通过。日志不拥有业务错误或结果状态。

报告校验的 typed diagnostic 不是未知 IPC 异常，不能机械转成通用错误。原 JSON.stringify 会读入
outputPinId/fieldPath/reason 等自由字符串，现由原 reportViewIssue owner 的具体动作只挑选已验证的
resultId/runId/nodeId 与封闭 report/valueKind。ReportView 直接传 typed diagnostic，现有用户显示不变。
新增一个纯边界回归在保持旧 JSON 行为的提取版本实际失败，读取四个自由字段 getter；修复后通过。
Observability README 同步这些职责与日志内容边界，不依赖后端 sanitizer 为原始内容兜底。

Harness Contract 本轮补读 agents/capabilities/context/graph/inspection/knowledge/resources/
statistics/validation、测试和 Cargo，结合前批 harness/gateway/persistence/lib 已覆盖全部生产文件。
context 与 harness 两份 string_identity 宏合并为原 context 宏的 crate 内复用；21 个类型保持独立，
derive、serde/schema、错误文本、UTF-8 128 字节限制及原字符串不归一化行为不变，没有新增抽象或测试。
Contract 6、Core 26、Rig 21 项通过，Contract all-targets Clippy 通过。

UI Contract 的唯一 lib、Cargo、README、局部规则及 Application presentation 生产代码/模板入口已全文
读取。请求结构、当前 source/session、合成完整候选和 binding capability 分层验证，提交前检查完整
页面；未复制校验器或改变协议。发现 UiSubscription 移除最后一个 observer 时在 registry 锁内析构
捕获对象；现 remove 返回值带出锁后再 drop。一个新安全 Rust 回归用 Weak/try_lock 验证并重入订阅，
旧实现实际 1 失败、正常退出无挂死，修复后单例及 presentation:: 八项通过。此证据限定自定义析构
边界，不代表已发生生产死锁。commit 锁内同步发布承担既有顺序，本轮不贸然移锁。Application Clippy
退出成功，SCI 依赖有六项现有警告，未触碰并行的统计实现。

主要 L2 命令如下；重叠的定向复跑不累计为不同测试总数：

```powershell
pnpm test:ts src/features/core/dataStore/graphProjection.test.ts src/features/domain/editorProjection/editorProjection.test.ts src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphEditorSync.test.ts
pnpm test:ts src/features/core/dataStore/nodeView.test.ts src/features/application/results/runtime.test.ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/core/state/readonlySnapshot.test.ts
pnpm test:ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/editor/dropFunctionIntoEventEditor.test.ts src/features/application/editor/useEditorOperations.capabilities.test.tsx src/features/application/editor/useProjectOperations.execution.test.tsx src/features/application/editor/useProjectOperations.saveActiveFile.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx src/features/application/editor/useCanvasDrop.test.tsx src/utils/frontendLogger.test.ts
pnpm test:ts src/utils/frontendLogger.test.ts src/utils/frontendLogBatcher.test.ts src/services/log/logService.test.ts
pnpm test:ts src/features/application/editor/confirmDirtyEditorClose.test.ts src/features/application/editor/useWorkbenchWindowCloseGuard.test.tsx src/features/application/observability/reportViewIssue.test.ts src/features/application/databaseEditor/useDataLoader.test.ts src/features/application/dataManagement/databaseRead.test.ts src/features/application/presentation/loadPresentationWindow.test.ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/initialization/appInitialization.hook.test.tsx src/shared/types/report/report.test.ts
pnpm test:rs:package -p yss-harness-contract --lib
pnpm test:rs:package -p yss-harness-core -p yss-harness-rig --lib
pnpm test:rs:package -p yss-application --lib presentation::
pnpm lint:rs:package -p yss-harness-contract --all-targets
pnpm lint:rs:package -p yss-application --lib
pnpm check:ts
pnpm lint:ts
```

上述前端组依次为 57、31、53、13、29 项。53 项那次命令另带了不存在的 tooltip.test.tsx 路径，实际
只运行八个文件，未将缺失文件计为验证；其中 logger 随后因新增 Error 捕获回归单独复验。关闭助手的
另三个既有消费者文件 25 项通过，最终一次性许可修改后再次运行既有 hook 四项通过。全局 TypeScript
通过，前端 lint 从四条既有警告减为三条（globalEvent 冗余分支已删除）。本轮新增六项分别对应独立
Application/传输/Rust 风险，没有新增 UI 测试，完整 CI 和真实桌面仍未运行。

最终收尾：25 个 TypeScript 文件及 6 份 Markdown 的格式检查、文档契约 6 项、相关 Rust 包格式和
独立全工作树 diff 检查通过；最后许可改动后的 TypeScript 与最终 lint 已复验。没有新增基准记录，
没有将旧测量哈希或局部测试描述为当前全模块的性能/合规证明。

下一批需先验证 Workbench controller.unbind 在捕获 Logs 触发同步重绑时是否会清除后继持久化/
hydration 状态；本轮仅定位调用证据，尚未修改或复现。Graph Analysis 后续按 semantic_snapshot、
analysis、result_category 到 Runtime/Application/Execution 的真实消费者逐入口核对，不能把以往主题
检查算作该 crate 全面完成。StatisticalPlan 的 confidence_level 目前接受 0，但缺少本模块明确开区间
规范和实际数值消费证据，先保留开放判断，不擅改并行统计契约。

### 并行接续：绑定归属、语义标题与剩余服务边界（2026-10-02）

继续由主代理与三个子代理分工，分别处理 Workbench 绑定、Graph constants 默认 Immer 方案、Rust
Analysis/Runtime 消费链及前端服务入口；有依赖的修改和性能采样仍顺序执行。

本轮全文读取 Chart 四个 service 生产文件、Clipboard 两个入口、ResultService/ResultSessionChannel、
GraphService、DocService、MindService 与共享 fileContentService，并追踪 Chart 的 useChartPreview/
toChartModel、Results 的 resultLeases 和 Mind 的实际投影消费。Chart 的缓存/在途请求依据项目实例、
资源版本和请求身份隔离；反向路径索引随 LRU 或显式失效释放。Results 租约回执身份由现有
Application acquire 关联校验并清理失败申请，不在 service 重复增加第二套协调逻辑。该部分未发现
需新增 owner 的问题，8 文件 62 项现有边界回归通过；这不替代尚未读完的全部共享图表呈现。

Mind parser 原先只检查节点 ID 唯一和 parent 存在，仍接纳根有父节点、多个根以及脱离根的环；
renderer 的 visited 只能防止重复遍历，不能把这种无效输入恢复成完整的树。现沿既有 parser 将 ID
集合改为临时 parent 索引，核对主根，并用 connected 集合逐链接入；迭代且线性，不新增校验框架，
不在 renderer 重验。新增一个纯服务回归实际旧红：三个无效树断言均失败；修复后同时接纳反序的
合法 5000 节点链，保留原内容引用。Service、Mind actions、项目发布和 Mind 投影四文件 10 项通过。
Document editors README 同步边界；独立子代理只读复核该改动通过，没有新增 UI 测试。

Workbench 两个新纯 runtime 回归实际旧红后转绿：Logs 最后捕获同步重绑会使后继 hydration 被旧
unbind 清理；旧 flush 等待 idle 的全局暂停计数还会压住后继持久化。修复复用 BoundRoot、现有
bindingGeneration 和 HydrationCycle：bind 在交接及同步 beginHydration 后检查代次，unbind 在 Logs
捕获和原生 toJson 后核对原身份，只释放仍由自己持有的绑定，并在发布 unbound 前交出 bound。
暂停计数移入原 cycle，旧 flush finally 只取消自己 cycle 的定时写。没有新增订阅框架或第二份布局。
9 文件 52 项 L2、三文件格式和差异检查通过；原生窗口与浮窗交互仍待人工验收。

Rust 本批全文复核 Analysis 的 semantic_snapshot、analysis、result_category、lib、concrete_interface、
schema_state、resolution，以及 Runtime lib、Application graph/inputs、Execution graph_preparation。Ready 状态在
Application 准入和 Execution 准备中继续复核；kernel 指纹一致性和 output category 的穷尽映射保留。
函数语义按现有去重入口解析；未实现的函数 lowering 仍由缺少 kernel 的诊断阻断，不误记为可执行。

Runtime 本地化原先扫描节点所有原始字符串来猜资源名称，漏掉解析默认值，也可能被无关参数抢占。
现按协议 instance_display 指定的 key 消费 GraphResolvedParameterValue::Resource，不再读取文档猜测，
并删除本地化两个函数的多余 document 参数。常量标题保留，用户 user_label 仍由 Editor 从文档独立
交付。一个新纯 Rust 回归旧实现实际失败，修复后验证默认资源、无关文本不抢标题、不写入默认值
和自定义标签保留。Runtime 15 项通过（1 项既有手动 benchmark ignored），Execution 准备 6 项与
Application 会话消费者 3 项通过；Runtime all-targets Clippy 无警告，包格式通过。
解析缓存复用还核对实际资源依赖及 absent lookup；Application 的逐节点有效性与整图 ready 用途
不同。现有 Execution 回归确认已缓存计划不能绕过当前不可用的 kernel。后续优先补读 Analysis
parameter_projection、node_projection、port_projection、function_validation 的其余部分，以及
Runtime semantic_cache/connections；本批未据此宣称整个 Analysis 或 Runtime 已完成。

Graph constants 已实际实现并验证一次 produce 参考，但发现不能覆盖现有合法 JSON 契约：Rust
DataValue::Object 的 BTreeMap 及前端既有 parser 均允许自有 `__proto__` 字段；当前 Immer 的 draft
赋值会先调用继承的原型 setter 并抛错。参考路径在纯回归中实际失败。独立代理读当前依赖源码确认
严格浅复制、createDraft、defineProperty 和 patch API 均未提供保持此键和相等分支引用的等价入口；
这只说明当前参考的限制，不声称所有未来默认方案均不可能。

因此 constants 的生产调用恢复既有 shareProjection；参考移入现有 benchmark 目录，没有未用的生产
模块或可选双策略。新增一个纯边界用例保留内容/删除/顺序/旧值隔离与已共享 delta 身份，并经真实
Store 安装验证自有特殊键及其相等分支引用；7 文件 63 项通过，TypeScript 通过。没有放宽 parser、
改变 JSON 原型或添加通用手工 fallback。下述普通键测量只能比较所声明输入，不能单独成为保留
通用递归的规则豁免；该项继续开放。

新增非空 constants 基准使用 500 个常量，每个含两组 16 项 List、metadata、description 和 tags。
两策略按场景交错登记在同一进程，独立预热、每组实际 30 样本；JSON 物化、冻结和 DTO 校验、结果
内容/引用断言均在计时外，计时体均包含共享函数及相同发布冻结证明。没有同时运行 Cargo 或前端
测试。命令与 [原始数据及环境/源码哈希](../benchmark/probes/graph-constants-immer-2026-10-02.txt)：

```powershell
pnpm bench:graph --testNamePattern='^500 constants:'
```

| 500 constants 场景 | 现有 helper 均值 ms（RME） | 一次 produce 参考均值 ms（RME） |
| ------------------ | -------------------------: | ------------------------------: |
| 同值新快照         |           3.2791（±4.00%） |               31.7500（±1.32%） |
| 单个常量改名       |           3.6161（±4.66%） |               32.9931（±2.68%） |
| 单个嵌套值变化     |           3.9552（±6.61%） |               32.0975（±1.19%） |

Vitest 文件耗时 16.577 秒，六组内容和身份断言均通过。这是固定登记顺序的一次进程观察，既未随机化
多进程重复，也未测内存、Graph projection、IPC 或 UI。当前参考在此规模更慢且存在上述合法输入
限制，不能直接采用；数字不等于 Immer 库的普遍代价，也不替其他通用 helper 调用批准规则例外。

```powershell
pnpm test:ts src/services/mind/mindService.test.ts src/features/application/resource/mindActions.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/modules/document-editor/internal/mindProjection.test.ts
pnpm test:ts src/services/chart/chartPreviewCache.test.ts src/services/chart/chartPreviewDataService.lifecycle.test.ts src/services/chart/chartPreviewDataService.error.test.ts src/services/chart/chartService.test.ts src/services/clipboard/graphClipboardService.test.ts src/services/result/resultService.test.ts src/features/application/results/resultLeases.test.ts src/features/application/chart/toChartModel.test.ts
pnpm test:ts src/modules/workbench/internal/application/workbenchLayoutController.test.ts src/modules/workbench/internal/layout/logsRuntime.test.ts src/modules/workbench/internal/layout/workbenchActivation.test.ts src/modules/workbench/internal/layout/workbenchFloatingLayout.test.ts src/modules/workbench/internal/layout/workbenchLayoutPersistence.test.ts src/modules/workbench/internal/layout/workbenchNotifications.test.ts src/modules/workbench/internal/application/workbenchLayoutActions.test.ts src/features/application/editor/editorPanelActivation.layout.test.ts src/features/application/editor/workbenchPanelClose.test.ts
pnpm test:ts src/features/core/dataStore/graphProjection.test.ts src/features/domain/editorProjection/editorProjection.test.ts src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphEditorSync.test.ts src/features/application/graphEditing/graphConstantActions.test.ts src/features/application/editor/canvasDrop/dropGraphConstant.test.ts src/features/core/state/readonlySnapshot.test.ts
pnpm test:rs:package -p yss-graph-runtime --lib
pnpm test:rs:package -p yss-graph-execution --lib graph_preparation::
pnpm test:rs:package -p yss-application --lib session::components::tests::
pnpm lint:rs:package -p yss-graph-runtime --all-targets
```

本轮最终文档契约 6 项、8 个 TypeScript 文件与 6 份 Markdown 格式检查、独立全工作树 diff 检查
通过。最终 TypeScript 通过，前端 lint 仍仅三条既有警告；Runtime 包格式和 Clippy 结果沿用本轮
未再变动代码的验证。基准六份源码运行前后哈希一致，生产没有留下临时参考模块。完整 CI、真实
桌面和 Julia 完整计算链未运行，全部模块核销与 Graph 默认共享方案仍需继续。

### 并行接续：意图恢复、共享图表与函数成员恢复（2026-10-02）

上一轮已有实际修复和测量，本轮继续主代理与三个子代理并行。新增全文复核 App 的 17 个非 locale
数据 TS/TSX 入口：App/main、i18n 初始化、两个 Provider、UIHost、WorkbenchComposition、五个注册
入口和五个 integration；翻译正文及 CSS 不计入该声明。组装复用既有窗口、面板和 Application
能力，Plugin Context 只传稳定 registry，图表 Context 只传低频主题；没有因此新建共享业务状态。
另全文读取 fileResourceService、activityPanelService、projectEventStream 及 presentation 的页面/意图
hook 和 service。删除菜单 builder 未使用的 activeResourceRef 参数，授权仍由已有 capability 决定；
既有菜单、历史可用性和侧栏三文件 8 项通过，没有新增 UI 用例。

意图恢复有两个真实缺口。后台 pending 查询克隆当时回执，128 条广播 Channel 可在查询未返回时
再次报告 resync；原 hook 直接忽略第二次请求。其次，后台会将 30 秒未确认回执置 expired 并淘汰，
前端旧执行队列却可能仍占满 128 项，随后收到的新 pending 被丢掉后没有再次读取机会。现将原有
串行认领/确认和恢复流程放到具体 uiIntentDelivery，hook 只处理订阅与项目/挂载身份；读取期间的
新缺口用一个标记合并为后续只读查询，满载标记在原队列归零时消费并复用同一恢复入口。保持
原容量、ID 去重、后台认领、现有 execute 和项目失效检查，不重放已认领写操作。每次请求独立结束，
不把持续缺口的所有历史 Promise 串成等待链。

新增三项纯 Application 回归分别保护：在途成功读取后的新缺口、失败读取后恢复及绑定失效、满载
旧队列排空后的 pending 补读。前两项在保持旧行为的提取版本实际 2 失败；第三项随后实际 1 失败/
2 通过；最终 delivery、Service 和两个消费者 smoke 四文件 8 项通过。两个 smoke 分别只覆盖
Workbench loading 组装与 当时带 mock 的报告消费，不能算真实 hook 恢复集成。初次命令带了
不存在的 uiPresentation.test.ts，实际仅两文件 5 项，未将缺失文件算作通过。UI Contract README
同步临时队列和恢复归属；原生跨窗口恢复仍需人工验收。

共享 charts 的 24 个生产文件本轮全部逐读，另追踪 14 个 Chart/Results 消费者与适配器。
未发现写回输入、重复业务 Store 或在 renderer 重新解析 IPC；尺寸和 tooltip 保持组件生命周期。
LinePlotControls 原先漏传模型已有 referenceLines/xDomain/yDomain，现交给 LineChart 既有 domain
与 padding 规则，恢复 SCI 生存分析等真实来源的参考线。Heatmap 复用 D3 extent，一次扫描替代
两次数值数组与 Math.min/max 参数展开；当前原生 SCI 限制为 64×128 单元格，不能宣称已复现生产
大矩阵崩溃。八文件 35 项现有消费者通过，但不直接验证新增透传或大矩阵 DOM；未增加 UI 测试。

交叉检查补齐上一批安全日志的遗漏：ReportView 两个已有测试仍 JSON.parse 日志并要求原自由字段，
实际运行两项均失败，界面诊断断言原已通过。现同步原两例为完整固定 code 与安全身份字段，并明确
拒绝 outputPinId/fieldPath/reason 及原诊断正文；原 UI Alert 内容断言保留。与纯 reportViewIssue
合跑两文件 3 项通过，无新测试或生产改动，不把这项遗漏描述为此前已经验证。

Rust 本批全文补齐 Analysis parameter_projection、node_projection、port_projection、function_validation、
derived_ports、node_projection/interface，复读 concrete_interface；全文核对 Runtime semantic_cache/
connections 和 Editor projection/connections。参数默认/隐藏/非法显式值仍归 Protocol，连接候选复用
提交 planner，Application 重验资源/session/version；缓存取出/安装在锁内，解析和大对象释放在锁外。

发现函数成员恢复时事实不一致：旧 Orphan binding 的端口已由当前语义解析为有效，ABI 却只接受
存储 Resolved，误报 graph.function.abi_mismatch。现复用 port_projection 的 binding_origin，继续
核对当前端口 orphan、方向、精确类型及签名，保留 Entry/Return 完整实例地址且不改文档。一个新
Rust 回归实际旧红，验证缺失仍阻断、恢复后 ABI 地址正确和正文未变；Analysis 43、Runtime 15、
Execution 准备 6 项通过，另有 1 项既有手动 benchmark ignored。Analysis all-targets Clippy 无警告、
包格式通过，README 同步恢复规则。函数 lowering 仍未接入，不将语义修复描述为函数已可执行。

Graph 参数组/参数原来在 raw node 递归后又于实体安装时按 key 重做共享，现移到接收节点的唯一
规范化入口；bucket 直接借用 canonical parameterGroups。组及参数重排按 key 保留成员，已经共享
的 delta 保留输入组、数组和 projection 根身份。既有用例补充这些断言，未新增测试。

constants 的生产路径现在用一次 Immer 处理 UUID 表与 GraphConstantDto 固定字段；dataType、
dataValue、tabular 先在 draft 外按内容共享，再赋给固定字段，tags 按标量数组比较。任意 JSON 自有
`__proto__` 留在 opaque 值内，不能进入 Immer 字段赋值；同值返回旧根，已共享 delta 无写入返回
next，删除和可选字段依据原已验证 DTO 保留。既有常量用例继续覆盖特殊键，并补表格列同一边界。
A+B 的 11 文件 89 项及 TypeScript 通过，生产只保留这条 typed constants 路径；通用递归仍用于
明确 JSON 值及尚待收窄的 Graph 投影，不能因外壳迁移便宣称整个共享问题完成。

本轮在独占窗口复用同一 500 常量、两组 16 项 List 的三场景，加入 typed Immer 作为第三策略，
每组真实 30 样本。三者共享相同准备、发布冻结及计时外内容/引用断言，完整输出与源码哈希见
[typed constants 原始证据](../benchmark/probes/graph-constants-typed-immer-2026-10-02.txt)。上轮 raw 保留。

| 场景       | 现有通用 helper ms（RME） | 受限递归 Immer 参考 ms（RME） | typed Immer ms（RME） |
| ---------- | ------------------------: | ----------------------------: | --------------------: |
| 同值新快照 |          3.3538（±5.76%） |             30.4548（±0.99%） |      3.6592（±4.65%） |
| 单个改名   |          3.4024（±4.22%） |             30.9611（±1.01%） |      3.7549（±3.91%） |
| 嵌套值变化 |          3.3786（±3.54%） |             31.1632（±0.97%） |      3.7339（±3.53%） |

文件耗时 24.100 秒。typed 方案本次观察比通用 helper 多约 0.31–0.36 ms（9.1–10.5%），在声明的
500 常量场景采用这项绝对成本，以复用默认库管理固定外壳。一次固定登记顺序、同进程比较不等于
随机多轮因果结论；受限参考仍不支持全部合法 JSON，typed 路径也仍借用 opaque 共享，不能用此
批准递归叶或全 Graph 的规则豁免。没有测内存或原生 UI，也没有重新宣称上轮空 constants 整图数据
代表当前实现。

```powershell
pnpm test:ts src/features/application/presentation/uiIntentDelivery.test.ts src/services/workbench/presentationService.test.ts src/app/windows/workbench/WorkbenchComposition.test.tsx src/modules/results/internal/ui/info/LinearRegressionComponent.test.tsx
pnpm test:ts src/app/windows/workbench/menuContributionRegistry.test.tsx src/features/application/editor/useEditorHistoryAvailability.test.ts src/tests/integration/activityPanels/sidebarEmptyStates.test.tsx
pnpm test:ts src/features/application/presentation/LinePlotControls.test.tsx src/shared/charts/cartesian/LineChart.test.tsx src/features/application/presentation/PlotResultView.test.tsx src/shared/charts/ChartRenderer.test.tsx src/features/application/presentation/toResultChartModel.test.ts src/features/application/chart/toChartModel.test.ts src/shared/types/dto/plotPayload.test.ts src/features/application/presentation/loadPresentationWindow.test.ts
pnpm test:ts src/modules/results/internal/ui/info/ReportView.test.tsx src/features/application/observability/reportViewIssue.test.ts
pnpm test:ts src/features/core/dataStore/graphProjection.test.ts src/features/domain/editorProjection/editorProjection.test.ts src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphEditorSync.test.ts src/features/application/graphEditing/graphConstantActions.test.ts src/features/application/editor/canvasDrop/dropGraphConstant.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/core/dataStore/nodeView.test.ts src/features/application/editor/setNodeParameters.test.ts src/modules/details/internal/ui/node/parameterEditors/NodeParameterEditor.test.tsx src/modules/details/internal/ui/node/parameterEditors/RelationalParameterEditors.test.tsx
pnpm test:rs:package -p yss-graph-analysis --lib
pnpm test:rs:package -p yss-graph-runtime --lib
pnpm test:rs:package -p yss-graph-execution --lib graph_preparation::
pnpm lint:rs:package -p yss-graph-analysis --all-targets
pnpm bench:graph --testNamePattern='^500 constants:'
```

本批最终 TypeScript 检查、文档契约 6 项、12 个 TS/TSX 与 5 份 Markdown 的格式检查及全工作树差异检查通过。
前端 lint 仍仅有三条既有警告；Analysis 包格式和 Clippy 沿用本批代码未再变动后的通过结果。基准七份源码在运行前后及最终格式检查后哈希一致。
完整 CI、原生桌面验收及 Julia 完整计算链本批未运行；全模块核销、任意 JSON 递归共享及已有 Application 异常根因仍开放。

### 并行接续：剩余求解链、Results 与前端边界（2026-10-02）

上一批属于实际进展：已有生产修复、测试及性能证据。本批仍由主代理与三个子代理分工，
分别推进前端剩余 owner、Graph typed 投影、Rust Analysis 与 Results；共同文件由单一代理编辑。

主代理全文读取当前 Domain 的 14 个生产 TS 文件：nodeCatalog 四项、resource 两项、
editorProjection 三项及 graphDiagnostics、graphConstants、databaseEditor、result、log。
生成诊断 JSON 不计本批全文覆盖。另完整核对 Core 的 graphInteraction、sidebar、sidebarDrag、
gesture、keyboard、dnd、theme、statusBar、nodeCatalog 和 canvasInteractionCleanup，当前共 28 个
TS/TSX 生产文件；并读完侧栏文档/展开 hook、目录 hook/browser、拖放/快捷键/取消直接调用链。
这些读取不等于其余 Core、Application 或桌面验收已经完成。

发现并删除一条没有生产消费者的数据链：useModifierKeyStore 在每次按键、松键、失焦及拖动时
写入全局修饰键镜像，但唯一读端仅向 CanvasDropHandler 传递修饰键，唯一生产处理器 useCanvasDrop
从未消费该参数。移除 Store、两层读取/透传和只服务该镜像的 keyup/blur 监听；键盘命令及画布原生
事件仍直接消费自己的修饰键。输入位置、项目身份、面板激活和注册清理保持既有 owner。
没有为预留参数保留旧接口，也没有新增 UI 用例。

该行为保持重构的检查实际共 9 文件 53 项通过。首个命令误带不存在的 canvasShortcut.test.ts，
实际只运行 5 文件 28 项，不把缺失路径记为覆盖。扩展调用方时曾误删同时导出 modal guard 的
keyboard/index.ts，useCanvasDrop suite 导入失败；已恢复 isAppModalOpen 的原导出，再通过该用例
及 editorCommandFocus 共 2 文件 18 项。其他两个投放消费者此前已实际 7 项通过，未伪称失败套件通过。

Domain 中原 node/utils/nodeClassNames 只包含 Graph 节点 CSS、背景及 Reroute 尺寸，sidebar/constants
只包含项目侧栏图标颜色。已分别移动到 graph-editor/internal/ui/Nodes/nodeAppearance 与
project-explorer/internal/ui/activity/resourceIconColors，四个界面消费者使用本地 import；删除旧文件与
仅为其服务的两个 barrel。原样式、尺寸和值保持，Graph README 与 Features 边界同步。
四文件 12 项现有呈现/侧栏消费者通过；没有新增 UI 测试，也不据此宣称已做视觉验收。

```powershell
pnpm test:ts src/features/application/editor/dropFunctionIntoEventEditor.test.ts src/features/core/sidebarDrag/canvasDropHandlerStore.test.ts src/features/core/dnd/dragEventInput.test.ts src/features/core/dnd/dndContracts.test.ts src/features/core/keyboard/canvasShortcut.test.ts src/features/application/editor/useEditorKeyboard.test.tsx
pnpm test:ts src/features/application/editor/useCanvasDrop.test.tsx src/features/application/editor/canvasDrop/spawnFromTemplate.test.ts src/features/application/editor/canvasDrop/dropGraphConstant.test.ts
pnpm test:ts src/features/application/editor/useCanvasDrop.test.tsx src/features/application/editor/editorCommandFocus.test.ts
pnpm test:ts src/modules/graph-editor/internal/ui/Nodes/RerouteNodeLayout.test.tsx src/modules/graph-editor/internal/ui/Nodes/DefaultNodeLayout.test.tsx src/tests/integration/activityPanels/SidebarResourceRows.test.tsx src/modules/project-explorer/internal/ui/activity/SidebarFileRow.test.tsx
```

Rust 本批完整补齐 document_index、semantic_validation、schema_resolution 及 aggregation/composition/
generated_tables/transforms、type_resolution 及 cache/constraints/domains/node_rules 共 12 个指定生产
文件，另完整复读 resolution。缺失端点不进入拓扑但保留阻断；方向、容量、literal 冲突与参数适用性
仍由现有 owner 判断。缓存核对参数、binding 顺序、地址、上游 Schema、列含义、常量和实际资源依赖，
命中保留 absent lookup；类型约束收窄至稳定后才执行最终类型、specialization 与诊断阶段，未按图规模截断。
这不是所有扩展协议组合或全仓行为的证明。

发现 Decompose 的成员恢复遗漏：当前端口已从旧 Orphan binding 恢复有效，但 composition 的列含义
读取只接受存储 Resolved，下游 Frequency 因而报告 schema_dependency_unresolved。现在复用已有
binding_origin，再按当前字段与 lineage 查找；不采用 last_known 类型，也不改写 GraphDocument。
一个新增 Rust 回归验证缺失→恢复→Numeric/Categorical 类型变化、增量/full 的结果与依赖一致，
binding 仍保持原样。旧代码实际 0 通过、1 失败、43 filtered；修复后 Analysis 44、Runtime 15、Execution
准备 6 项通过，另 1 项既有手动基准 ignored。Analysis Clippy 无警告，包格式及 README 检查通过。

```powershell
pnpm test:rs:package -p yss-graph-analysis --lib schema_resolution::tests::restored_decomposed_columns_refresh_consumers_through_orphan_bindings
pnpm test:rs:package -p yss-graph-analysis --lib
pnpm test:rs:package -p yss-graph-runtime --lib
pnpm test:rs:package -p yss-graph-execution --lib graph_preparation::
pnpm lint:rs:package -p yss-graph-analysis --all-targets
pnpm format:rs:package -p yss-graph-analysis
```

Results 本批全文覆盖目录内 25 个生产文件（含 components/renderers），以及 stats 的线性报告 hook、
execution 两个结果入口、Result Service 和 session Channel。查询继续只有一个 Zustand owner；展示和
搜索是只读派生，Context 只传展示模式。系数/结构化章节的语义检查补足通用分页契约，不重复解析 IPC；
网格只复制框架适配所需的页外层数组。独立窗口的 null 项目身份仍受原结果引用及租约约束。

修复三个不同回归：多上游 Pin 打开循环在第一项 await 后换项目，原来会在下一项重新捕获新项目并
继续旧点击；现复用循环开始的项目身份逐次重验。报告补选取得 lease 后，原来直接 retain/loadValue；
现复用 currentVersion 检查调用方、项目和图版本，失效先释放 lease。分页读取原来要求回复 offset
与请求严格相等，错误拒绝 Rust 普通值、报告表与结构化数组允许的 Math.min(offset,totalCount) 截尾；
现按原请求键读取 Service 已验证的页面，删除该镜像校验，Service 的响应关联和坏响应拒绝保持。
新增两个纯 Application 用例，第三风险补入既有 runtime 用例；旧实现实际 3 红/19 绿，修复后
3 文件 22 项通过，另 13 文件 39 项消费者通过。新增报告用例直接覆盖消费者失效，其余失效因素
共用既有 currentVersion；UI mocks 不能代替真实窗口验收。Results README 同步。

```powershell
pnpm test:ts src/features/application/execution/openInspectableResult.test.ts src/features/application/results/addLinearSummaryContents.test.ts src/features/application/results/runtime.test.ts
pnpm test:ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/results/resultLeases.test.ts src/features/application/results/pinResultSearch.test.ts src/features/application/results/inspectableResult.test.ts src/features/application/results/graphPresentation.test.ts src/features/application/results/usePagedResultRows.test.tsx src/features/application/results/resultReadErrors.test.tsx src/features/application/results/components/renderers/ResultRenderers.test.tsx src/services/result/resultService.test.ts src/features/application/presentation/loadPresentationWindow.test.ts src/modules/results/internal/ui/panel/ResultPanel.test.tsx src/modules/graph-editor/internal/ui/Pins/Pin.preview.test.tsx src/modules/results/internal/ui/info/LinearRegressionComponent.test.tsx
```

Graph C1 将根、节点、端口、参数组与参数的固定结构共享放入一次 produce(next)，沿当前 nodeId、完整
端口键与参数 key 寻找旧成员，固定标量字段浅比较。删除节点整树共享后的再次端口对齐；只有当前下标
无法匹配身份时才建立旧成员索引。全等返回旧根，已共享 delta 不写入并返回 next；重排保留未变实体，
旧数组顺序和对象不变。既有 Core 用例补参数值/输入默认值内合法自有 **proto**、新快照变化的旧子枝
共享与同版本全等身份，没有新增 UI 用例。11 文件 89 项通过，主代理交叉核对固定 DTO 字段齐全。

仍明确保留在旧共享边界的复杂 typed 字段包括 basis、connections、diagnostics、portInstanceAdditions、
typeState、resolvedSchema、参数 editor/valueType；真正任意 JSON 为参数 value 与 input 的 literalOverride/
protocolDefault。前批 constants 的递归分支也仍在。它们不是因 C1 完成便获豁免，也不能全部称为 opaque；
本批仅推进固定拓扑部分，没有宣称全 Graph 已符合默认方案。

```powershell
pnpm test:ts src/features/core/dataStore/graphProjection.test.ts src/features/domain/editorProjection/editorProjection.test.ts src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphEditorSync.test.ts src/features/application/graphEditing/graphConstantActions.test.ts src/features/application/editor/canvasDrop/dropGraphConstant.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/core/dataStore/nodeView.test.ts src/features/application/editor/setNodeParameters.test.ts src/modules/details/internal/ui/node/parameterEditors/NodeParameterEditor.test.tsx src/modules/details/internal/ui/node/parameterEditors/RelationalParameterEditors.test.tsx
```

本批 C1 单独运行现有 5,000 节点安装基准；无其他 Cargo/前端测试同时运行。计时只覆盖安装，
同步、准备与内容/身份/冻结断言在计时外。delta 新补 next 投影根、发布凭据、位置与未变节点/pins/
顺序引用检查。既有 time=500、iterations=30、warmupTime=100 保持，因此真实样本数以输出为准。

```powershell
pnpm bench:graph --testNamePattern='^5000 nodes: (delta adoption|existing graph .* snapshot adoption)$'
```

| 当前安装场景           | mean ms |     RME | 实际样本 |
| ---------------------- | ------: | ------: | -------: |
| delta                  |  2.7672 |  ±1.17% |      181 |
| 同值完整快照           | 88.8806 |  ±2.57% |       30 |
| 单节点移动完整快照     |  100.23 | ±25.65% |       30 |
| 节点和端口重排完整快照 |  154.57 |  ±0.64% |       30 |

Vitest 文件耗时 40.970 秒，单节点移动出现 464.52 ms 最大值，离散明显，不能将该均值作为稳定精确估计。
delta 沿用每节点一个端口且持续移动的原 fixture，完整快照为每节点两个端口、每次恢复同一基线并读取
新对象的 fixture；不能横向当相同工作量比较。七份源码运行前后哈希一致，完整命令、输出和环境见
[typed topology 原始测量](../benchmark/probes/graph-typed-topology-2026-10-02.txt)。
历史 raw 的源码不同，本批没有声称改造加速；也未测 allocation、IPC、React 或桌面绘制，
这些数值不批准剩余递归边界的规则例外。

交叉复核另外确认并移除目录搜索的重复工作。原构建器已规范化字段，matcher 每次仍重新规范化、
去重、拼接全部元数据，Core 又给每个条目重复准备同一 query。现在 Domain 构建时直接返回最终文本，
删除仅被测试读取的中间字段模型；Core 仍复用原响应对象 WeakMap，每次查询只调用一次 catalogSearchTerms，
matcher 只执行多词 AND 子串匹配。拼音生成原来已经缓存，不把它误报为此次才消除的工作。
原 Unicode/NFKD、非 locale 小写、全拼/首字母、正文排除、空/纯符号查询、条目顺序/引用及响应身份
隔离继续验证；节点文档仍只搜索标题和别名。复用六文件 53 项已有用例，无新 UI 测试、缓存层、
benchmark 或数值加速声明；Features README 同步当前搜索契约。

```powershell
pnpm test:ts src/features/domain/nodeCatalog/searchDocument.test.ts src/features/core/nodeCatalog/localizedSearchIndex.test.ts src/services/nodeSystem/catalogSearchWireGolden.test.ts
pnpm test:ts src/modules/graph-editor/internal/ui/NodePalette.test.tsx src/modules/node-catalog/internal/ui/activity/SidebarNodesTab.test.tsx src/modules/node-catalog/internal/ui/documentation/NodeDocumentationModal.test.tsx
pnpm check:ts
pnpm lint:ts
```

主代理另完整补读 Analysis 的 lib、analysis、schema_state、semantic_snapshot 和 result_category，
核对封装快照、借用 ready view、basis/Arc 绑定、执行指纹及结果分类的归属，没有新增生产修改。
README 的状态枚举补齐源码已有 Deferred。结合前批七个投影/ABI 文件和本批十三个求解文件，
Analysis 当前 25 个生产 Rust 文件已累计逐读；单独测试文件和全部扩展协议组合不因此视为全文或行为完备覆盖。
本批跨代理只读审查确认 modifier 删除、呈现文件迁移与 C1 固定字段、身份和特殊 JSON 键处理未发现遗漏。
参数跨组移动不保证复用另一组的旧对象，保持原匹配范围；没有因此修改内容或增加全局参数索引。

本批最终 TypeScript 与 lint 通过，后者仍仅三条既有警告；文档契约 6 项、28 个 TS/TSX 与 6 份
Markdown 的格式检查、独立全工作树 diff 检查通过。七份实测源码在最终收尾仍与 raw 的 post-run
SHA256 一致。Rust 结果复用本批未再变动源码的通过输出；没有启动全 CI 或原生桌面/Julia 完整链验收。
全模块目标仍在进行，剩余复杂 typed/任意 JSON 共享及其他 crate 内部仍继续复核，没有批准规则例外。

### 并行接续：Database、项目生命周期与插件协议（2026-10-02）

本批继续主代理与三个子代理并行，按文件 owner 分工；Cargo 检查依次使用构建目录，
交叉审查只在明确移交后修改另一模块。没有重复空字段 Graph benchmark，也没有批准规则例外。

Graph C2 在 C1 同一次 Immer 事务中继续处理固定连接端点、诊断位置/相关位置、Schema 字段、
端口添加项及 basis/typeState 外壳。字符串字典只浅比较，保持数组原有位置匹配与输入次序。
现有 Core 测试另加一个纯回归，覆盖合法自有 `__proto__`、部分字段变化、Schema kind/null、
类型联合分支切换、同值旧根和已共享增量输入身份。L2 十一文件九十项、类型检查和 lint 通过；
lint 仍为三条既有警告。主代理对照 DTO 与接入代码复核未发现字段遗漏。
尚未核销的边界为资源观察对象字典、ValueType、参数编辑器规格、任意 JSON 及常量内部三类值。
C1 raw 对应旧源码，不能代表 C2；原样本中连接、诊断与 Schema 为空，本次不通过重跑它宣称新叶路径性能。
Features 与 benchmark README 已同步范围。

Database 延迟关系原实现允许两个解析者都计算 plan，`OnceLock::set` 的失败方仍返回自己的 plan。
后续 DropRows 等操作新建 row domain，因而同一 handle 的并发结果与缓存结果可能具有不同身份。
现在直接返回原 `OnceLock::get_or_init` 的胜出 plan，没有新增缓存、锁或数据模型。
新增一个纯 Rust 回归使用真实 Parquet 源，在同一线程 runtime 中确认两次首次 poll 均 Pending，
随后检查并发结果、缓存命中及两路 series 组合。实际旧实现在 row-domain 断言失败，修复后通过。
最初内存表版本在构造竞态时已经 Ready，其失败不计作缺陷证据。未声称已发生生产 Graph 错配事故。

```powershell
pnpm test:rs:package -p yss-database-engine --lib deferred_resolution_race_uses_one_row_domain
pnpm test:rs:package -p yss-database-engine --lib
pnpm test:rs:package -p yss-database-runtime --lib
pnpm test:rs:package -p yss-application --test numeric_execution project_dataset_graph_runs_through_application_authority_and_paged_results
pnpm lint:rs:package -p yss-database-engine --all-targets
pnpm format:rs:package -p yss-database-engine
```

此阶段 Engine 27、Runtime 12、Application 真实项目 DropNa 与分页消费者 1 项通过，
合计四十个不同用例，focused 不重复计数；Engine Clippy 无警告。Runtime README 同步缓存身份契约。
本批 Database 全文生产覆盖 37 文件：Runtime 的 lib、database_instance、database_state、edit_history、
error、plot_query、project_storage、session_api 与 runtime/{mod,physical,registry}；Store 的 lib、catalog、
codec、edits、gc、leases、paths、prepare；Engine 的 lib、aggregation、alignment、comparison、composition、
dataset、difference、drop_na、imputation、literal、page、profile、relation、series、series_transform、
table_transform、windows；Schema 的 lib。测试、examples、Arrow/IO/Source/Contract 未因此视为全文覆盖。
Store 租约锁跨 snapshot 登记与 GC 是现有保护边界，提交后的待发布记录也保护新快照，未机械移除该锁。

Database 同一全文审查又定位到 Numeric 语义准入的绕行：整数 Sum 按 Physical 分支执行，
Difference/PercentChange 也未像相邻数值分支一样检查 Semantic。Data Contract 和节点目录均明确
要求 Numeric，NodeKernel 的现有 Series 适配直接转交关系 owner，不能由整数存储猜测语义。
新增一个复合纯回归先验证同为 Int64 的 Numeric 输入得到精确 sum=14、difference=[null,2,4]、
percent-change=[null,1,1]，再验证 Identifier 三种操作均应拒绝；旧实现三项均未拒绝，实际用例失败。
修复复用 Engine 的 `is_numeric_field`，窗口数值分支统一准入，reduction 在 Length/Count 之外统一准入。
Shift、Rank、Fill 与计数继续允许原非数值语义，整数 Sum 的 Decimal128 精确路径不变。
最终 Engine 28 项与 Application 数列消费者 1 项通过；后者包含宽整数/溢出、空值、分组、标准化、
预算和取消。Engine Clippy、包格式及局部 diff 检查通过。结合上一阶段，本批 Database 共 42 个不同
用例通过，不把两个阶段重复运行的 Engine 用例相加；也不由关系入口缺陷推断 Graph 用户路径已发生事故。

```powershell
pnpm test:rs:package -p yss-database-engine --lib numeric_series_operations_require_semantic_admission_for_integer_storage
pnpm test:rs:package -p yss-application --test numeric_execution series_kernels_consume_arrow_nulls_metadata_and_grouped_order_with_budgets
```

主代理全文读取 Plugin protocol 五个 Rust 文件、schema 生成脚本和 SDK 的生产实现，并追踪宿主
process、package 清单验证及任务准入消费者。清单原来接受同一个 taskType ID 的不同产物规则，
而宿主使用首条匹配；现在复用已有 ID 集合拒绝同组重复任务 ID。新增纯协议用例先实际失败，
修复后验证独立 ID 与重复声明的两种顺序；未改变 wire shape 或引入第二份 schema。
SDK 原来分开保存 `alive` 与 pending，请求可在检查 alive 后、关闭清空 pending 后才登记。
现在原 pending 表以 `Option` 同锁拥有准入与关闭，关闭取走表后在锁外唤醒等待者。
新增一个 SDK 用例验证已登记请求被唤醒、关闭后拒绝新调用及重复关闭；它不强制原竞态的全部调度。
两个子代理只读复核锁顺序与响应/超时处理未发现阻塞问题。已准入请求仍可能正在发出，
关闭不证明远端从未执行；传输和进程停止继续归 process owner，SDK README 明确这一界限。

```powershell
pnpm test:rs:package -p yss-plugin-protocol --lib
pnpm test:rs:package -p yss-plugin-runtime --lib --test installation
pnpm test:rs:package -p yss-plugin-sdk --lib
pnpm lint:rs:package -p yss-plugin-sdk -p yss-plugin-protocol --all-targets
pnpm format:rs:package -p yss-plugin-protocol -p yss-plugin-sdk
pnpm plugin:julia:package --dev
$env:YSSBI_PLUGIN_TEST_PACKAGE = 'G:\006RustProject\YssBI\target\plugin-packages\yssbi.julia-0.2.0-dev.1790939008718.b62dc5e7d339-x86_64-pc-windows-msvc.yssplugin'
pnpm test:rs:package -p yss-plugin-runtime --test native_extension view_ -- -- --ignored --nocapture
```

协议三项、清单修复后的运行时/安装九项通过；SDK 最终实现一项与含该实现的真实插件进程三项通过，
两个 crate 最终 Clippy 无警告。开发包保留既有网页 chunk 大小警告。首次 native 命令被 pnpm 吞掉
Cargo 的参数分隔符，在参数解析阶段失败，未运行测试；上列双分隔命令实际运行三项，完整 Julia 用例
被过滤。构建期间删除 SDK 冗余早期存活检查后重新构建最终包，实际 native 用例使用上列最终产物。
没有准备或运行 Julia 推断，也没有将局部进程验证记为完整插件计算链验收。

项目生命周期继续使用原 PublicationCoordinator、hydration 与回执登记表。
startProject/cancelProject 返回同步通知前捕获的准确身份，reset 的 NodeCatalog 通知后重验，
避免继续清除后继 Sidebar。hydration 每个发布步骤前后及每次 panel bind 前后核对该身份和发布水位；
prepared.commit 返回实际提交身份，await 后不再重新捕获一个可能已经属于后继的 epoch。
Results 项目失效重置移入成功回执结算入口，直接响应与事件共用一次处理，stale/duplicate 不清后继。
处理 Promise 先登记再在微任务中执行原流程，封闭同步依赖重送同一回执时的重复结算窗口。

普通资源发布也保留同一 owner：空回执在 revision/catalog 通知后的检查完成前不删除 pending，
异常清理按原 pending 对象过滤；索引、面板与目录通知后均核对身份，不以旧 revision 清理后继同号请求。
driver 收尾仍按原 Promise 身份判断，不清理后继 driver。没有增加第二套发布队列或生命周期计数器。
三项新增纯回归分别覆盖 hydration 发布重入、直接/重复/迟到回执与同步重送、普通资源发布通知期间
替换项目；第三项是在解释其独立回归后追加。三项均实际旧红再通过，已有 UI 用例仅更新相关 mock。
普通发布旧红实际覆盖 receipt/index；修复后在 receipt/index/panels/catalog 四个通知点检查后继状态。
此前一次 fixture 阶段间未清理产生的额外失败不计作完整缺陷证据。最后受影响重测十二文件七十二项，
结合此前未受影响结果，项目代理本批实际去重为二十二文件一百二十一项；其中 Results runtime 随后
修改，由下方更晚的 Results L2 替代，两个集合不直接相加。主代理与另一子代理进行了只读交叉复核。
本支全文覆盖 project 目录全部十七个生产文件，另覆盖 lifecycle receipt/dependencies、useProjectSync、
appInitialization、useProjectOperations、PublicationCoordinator、Core lifecycle authority、Project event stream、
Database useDataLoader、usePresentationWindow、当时的报告页面加载器、DatabaseEditorWindow、Sidebar Store、Chart preview cache
与 Chart lifecycle coordinator 十五个直接链路文件；其他 Graph/NodeCatalog/资源候选实现只按调用片段核对。
Features 与 Rust Application Project README 更新对应契约。

```powershell
pnpm test:ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editorMutation/projectSnapshotResources.test.ts src/features/application/editorMutation/projectSnapshotMetadata.test.ts src/features/application/projectLifecycleReceipt.test.ts src/features/application/project/projectHydration.test.ts src/features/application/project/closeProject.test.ts src/features/application/project/projectEventConsumer.test.ts src/features/application/project/projectEventIngress.test.ts src/features/application/initialization/useProjectSync.test.tsx src/features/application/project/projectLifecycleOperations.test.tsx
pnpm test:ts src/features/application/project/projectLifecycleOperations.test.tsx src/features/application/project/projectIOStore.test.ts src/features/application/project/projectEventConsumer.test.ts src/features/application/project/projectEventIngress.test.ts src/features/application/project/projectWorkbenchLifecycle.test.ts src/features/application/initialization/useProjectSync.test.tsx src/features/application/initialization/appInitialization.hook.test.tsx src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editor/useProjectOperations.saveActiveFile.test.tsx src/features/application/editor/useProjectOperations.execution.test.tsx src/features/application/databaseEditor/useDataLoader.test.ts src/features/application/nodeCatalog/useLocalizedNodeCatalog.test.tsx src/features/application/nodeCatalog/useCompatibleNodeCatalog.test.tsx src/features/application/results/runtime.test.ts src/modules/results/internal/ui/panel/ResultPanel.test.tsx
```

Results 内部的项目重置也补齐同步通知后的所有权检查。原入口发布空查询投影后无条件清空
Execution outputRuns，通知期间建立的后继项目或同项目新 executionSession 会被旧尾部清除。
现在复用原项目身份与 executionSessionId，只有原 owner 仍有效且没有新会话时才继续；
prepareResultExecutionSession 返回是否仍可接纳，runtime 与 observeGraphRunEvent 的四处调用均据此停止旧流程。
新增一个纯 runtime 回归先实际复现后继 outputRuns 被清空，再覆盖跨项目及同 epoch 新会话两种重入。
原 null-project 独立结果读取用例仍通过；五文件 33 项 L2 通过，Results README 同步契约。

```powershell
pnpm test:ts src/features/application/results/runtime.test.ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/editor/observeGraphRunEvent.test.ts src/features/core/execution/useExecutionStore.lifecycle.test.ts src/features/application/editor/useProjectOperations.execution.test.tsx
```

本批最终全部生产变更冻结后，TypeScript 检查通过，lint 仍仅三条既有警告；文档契约六项通过，
十五个 TS/TSX 与八份 Markdown 的定向格式检查、三个 Rust 包的 fmt check、独立全工作树 diff 检查通过。
插件 runtime 的五项库测试及四项安装测试也已在最终 SDK 上重新通过。
尚未进行全 CI、原生桌面交互与完整 Julia 计算链验收；剩余共享边界、未覆盖模块与下列候选保持开放。

```powershell
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:rs:package -p yss-plugin-protocol -p yss-plugin-sdk -p yss-database-engine -- -- --check
git -c core.safecrlf=false diff --check
```

- [ ] 按上表完成全模块复查，再重新检查是否还有职责混合、重复规则、无用抽象、平行状态、旧代码和跨层调用。
- [x] 下一批已实际验证并修复视口清空的同步重入，以及 projectHydration 的 Loading 发布先于在途登记导致后继去重句柄被覆盖；旧红、新绿与消费者范围见下方接续记录。未由回归推定已发生生产故障。
- [x] 核对并修复数据库首次加载、导入元数据回执及 Details 缓存的版本关联；上述后端与非 UI 验证通过，桌面验收仍独立开放。
- [ ] 对任何拟保留的规则偏离补齐独立 benchmark 和行为证据；目前没有批准的例外。
- [ ] 桌面验收：导入对话框嵌套及取消、数据库版本变化与迟到元数据响应、Details 空 schema/未知计数显示、常量 JSON 草稿在无关更新时保持及切换图重置、进度更新、Assistant 流式状态与切换会话、文件输入保存/丢弃、分屏视口导航和取消恢复。
- [ ] 插件完整计算链验收：现已生成开发包并通过 3 项真实进程视图生命周期用例；Julia 环境准备、推断、任务取消及计算结果交接的完整原生用例仍未运行，局部进程检查不替代它。
- [ ] 定位 Application 库测试首次出现的 `0xc0000409 / STATUS_STACK_BUFFER_OVERRUN` 进程终止；两次完整复跑通过，尚无可定位的事件或转储，根因未确认。
- [ ] 全模块修复后完成最终的消费者、文档、格式和差异检查，并按完整原目标逐条核销；本轮局部检查不自动完成此项。

仓库中另有持续更新的 Meta 分析、统计目录、回归/推断及相关前端呈现改动，这些改动继续保留。
上述记录仅覆盖每批明确列出的范围；后续复核必须重新读取当时工作树，不能沿用本轮清单推定其最终合规。

### 并行接续：视口重入、类型共享与日期输入（2026-10-02）

本批继续主代理与三个可复用子代理并行，分别处理 Core 视口、Graph 共享、Application 生命周期和
Database 边界；完成局部修改的代理交叉复核其他分路，Cargo 工作按窗口串行执行。没有批准规则例外。

视口清空原先先发布 committed 状态，再无条件清除实时值；通知期间写入的后继手势会被旧尾部清除。
live Map 的直接遍历也会进入通知中新加的 entry，旧快照同步则可能覆盖通知中提交的新快照。
现在沿原每 pane Zustand store，在 committed 发布前捕获原 entry 和坐标引用，只清理仍属于该批的值；
同步和清理逐项检查既有项目身份，同步还检查 committed 根引用。没有新增 owner、计数器或深拷贝，
逐帧写入入口保持原实现；释放后仍按订阅数、空值与 entry 身份回收。

新增两个纯状态回归分别覆盖 committed-release 通知和 live/snapshot 遍历通知。19:15:58 实际旧实现
两项失败、四项通过，分别出现新坐标被清除和后继快照被旧值覆盖；修复后六项通过。
随后在第二项内补充换项目但坐标相等、存储引用不变的阶段，验证 epoch 检查仍保留后继实时值，
19:21:11 六项通过。未新增 UI 测试，另一代理只读复核清理、订阅回收及同项目重入没有发现问题。
19:19:32 受影响 L2 八文件三十四项通过，后续只有上述测试补充和格式调整。

```powershell
pnpm test:ts src/features/core/viewport/viewportSession.test.ts src/features/core/viewport/editorViewStateMemento.test.ts src/features/core/viewport/fitViewport.test.ts src/features/core/viewport/resolveInitialGraphViewport.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/project/projectHydration.test.ts src/features/application/editor/useCanvasViewport.test.tsx
```

Core viewport 全部十二个生产文件已全文核对：editorViewport、editorViewStateMemento、fitViewport、
resolveInitialGraphViewport、viewportTransform、liveViewportState、useViewportStore、viewportSession、
projectPath、persistGraphViewport、viewportScope 和 index。Features README 同步生命周期契约。

项目加载在 Loading 通知前登记原在途 Promise，微任务开始时与 Loading 通知后核对原项目身份，
收尾仅清理自己的 entry。新增一个纯 Application 用例在 19:13:55 实际失败：后继 B 重复发出索引
请求，重复调用最终取得 null；19:14:14 修复后 hydration 四项通过。原去重入口与状态 owner 保留。
19:15:21 L2 九文件四十项通过，另一代理复核 Promise 登记、错误捕获与迟到 close 未发现问题。

```powershell
pnpm test:ts src/features/application/project/projectHydration.test.ts src/features/core/dataStore/projectIOStore.error.test.ts src/features/application/project/projectIOStore.test.ts src/features/application/project/projectLifecycleOperations.test.tsx src/features/application/initialization/appInitialization.hook.test.tsx src/features/application/initialization/useProjectSync.test.tsx src/features/application/presentation/usePresentationWindow.test.tsx src/modules/results/internal/ui/info/ReportView.test.tsx src/services/workbench/presentationService.test.ts
```

本支全文覆盖 initialization 四个生产文件和 presentation 十一个生产文件；另全文核对 hydration、
projectRuntime、projectSession、useResultSession、useCurrentWindowActions、presentationService、
当时的报告页面容器，以及 Rust lease command、Application retention 和 ResultStore retention。
Tauri composition 仅补读 Destroyed 接入段。窗口 owner 已包含未认领 handoff，前端无需另建租约 owner。
Features 和 yss-ui-contract README 分别同步项目加载与订阅契约，真实 native/IPC 交互仍未验收。

Graph C3 继续在原各一次 Immer 事务内处理 ParameterEditorSpec 的固定分支和递归 ValueType。
新增 valueTypeProjection 由图投影与常量共用；只按 Array、DataSeries、OneOf 递归，其他联合成员
按其明确字段比较。graphMeta 的旧基底候选更新仍拥有外部输入隔离，本次接纳新输入、恢复已发布
引用的方向不同，未强行合并两种更新职责。剩余六处通用共享为资源观察对象字典、参数值、输入
literalOverride/protocolDefault、常量 dataValue/tabular；它们保持开放，未记作已批准偏离。

新增一个纯 Core 回归并扩写现有常量用例，覆盖关系编辑器、递归部分共享、Array→DataSeries、
谓词删除、变体切换、同值旧根与已共享增量输入；合法自有 **proto** 既有用例仍通过。
19:18:41 原十一文件 L2 九十一项通过，主代理对照当前 DTO 和 helper 复核未发现字段遗漏。
本批未跑 benchmark，历史空字段样本不能支持 C3 性能结论。Features README 与新增模块映射检查已同步。

```powershell
pnpm test:ts src/features/core/dataStore/graphProjection.test.ts src/features/domain/editorProjection/editorProjection.test.ts src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphEditorSync.test.ts src/features/application/graphEditing/graphConstantActions.test.ts src/features/application/editor/canvasDrop/dropGraphConstant.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/core/dataStore/nodeView.test.ts src/features/application/editor/setNodeParameters.test.ts src/modules/details/internal/ui/node/parameterEditors/NodeParameterEditor.test.tsx src/modules/details/internal/ui/node/parameterEditors/RelationalParameterEditors.test.tsx
```

Database Arrow 时间编辑与筛选字面量原来直接 strict_cast，纳秒输入可以静默截成微秒、非午夜可以
截成纯日期。现在复用原 lossless_cast，并由同一 owner 去除时区，保留原 wall-clock，不重复预处理。
仅扩写现有 edited_values_reject_overflow_and_decimal_truncation：实际旧红零通过、一失败、十过滤，
首个失败点为 Timestamp(us) 接受 1ns 输入，并不代表同一复合用例的全部分支都在旧实现执行；
修复后验证拒绝精度损失及合法纯日期、带偏移午夜、微秒、null。Arrow 十项、Store 两项定向消费者、
Engine 二十八项通过，共四十个不同用例；Arrow 原有一个手动计时用例 ignored。包格式与 Clippy 通过，
Store README 同步输入契约。没有新增时间测试函数或转换 owner。

```powershell
pnpm test:rs:package -p yss-database-arrow --lib
pnpm test:rs:package -p yss-database-store --lib datetime_column_cast_retains_clock_values_and_forced_nulls_in_persisted_data
pnpm test:rs:package -p yss-database-store --lib sparse_edits_merge_before_filters_and_nulls_survive_column_rename_and_reopen
pnpm test:rs:package -p yss-database-engine --lib
pnpm lint:rs:package -p yss-database-arrow --all-targets
```

Database 本批再全文覆盖二十六个生产文件：Arrow 的 lib、edit_type、conversion、scalar、schema、
temporal、semantic；IO 的 lib、csv、excel；Source 的 lib、batch、mysql、postgres、reader、runtime、
sqlite；Contract 的 lib、declaration、edit_state、engine、export、fingerprint、identity、observation、
session。另读四份 Cargo、全部既有测试与 canonical README；Store import/edit、Engine filter、
Application Excel import 仅补读相关段落，不重复计为全文覆盖。Text 对非字符串 Dictionary 的局部
校验虽宽松，但 Store import 已拒绝该物理类型，本批未证明真实入口缺陷，不凭推测扩大改动。

Application/window 十三个生产文件也已全文覆盖：createPersistedWindow、index、openDatabaseEditor、
openExternalUrlWithDialog、openLogsWindow、openPresentationWindow、PresentationWindowShell、
useCurrentWindowActions、useResultSession、useWindowDecorations、windowDecorationPolicy、
windowInteraction、windowLabels。该审查发现申请结果交接租约后没有重验前端项目身份，迟到租约
仍可创建无效结果窗口；后端 claim 会拒绝旧引用，不代表允许继续这次前端副作用。

openPresentationWindow 现在在 acquire 前捕获原 nullable lifecycle，返回后若身份或 epoch 已变化，
在原 try/finally 中停止，仍经 finish(false) 释放租约。合法 null 上下文仍可申请；已发出的原生创建
不由此检查撤回，Results README 明确界限。新增一个纯用例覆盖正常 null 成功、null→项目与同 ID
换 epoch；19:21:50 实际旧红一失败、五通过，两个失效阶段均曾调用 create 并 finish(true)。
19:22:04 修复后六项通过，19:22:23 受影响 L2 五文件二十二项通过。主代理只读核对原 finally 清理和
测试隔离，没有新增 UI 测试或原生窗口 owner，尚未进行真实 native 交互验收。

```powershell
pnpm test:ts src/features/application/window/openWindowHelpers.test.ts src/features/application/window/createPersistedWindow.test.ts src/features/application/results/resultLeases.test.ts src/features/application/execution/openInspectableResult.test.ts src/modules/results/internal/ui/panel/ResultContent.test.tsx
```

视口直接调用方另补测 graphDocumentUnload 与 workbenchPanelClose 既有用例，19:31:46 两文件
十九项通过。两生产文件与测试已全文读完；以下新候选仍只有源码依据，没有新增行为回归，留待下一批：

- graphDocumentUnload 在 Rust await 后检查项目与图 token，但 removeGraphSession 的同步通知之后，
  Results、交互、Execution 与 viewport 清理没有逐步重验。队列只在任务开始检查代次，不能覆盖此尾部；
  finishGraphUnloadLifecycle 原有 token 检查本身可靠。
- workbenchPanelClose 的 finalizeClosedPanels 不检查原 snapshot 身份，remainingEditors 仅捕获一次；
  关闭前授权不能保护通知期间换项目、同 scope 新开面板或后续面板的清理。下一批应验证重入与重新判断，
  不由本批既有绿灯核销该候选。
- resetGraphResultQueries 的投影通知后清 outputRuns、clearCanvasInteractionGraph 的回调后清交互/gesture
  也需一并验证。只在卸载外层添加检查不能自动保证子调用内部的后续清理仍归原操作所有。

```powershell
pnpm test:ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/editor/workbenchPanelClose.test.ts
```

日期分路的交叉复核还纠正了初版修复的输入兼容收窄。lossless_cast 原有固定格式 identity 不能覆盖
Arrow 已接受的短时间、AM/PM、小写 t、紧凑时钟、非补零日期、紧凑日期和带符号年份。
最终在原 semantic owner 中复用 Arrow 的 Date32、DateTime 与 Time 解析器，以日与日内纳秒比较身份，
不用新列出的格式集合维护另一份语法；纯日期路径只允许完整的数字/正负号拼写，不能吞掉时刻。
负 epoch 使用欧几里得除余；Time 整数 tick 继续按目标单位由原严格解析器准入，不新增日期数字推断。
超过九位的小数仅在丢弃部分全零时允许，避免 native parser 自身截断亚纳秒后产生虚假的相等。
仍只扩写原编辑输入用例，最后由另一代理对照依赖源码复核上述范围。

Excel 原 DateTime 的 Display 只输出 serial，Application 转 CSV 后会将日期推断成 Numeric。
现在 IO formatter 使用 Calamine 的 1900/1904 日历分量输出无时区毫秒文本；日期/纯时间都保留它
提供的日历与钟表，不额外推断 Time 类型。Duration 仍为数值天数，已有 ISO 单元格保持原文。
扩写原 formatter 并新增一个真实文件到 CsvBatchReader 的纯消费者用例：实际旧红一通过、两失败，
分别得到 serial 文本和非 Timestamp 类型；修复后 Excel 三项通过。消费者验证日历值、午夜进位、
null，以及 Duration 的 Float64=1.5，未把数字型成功读取误记为日期导入正确。

交叉复核另发现 Calamine 的毫秒舍入可能只把小时进到 24，未推进日期。扩写原用例实际得到
T24:00:00.000 后失败，修复后通过。普通日历复用现有 workspace Chrono 的日期进位，IO 显式增加
该依赖并更新锁文件，不增加版本或手写日历算法；1900 年的特殊进位仍由 Calamine 转换 ceil(serial)，
保留虚构二月二十九日。非负 serial 的 1900 年不可能来自 1904 epoch，未猜测其私有 epoch 字段。
极大数、非有限值、负数与进位超出 9999 年不进入有效日历格式化，沿原数值表示处理。
Runtime README 记录毫秒精度、日历 owner、Duration 与范围界限。仓库没有 xlsx fixture，本批未引入
工作簿写入依赖，也未把 CSV 消费者用例描述为真实 Excel 工作簿端到端验收。

日期兼容修正分别取得实际红例：亚纳秒阶段首个失败为 Time64(ns) 接受非零第十位小数；短时间
阶段首个失败为 02:10 被拒绝；纯日期阶段首个失败为 2026-9-6 被拒绝。各次均零通过、一失败，
不将首个断言之后尚未执行的分支另计为已复现；最终绿色实际执行全部新增断言。
最终八个 Database 文件定稿后，Arrow 十项、Engine 二十八项、IO 十项、Store 两项与 Application
导入/编辑/cast/undo/save/reopen 流程一项通过，共五十一个不同用例；Arrow 既有手动计时用例一项
ignored。该结果替代本批早期同集合结果，不重复相加。两 crate Clippy、包格式、两个 README 的
格式检查和限定 diff 检查通过，Graph 分路的最后只读复核也已完成。

```powershell
pnpm test:rs:package -p yss-database-arrow -p yss-database-io -p yss-database-engine --lib
pnpm test:rs:package -p yss-database-store --lib datetime_column_cast_retains_clock_values_and_forced_nulls_in_persisted_data
pnpm test:rs:package -p yss-database-store --lib sparse_edits_merge_before_filters_and_nulls_survive_column_rename_and_reopen
pnpm test:rs:package -p yss-application --lib project_import_edit_cast_undo_save_and_reopen_use_committed_dataset_snapshots
pnpm lint:rs:package -p yss-database-arrow -p yss-database-io --all-targets
```

本批最终 TypeScript 与 lint 通过，后者仍只有三条既有警告；模块映射、crate 依赖文档门禁通过。
文档契约六项、十二个 TypeScript 文件与六份 Markdown 的定向格式检查、Arrow/IO fmt check 及
独立全工作树 diff 检查通过。没有运行全 CI、PostgreSQL/MySQL 实库、原生桌面或完整 Julia 链路，
没有批准规则例外。上述开放候选、剩余通用 JSON 边界、其他未覆盖模块及最终整体核销继续进行。

```powershell
pnpm check:ts
pnpm lint:ts
pnpm docs:module-map:check
pnpm docs:crate-dependencies:check
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:rs:package -p yss-database-arrow -p yss-database-io -- -- --check
git -c core.safecrlf=false diff --check
```

### 并行接续：图释放重入、Binary 注解与动态键共享（2026-10-02）

本批继续采用主代理加三个复用 sub agent：主代理处理 Results/Canvas 内部释放与整合，插件分路处理
卸载、面板关闭及显式手势替换，Database 分路核对 NodeKernel 与 Harness，Graph 分路处理固定类型
共享与合法动态键 benchmark。生产修改完成后交换只读复核；计时窗口暂停其他代理的 CPU 密集检查。
没有提交、暂存或覆盖其他分路的 SCI、Meta、统计与 Labels 在途修改。

上一批末尾的 graphDocumentUnload、workbenchPanelClose、Results reset 与 Canvas release 候选
已取得行为证据并修复。卸载捕获原项目身份与图 lifecycle token，在 Rust await 和每次可能发布通知的
清理之后重验，并将相同 isCurrent 传入 Results 与 Canvas 内部。原 finishGraphUnloadLifecycle 的
token 收尾保持。面板关闭按每个 panel 重验原 snapshot，逐次读取当前打开的面板、scope 和 document；
同项目重开物理面板或 scope/document 后停止旧清理，不复用通知之前的 remainingEditors 快照。

两个调用方各新增一个纯 Application 回归。实际旧红为两失败、十九通过；19:42:57 两文件二十一项
通过，19:43:58 L2 八文件五十七项通过。Workbench README §6.1 更新契约，另一代理独立复核通过。
全文核对两个调用方、graphDocumentRetention、graphDocumentCachePolicy、graphPanelSession、
clearDetailFocusForClosedPanel 与 editorPaneStateStore；队列、项目加载和相邻清理只补读相关调用段，
没有据此扩大全文覆盖统计。已有键盘 UI 用例使用 mock，不代表原生面板交互验收。

```powershell
pnpm test:ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/editor/workbenchPanelClose.test.ts src/features/application/project/projectIOStore.test.ts src/features/application/editor/graphPanelSession.test.ts src/features/application/editor/openGraphInEditor.test.ts src/features/application/editor/editorPanelCloseCommands.test.ts src/features/application/editor/useEditorKeyboard.test.tsx src/modules/workbench/internal/application/workbenchLayoutActions.test.ts
```

Results reset 在原查询取消与失效处理之前捕获项目 lifecycle、resource session、resultExecutionSessionId
和 outputRuns 引用；invalidateOutputs 同步通知后同时重验这些 owner 和调用方 isCurrent，再清旧 runs。
查询协调器自身的 reset 只删除 Map，没有额外同步发布。本批没有增加结果缓存 owner 或更改 payload
retention。新增一个纯 runtime 用例覆盖同项目新 run 与通知期间换项目，后继 outputRuns 均保留。

Canvas 图/项目批量释放在回调之前一次摘除原有 cleanup buckets，后继同键注册进入新 bucket；逐个
回调核对项目身份与该图原 interaction。随后只删除仍指向原对象的 interaction，在一次 Immer 事务中
发布；gesture 清理还要求原 gesture 对象、发布后的 interactions 根与相应空范围仍有效。删去不再有
生产调用的 store.clearGraphInteraction，原测试改经真实 lifecycle 入口保留删除断言。新增一个纯 Core
用例覆盖图/项目范围、cleanup/发布两种重入阶段，验证后继 interaction、gesture 抑制与新 cleanup 保留。

19:42:22 以上两个用例实际旧红两失败、二十三通过。初次修复后 Results 复合用例第二阶段因第一阶段
正确保留了 runs 而未触发预期 pin 发布；逐阶段清空该测试的 Execution fixture 后，19:43:50 两文件
二十五项通过。该 fixture 修正不另计为生产缺陷。19:45:01 L2 十文件八十五项通过，初次 TS/lint 通过。
Features 与 Results README 同步契约；另一代理对照具体 owner 和通知边界只读复核通过。

```powershell
pnpm test:ts src/features/application/results/runtime.test.ts src/features/application/results/resultQueryCoordinator.test.ts src/features/core/canvas/canvasInteractionCleanup.test.ts src/features/core/graphInteraction/graphInteractionStore.test.ts src/features/application/editor/useCanvasInteraction.test.tsx src/features/application/editor/observeGraphRunEvent.test.ts src/features/application/editor/useProjectOperations.execution.test.tsx src/features/application/projectLifecycleReceipt.test.ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/editor/workbenchPanelClose.test.ts
```

继续核对显式 cancel/start 时，发现 cleanup 回调安装后继后，旧 cancel 仍将其置 idle；旧 start
也可能在换项目后安装请求，或覆盖取消通知安装的后继。即使 store 的启动通知已保留后继，原 void 返回
仍让 hook 重新读取后继并登记旧取消回调。该风险不同于上述批量 release 与 Results 发布，说明独立失败
方式后，在原纯 Core 文件新增本批第三个根分路用例，没有增加 UI 测试。

cancel 现在逐回调和提交之前重验项目及原 interaction；start 同时检查取消结果、当前 IDLE 和原项目。
Core startInteraction 返回实际安装的 structuredClone，通知后若对象或项目已变化则外层返回 null。
hook 直接持有成功返回的确切对象，失效即停止，不再通过 getState 认领后继；没有新增计数器或租约 owner。
四阶段复合回归在 19:48:11 实际一失败、十通过，19:48:47 L1 三文件十四项通过，19:49:10 L2 七文件
五十八项通过。最后一阶段的旧红是返回契约不能表明失效，不是声称旧 store 覆盖了该阶段的后继。
Database 代理对最终三个生产文件、nullable lifecycle 和 renderer lease 消费只读复核通过。
测试集合互有重叠，不将各轮数量累加为不同测试总数。

```powershell
pnpm test:ts src/features/core/canvas/canvasInteractionCleanup.test.ts src/features/core/graphInteraction/graphInteractionStore.test.ts src/features/application/editor/useCanvasInteraction.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx src/features/application/projectLifecycleReceipt.test.ts src/features/application/project/projectWorkbenchLifecycle.test.ts src/features/application/editor/graphDocumentUnload.test.ts
```

以上同步重入修复没有承诺 cleanup 抛异常后的完整重试；异常仍可能打断已摘除的旧批次。该行为尚未形成
经过真实生产路径验证的缺陷，也未新增通用异常 runner。真实 Canvas、FlexLayout 与 Tauri 交互仍未验收。

NodeKernel Boolean 原先只接受裸 Scalar/List，合法 Type Conversion 输出的 Annotated Binary 无法进入
AND/OR/NOT；直接去掉注解又会误解 positiveValue=false 的物理布尔极性。本批从原 conversion 提取同文件
pub(super) materialized helper，Boolean 对 Annotated Binary 复用原 Arrow 转换后读取裸 bool/null，其他
裸输入用 Cow 借用，lazy Series relation 路径保持。预算与取消仍由原 owner 处理，Boolean revision 3→4。
没有新增依赖，也没有覆盖 conversion/mod 中既有 Labels 修改。

只扩写两个既有测试函数。Kernel 实际旧红零通过、一失败、四十八过滤，首个断言是正极性 Annotated
Binary(true)→NOT；未把该失败后尚未执行的分支记作独立复现。真实图测试初版错误使用 result 输出端口
而得到 NotReady，不作为缺陷证据；改为真实 output 后，原生产实现对已 Ready 的
Text(false)→Binary conversion→AND→NOT 返回 Kernel(Failed)，零通过、一失败、六十一过滤。
最终绿色覆盖 scalar/list/null、两种 positiveValue、两侧广播以及真实图链。

最终 L2 共五十五个不同用例通过：Kernel 四十九项，图链一项、semantic_conversion 两项、
automatic_conversion 一项、既有 Labels 一项及 numeric extension artifact 契约一项。只读交叉复核通过。
Kernel Clippy 与格式、Application 测试文件 rustfmt、README 格式和六文件 diff 检查通过。Clippy 仍报告
SCI 依赖六条既有警告，位置为 categorical.rs:275/311、nonparametric.rs:387、sample_mean.rs:154/439、
variance.rs:134；本批未修改 SCI。未运行完整 Rust workspace CI 或原生图编辑验收。

```powershell
pnpm test:rs:package -p yss-node-kernel --lib
pnpm test:rs:package -p yss-application --test numeric_execution comparison_masks_feed_boolean_nodes_with_series_and_scalar_broadcasts
pnpm test:rs:package -p yss-application --test numeric_execution semantic_conversion
pnpm test:rs:package -p yss-application --test numeric_execution automatic_conversion
pnpm test:rs:package -p yss-application --test numeric_execution value_labels_preserve_lazy_series_codes_nulls_and_ordered_meanings
pnpm test:rs:package -p yss-application --lib numeric_extension_uses_actual_capabilities_and_rejects_old_artifacts
pnpm lint:rs:package -p yss-node-kernel --all-targets
```

NodeKernel 本批全文覆盖十一生产文件：lib、error、identity、invocation、registry、value，以及 builtins
的 mod、numeric、boolean、comparison、conversion。其他 relation/series/aggregation/alignment/transform/
distribution/SCI adapters 尚未因此核销。Numeric 消费带 dummy_base_level 注解的值仍只是源码候选，
尚无实际生产缺陷证据，不因相似性直接扩大修改。

Harness 再完整核对 Core events/ports/host/mod/host/turn、SQLite events、Rig stream、Application
runtime/harness 与 command_harness、Channel harness 的生产实现，内嵌测试只读相关部分。
EventWriter 保留 append→publish 异步 gate：
SQLite BEGIN IMMEDIATE 拥有序号分配与提交，Host gate 拥有 generic sink 的追加/发布顺序，两者没有
建立平行序号模型。Channel 在 hub lock 外发送，由每个 subscription drainer 保序；未找到真实
callback→Host.append 的生产闭环，cancel_turn 也不获取此 gate。慢 sink 会延迟跨 session 与 terminal
发布，这是背压而非已经复现的死锁。移除 gate 会改变现有顺序契约，本批无修改，也无规则偏离批准。

Graph C4 将 resourceObservations 的固定 {kind, version} 字典与 tabular 的 {columns: scalar[]} 改为
具体类型比较，数组元素使用 Object.is。仅在恢复旧分支时惰性复制一次普通字典，自有 `__proto__` 在复制
时成为数据属性，动态键不进入 Immer draft 路径。保持旧根/next 增量根、删除、缺省、null 与 constructor。
DTO parser 对 tabular 的严格 columns 形状已核对。只扩写两个既有纯测试，19:44:27 十一文件九十一项
通过；剩余 shareProjection 生产调用从六处减至四处：参数 value、输入 literalOverride/protocolDefault
与常量 dataValue。没有因此默认批准这四处，也不使用此前不同 fixture 的 C1 数据宣称本批提速。

为常量边界公平对照，更新既有 benchmark reference 与 fixture：普通对象/数组仍使用一次 Immer 事务，
仅对输入中具有自有 `__proto__` 的字典子树，在任何 draft 写入之前退回既有 shareProjection。依赖 proxy
的 setter 分派先匹配原型 setter，即使有同名自有属性也会触发禁止的 setPrototypeOf，故不能简单通过
draft 写这个合法 JSON 键；constructor 则沿普通路径。该参考只存在于 benchmark，不进入生产导入图。

新增一个纯 parity 用例，在计时之外核对值、旧分支/根和 next 增量根身份、`__proto__` 替换/删除、
constructor、数组重排、整个常量删除、可选字段删除与 tabular 动态列。19:51:37 十一文件九十二项
通过；最后仅将测试删除操作改为 Reflect.deleteProperty 以满足 TS，19:53:05 定向十三项通过。
最终 check:ts 与 lint:ts 通过，仍只有三条既有 TS lint 警告。benchmark 结果与最终文档门禁另附于下。

19:53:54–19:54:20 完成两策略、四场景共八组计时，所有组 N=30、time=0、warmupTime=100ms。
每次准备的 500 个 Object 常量都经过同一 parser 与冻结入口，JSON 构造、校验、输入冻结和断言在计时
之外，两策略计时都包含共享与同一 freezePublishedValue(result)。每个常量含两个十六项 List、普通
metadata、自有 `__proto__` 两项 List 和 constructor Object；增量场景验证输入根身份被原样保留。

| 场景                            | 当前 typed production，均值 ms / RME | 单次 Immer 加特殊字典回退参考，均值 ms / RME |
| ------------------------------- | -----------------------------------: | -------------------------------------------: |
| 新鲜等值快照                    |                      4.5066 / ±4.80% |                             31.3138 / ±1.35% |
| 仅名称变化                      |                      4.2612 / ±1.48% |                             31.4255 / ±1.40% |
| 嵌套变化、键删除/新增、数组重排 |                      4.3196 / ±1.54% |                             33.1509 / ±1.20% |
| 已共享增量                      |                      0.0713 / ±2.95% |                              0.1083 / ±2.56% |

原始数据、完整命令、环境与九个测量源码文件的前后 SHA256 见
[动态键常量基准](../benchmark/probes/graph-constants-dynamic-keys-2026-10-02.txt)。所有源码哈希一致，
Vitest 文件耗时 21.795s，包含环境和记录准备的 wall time 25.3308s，exit 0。环境为 Windows 11、
i9-13900K/32 logical processors、Node 24.19.0、pnpm 12.3.4、Vitest 4.1.10。计时期间团队没有运行
其他重检查，但没有隔离机器的外部进程；共享 dirty workspace 当时有 1089 项，不能视为干净提交复测。

本结果支持保留当前 constants 整体路径，不采用这份更慢的整体通用 Immer 参考替代。参考仍对特殊
字典使用原 helper，不能称为纯 Immer 完整替代；外层 typed 字段比较也不同，不能把全部差额归因于
dataValue 或特殊键回退。固定顺序、单进程、单 fixture 不支持跨机器、参数、端口、所有 JSON 路径或
桌面帧率结论。若要单独核销 dataValue 的最小性能例外，还需保持相同 typed 外层、只替换 dataValue
遍历的对照，或说明那个具体默认实现的能力不足。本批没有批准通用共享引擎的全局例外。

原始记录最初由 PowerShell 混合写入 UTF8 header/footer 与 UTF16LE stdout；已按确切字节边界解码
stdout 并统一为 UTF8，保留全部输出、重复表格和原生 stderr 呈现，没有删样本或重跑。主代理复核
八行数据与前后哈希，插件代理独立复核参考的回退时机、父路径和准备/冻结公平性，均未发现遗漏。

最终 TS 与 lint、十六个 TypeScript 文件和六份 Markdown 定向格式、Kernel fmt、Application 测试
文件 rustfmt、模块映射与全工作树 diff 检查通过；19:57:00 文档契约六项通过。没有新增依赖，不为此
重复未受影响的 workspace 检查。整个目标继续进行：其他未全文覆盖模块、剩余三个 opaque 调用与
dataValue 最小对照、Numeric 注解候选以及真实 native/Julia 验收仍未整体核销。

```powershell
pnpm check:ts
pnpm lint:ts
pnpm docs:module-map:check
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:rs:package -p yss-node-kernel -- -- --check
pnpm exec rustfmt --edition 2024 --check src-tauri/crates/yss-application/tests/numeric_execution.rs
git -c core.safecrlf=false diff --check
```

### 并行接续：保存准入、面板续接与 Dummy 类型声明（2026-10-02）

上一轮属于实际进展。本轮仍由主代理加三个复用代理推进：主代理复核剩余前端 owner 与图保存，
插件分路处理面板剪裁、激活和打开续接，Database 分路继续 Kernel 消费链及 Catalog 声明，Graph
分路补齐同一 typed 外壳下的 dataValue 直接对照。以当前工作树重新核对规则、实现和调用方，
没有将已有绿色测试自动视为整个模块已核销；基准计时期间暂停团队其他 CPU 密集检查。

主代理本轮全文读取 Application/graphEditing 的二十六个生产 TS 文件，包括十二个顶层实现/导出
与 commands 的十四个具体命令、注册及导出文件；commandExecutor.typecheck 不计生产覆盖。
另全文复核 Core/editor 八文件、Core/ui 三文件、Core/chart 两文件及 Application/chart 五文件。
图命令继续通过原 FIFO 提交 Rust 高层意图，前端没有建立第二份 Graph 历史；错误由 Application
映射，Resources/Execution 仍拥有投影。UIStore 的弹窗/进度和 Chart 的草稿、读取 token 继续使用
既有 owner，没有为了统一形式把面板局部状态改成全局状态。

saveGraph 在 beginGraphSave 通知后才读取 sessionId：监听器关闭图会使该读取抛 TypeError；同项目
重开则令旧保存借用新会话，发送旧意图并在失败时清掉后继保存锁。现在在通知前捕获原 sessionId，
begin 返回后立即核对项目及原 session；队列仍允许此前已准入的编辑完成，不用 projectionGeneration
锁死入场时的版本，也不新增保存计数器。一个纯 Application 用例覆盖关闭和同路径重开两阶段，
20:01:03 实际旧红一失败、七通过，包含真实多发 save 和 successor.saving 被清空；20:01:19 八项通过。

单图安装的布尔返回此前只说明 set 已调用。若 ResourceStore 同步通知期间卸载/重开或换项目，
Application 仍用旧 previous 对账后继 Results，并清掉后继执行请求。Core 现有安装入口在通知后
比较当前 session 与本次 prepared session 的 sessionId/projectionGeneration，被替换则返回 false；
Application 原有失败分支即停止旧对账，无需另建回执 owner 或扩大公共签名。第二个纯用例实际复现同项目重开及换项目
后执行请求丢失，20:03:44 一失败、八通过，20:04:09 九项全绿。它证明 ResourceStore 发布通知这一
边界，不据此宣称 Results 对账内部所有通知与异常路径已完成整体证明。

20:05:51 L2 十文件四十五项通过，覆盖共享安装、常量编辑、保存、资源、只读投影、项目发布和保存/
执行消费者。Features README 同步保存准入与安装确认契约。本批两个新用例均为纯 Application，
现有 UI 用例仅作为消费者回归，未新增 UI 单元测试。

```powershell
pnpm test:ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/graphEditing/graphConstantActions.test.ts src/features/application/graphEditing/subgraphExportCoordinator.test.ts src/features/core/resource/resourceStore.test.ts src/features/core/dataStore/graphDocumentLoadPolicy.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/project/projectIOStore.test.ts src/features/application/editor/useProjectOperations.saveActiveFile.test.tsx src/features/application/editor/useProjectOperations.execution.test.tsx
```

面板剪裁在 commit 前虽有授权，await 返回后仍会释放新项目或同 ID 重开的 pane。现在逐个释放前
重验原 nullable lifecycle，并检查该物理 panel 是否已再次出现。激活在 Details 同步通知后、图焦点
发布后重验原项目和当前原生活动 panel，旧 Chart 激活也不能清除后继图焦点；Details reveal 返回后
继续检查原 epoch。两项纯 Application 回归在 20:00:20 实际两失败、八通过，20:00:32 十项通过。

打开流程另有不同的跨 await 准入风险：Workbench 的 generation 能拒绝已排队旧任务，但 A 的目标组
查询刚完成、B 已使队列换代后，A 再提交 open 会取得 B 的新 generation。第三个回归明确针对这一
独立风险，使用真实 LayoutModelBinding、运行时队列和项目代次，未挂载界面。20:02:46 旧实现一项
失败：旧 Chart 被物理加入新项目布局，迟到 DB 继续 reveal，Graph 调用方续接间隙仍安装视口和 reveal。

openEditorPanel 现在在目标组与 open 两次 await 后检查原项目，失效拒绝复用既有 handled rejection
集合停止旧打开错误反馈。Graph/File/Database 调用方保留自身原项目检查，防止 helper resolve 到
调用方续接之间换代；原 Workbench generation 继续负责已入队请求，没有新增布局 owner 或 guard API。
20:03:20 打开三文件三项通过；20:05:03 整批 L2 十二文件六十项通过。Workbench README 同步契约。
此错误反馈结论只覆盖目标组和面板打开；共享 revealWorkbenchView 的错误反馈仍需另行核对。

```powershell
pnpm test:ts src/features/application/editor/pruneEditorPanels.test.ts src/features/application/editor/editorPanelActivation.layout.test.ts src/features/application/editor/openEditorPanel.test.ts src/features/application/editor/openFileInEditor.test.ts src/features/application/editor/openGraphInEditor.test.ts src/features/application/editor/editorOpenTarget.test.ts src/features/application/editor/revealGraphProblem.test.ts src/features/application/editor/useCanvasInteraction.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx src/tests/integration/activityPanels/SidebarResourceRows.test.tsx src/modules/workbench/internal/application/workbenchLayoutActions.test.ts src/features/application/project/projectWorkbenchLifecycle.test.ts
```

saveAllDirtyDocuments 的 barrier、每次 save 及错误反馈前后已有 active/project 检查；settleEditorFileEdits
只等待去重后的原 handler barrier，没有跨 await 尾部写。Chart lifecycle 的 token Map 不同步通知，
rename 继续在 await 后检查 context/token，本批无需修改。相邻 createFileActions 在 flush 后重新捕获
项目、discard 尾部，以及 panel publication 释放尾部目前仍只是源码候选，未作为已复现缺陷记账。

Kernel 的 numeric 注解候选经真实生产链排除：annotate_dummy 只接受 Categorical/Ordinal/Binary 的
lazy Series；物化预测保留分类标签元数据，转换到 Numeric 时 PreparedConversion 清除 dummy_base_level，
RuntimeValue::with_metadata 随即返回裸 Numeric。mask/choose 使用 Engine 既有 binary_expression：
Boolean 的 positiveValue=false 取反，其他编码按 positive_value 比较，不重复上一轮 Boolean 漏检。
series::load 与 project_series 检查行域，transforms::operands 复用 check_row_domain；aggregation/
alignment 下发中立关系请求，由既有 Engine 负责扫描和错误。这些 Kernel 生产文件保持不变。

继续向上追踪后证实真正偏差在 Catalog：DummyInfo 原输入/输出声明为无约束 Generic(element) 且
typing 为 Fixed，令 Numeric 数列错误地获得 Ready，直到执行才失败。现用既有三种分类数列 Union
限制输入，typing 复用 Identity，合法 Categorical/Ordinal/Binary 输出仍解析为对应 Exact 类型。
只扩写 Analysis 现有 semantic_conversion_tracks_input_shape_and_target_changes；首个 Numeric
断言实际旧红零通过、一失败、四十三过滤，绿色执行三种合法和四种非法语义的全部断言，非法连接
具有 TypeConnectionMismatch 定位。原 conversion 的后续形状/目标类型断言保留，没有增加测试函数。

最终 L2 六十六个不同用例通过：Catalog 十四、Analysis 四十四、Application numeric_execution 中
series_ 八项；消费者包含 Series 目录、metadata、null、预算及 Labels/Boolean/transform/constant/
unary/time_series 路径。两个 crate all-targets Clippy 无警告，两个 Rust 文件定向 rustfmt 与检查、
Catalog README 格式及限定 diff 检查通过。只改 Catalog dataframe/mod.rs、Analysis tests.rs 和
Catalog README，保留其他分路 Labels/SCI/Meta 修改，未新增依赖。主代理只读交叉复核声明和回归通过。

```powershell
pnpm test:rs:package -p yss-node-catalog -p yss-graph-analysis --lib
pnpm test:rs:package -p yss-application --test numeric_execution series_
pnpm lint:rs:package -p yss-node-catalog -p yss-graph-analysis --all-targets
```

Kernel 本批新增完整生产覆盖共十四文件：builtins 的 relational、series、aggregation、alignment、
transforms、distribution，statistics 的 mod、common、linear、survey/estimates，以及
statistics/common 的 inputs、finite、tables、models。linear 只全文核对生产区，内嵌测试未全文读取。
统计 serializer 在 JSON 前拒绝非有限值，模型 decode 在重建 JSON 前检查深度与预算；这些检查
不是可机械移除的重复校验，ScientificExecutionControl 沿原调用传递。Catalog dataframe、Graph
类型规则/缓存、Registry 指纹、Database Binary/Conversion/Dummy 以及相邻统计方法只补读相关段，
未将全部 SCI、统计方法或调用者预算记为已全面审查。

交叉复核进一步纠正了主代理初版安装确认的过度限制。最初用整个 session 对象引用判断，Save A
发布 saving=false 时，监听器启动同会话 Save B 只改变保存锁，也会令 A 错误返回 false；A 的 finally
随后清掉 B 的锁。扩写已有保存回归的第三阶段，在 20:10:03 实际一失败、八通过，A 结果和 B 锁均为
false。最终改用既有 sessionId/projectionGeneration：重开或新图帧使它们变化，begin/fail save 只改
saving，后续保存锁不否定前次安装。20:10:38 九项通过，另一代理只读复核队列前序编辑、结果摘要
同时接纳与该修复通过；没有新增第三个根分路测试函数，也没有保存 token 或并行 owner。

图编辑错误表的两个 `in` 检查也会错误接纳 Object 原型键，messageKey 可返回函数或对象；其中
constructor 能通过真实 lower_snake_case IPC parser，被误认作稳定业务拒绝码。两个查表入口改为
自有键检查，未知码仍走原失败流程；只扩写既有 unknown-code 用例。20:10:15 实际一失败、二十三
通过，四个失败断言分别为 messageKey 的 constructor/toString/`__proto__` 和真实规范化 constructor
错误。toString/`__proto__` 本就不能通过 wire parser，不将它们描述成后端有效错误码。

20:10:39 最终错误与保存消费者 L2 四文件五十六项通过，包含扩写后的 Save B 阶段；20:11:51 剩余
九文件三十六项通过，合并为十三个不同文件九十二项。该集合替代此前主代理四十五项的早期结果，
不重复累加。最终 TypeScript 与 lint 再次通过，仍只有三条既有 TS lint 警告。

```powershell
pnpm test:ts src/features/application/graphEditing/graphEditError.test.ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/graphEditing/editorCommands.test.ts src/features/application/editor/useCanvasMutationHandlers.test.ts
pnpm test:ts src/features/application/graphEditing/graphConstantActions.test.ts src/features/application/graphEditing/subgraphExportCoordinator.test.ts src/features/core/resource/resourceStore.test.ts src/features/core/dataStore/graphDocumentLoadPolicy.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/project/projectIOStore.test.ts src/features/application/editor/useProjectOperations.saveActiveFile.test.tsx src/features/application/editor/useProjectOperations.execution.test.tsx
```

constants.dataValue 的直接对照保持完全相同的 typed 外层、ValueType、tabular、一次 produce、输入
准备及发布冻结步骤，只替换 dataValue 的四个调用/比较/回填锚点。候选使用既有参考遍历，在访问
具有自有 `__proto__` 的字典 draft 之前回退到原共享 helper；未新增通用算法、第二次事务、生产策略
参数或永久 benchmark 导入。候选四文件六十一项 parity 和 check:ts 通过，覆盖值、旧/新根及分支
身份、名称、删除、变体、表格和已共享增量；随后精确恢复原生产字节。

```powershell
pnpm test:ts src/features/core/dataStore/graphProjection.test.ts src/shared/types/dto/editorMutationWireParser.test.ts src/features/domain/editorProjection/editorProjection.test.ts src/services/nodeSystem/graphEditorSync.test.ts
pnpm bench:graph --testNamePattern='^500 constants with dynamic keys: (equal|nested change) typed production$'
```

上面 benchmark 命令在 A/B/B/A 四个新进程各运行一次；A 为现生产，B 为仅替换 dataValue 的候选。
显示标签仍为 typed production，策略由每轮源码 hash 和 raw 标头明确。每轮只测 500 常量的新鲜同值
快照与第一条常量的嵌套变化，共八项，每项 N=30、time=0、warmupTime=100ms。准备、校验、输入冻结
与断言均在计时外，两种实现计时都包含相同 freezePublishedValue(result)。

| 轮次                 | 同值均值 ms / RME | 嵌套变化均值 ms / RME |
| -------------------- | ----------------: | --------------------: |
| A1 当前实现          |   4.3526 / ±4.70% |       4.3318 / ±1.27% |
| B2 仅 dataValue 候选 |  29.8407 / ±1.26% |      30.2174 / ±1.39% |
| B3 仅 dataValue 候选 |  29.6478 / ±1.02% |      30.2939 / ±1.26% |
| A4 当前实现          |   4.4885 / ±5.40% |       4.6064 / ±5.33% |

20:08:00.1107–20:08:27.4499 总 wall time 27.3575s。完整临时 patch、候选验证、环境、四进程原始
输出和九源码的每轮前后 hash 见 [dataValue 直接对照](../benchmark/probes/graph-constants-data-value-2026-10-02.txt)。
每轮以 finally 恢复原字节，最终生产及参考 hash 与记录相符；测试支持文件只导出既有 helper，
最终生产没有指向 benchmark 的 import。raw 为 UTF8、无 NUL，原生重复汇总表不是额外样本。

该数据与行为验证支持 constants.dataValue 这一最小边界保留原 draft 外共享，拒绝所测量的更慢
默认遍历候选。差值包含 draft/finalization 与相同发布冻结证明工作，不是单独遍历或特殊键回退的
self-time；候选仍依赖特殊字典 fallback，不能推导纯 Immer 一概不可行。这里只计时同值与嵌套
新鲜快照，名称/增量等只作行为验证。A/B/B/A 减少简单顺序漂移，但不隔离系统进程、GC 或机器差异；
当时共享工作树有 1098 项，并非干净 commit。不能外推 UI 帧率、IPC/Rust 或参数 value、
literalOverride、protocolDefault 三处。Features 与 benchmark README 同步这一局部决定，另外三处
及通用 helper 的其他路径继续开放；上一轮整体 constants 对照不再充当这次最小结论的替代证据。

本批最终十五个 TypeScript 文件与五份 Markdown 的定向格式检查、全工作树 diff 检查通过；
20:13:41 文档契约六项通过，包含模块映射门禁。Rust 格式/Clippy 复用本批定稿后的新鲜结果，未运行
完整 CI、原生桌面或 Julia 全链验收。整体目标仍开放：继续处理其余三个 JSON 边界、未覆盖模块，
并验证上列资源 flush/释放与 Results 内部通知候选；源码候选不作实际缺陷或完成证明。

```powershell
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts
git -c core.safecrlf=false diff --check
```

### 并行接续：Results 通知、资源续接与三 JSON 边界（2026-10-02）

继续采用主代理与三个复用 sub agent。主代理负责 Results 及报告；plugin_registry 处理资源与面板；
database_snapshot 核查统计适配层；graph_sharing_audit 核查剩余 JSON 共享。修改范围分开，完成后
交叉只读复核。Graph 计时窗口暂停团队测试、编译和 lint，不把同时运行的 Cargo/TS 成本混入对照。
本批开始共享工作树已有 1100 项改动，不暂存、不提交，也不覆盖其他 SCI/Meta/Labels 工作。

Results 的实际回归来自同步发布后的旧续体：执行会话准备期间重开图，旧摘要覆盖后继；摘要发布、
Execution 清理及 Pin 失效通知期间建立后继查询，旧对账再次发 Pin 请求，使后继请求变成 stale。
新增两项纯 Application 回归，通过真实 ResourceStore、ExecutionStore、Results 读取/协调入口触发，
只 mock Service 的 I/O。20:19:59 实际两失败、十五通过，十个失败断言；此前首次试跑的第二例因测试
构造的 semantic hash 不匹配中断，修正 fixture 后才取得上述完整旧红，不把构造错误当生产缺陷。

`publishGraphState` 在原执行会话准备与 Resource 发布之后，重验原项目 epoch、图 sessionId、代次、
hash 和执行会话。`reconcileGraphResultQueries` 同样绑定本次图帧、结果摘要引用与执行会话，在通知
之后检查；`reconcileCurrentResults` 用调用方的既有依据，在一次失效通知后停止旧 Pin 读取。
没有新增可写 owner、token、Store 或 Core 接口。图帧按原 sessionId/generation/hash 比较，saving
对象引用变化不否定合法操作；相同摘要继续由已有共享入口保持引用。

交叉复核补出同帧更高结果 revision 的变体：旧发布通知后借用新摘要也会多发查询。扩写第一项回归，
20:23:50 一失败、十六通过，实际多一次 Pin 读取且后继返回 stale。最终要求安装后当前摘要的执行
会话及 revision 对应本次回复，再进入对账；低版本被 Core 拒绝时，也不接管高版本 owner 的后续查询。
20:23:58 十七项通过；另一代理只读核对 Rust revision、Service、Core 共享及 saving 语义后确认。
20:24:18 Results、查询/租约、图展示、运行通知、卸载、图编辑、项目发布、资源和 Service 消费者
共十一文件六十八项通过。该集合包含本轮十七项，不重复累加；TypeScript 与 lint 同时通过，仍仅
三条既有 TS lint 警告。Results README 同步当前通知续接契约。

```powershell
pnpm test:ts src/features/application/results/runtime.test.ts
pnpm test:ts src/features/application/results/runtime.test.ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/results/resultLeases.test.ts src/features/application/results/graphPresentation.test.ts src/features/application/results/pinResultSearch.test.ts src/features/application/editor/observeGraphRunEvent.test.ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/core/resource/resourceStore.test.ts src/services/result/resultService.test.ts
pnpm check:ts
pnpm lint:ts
```

资源分路复现 rename/duplicate/save 在 `flushDocumentInputs` 等待后重新捕获后继项目并发送旧意图；
discard 的回执与输入丢弃通知之后也能清掉后继 dirty 或快照。现在调用前捕获原 ProjectCommandContext，
等待、输入通知及资源通知之后重验。该 context 只检查 project ID/epoch，flush 自身推进 publicationRevision
不会误拒正常保存。Workbench 发布回执后的 pane 清理逐项重验原 isCurrent 并读取 live panel；同 ID
重开或第一个 release 通知建立的第二个后继面板均保留。原事务、队列与面板 owner 不变。

两个纯回归实际 20:16:43 两失败、九通过，20:17:12 十一通过；20:18:51 受影响 L2 十三文件六十七
项通过，包含上述十一项。Resource 与 Workbench README 更新，六个文件局部格式与 diff 检查通过。
另一代理只读复核正常 flush 的接纳和逐 pane 的重验通过。该 L2 与 Results 集合有消费者重叠，不将
两组测试数简单相加为不同用例数量。

```powershell
pnpm test:ts src/features/application/resource/createFileActions.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts
pnpm test:ts src/features/application/resource/createFileActions.test.ts src/features/application/resource/docActions.test.ts src/features/application/resource/mindActions.test.ts src/features/application/resource/fileTextInput.test.ts src/features/application/resource/resourceActions.test.ts src/features/application/resource/fileManagement.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editor/workbenchPanelClose.test.ts src/features/application/editor/confirmDirtyEditorClose.test.ts src/modules/workbench/internal/layout/editorPaneStateStore.test.ts src/modules/workbench/internal/layout/workbenchNotifications.test.ts
```

Rust 分路全文核查 Kernel statistics 的 longitudinal、survival、inference、multivariate 四个生产文件，
最后一个包含其内嵌测试；Catalog 同名声明只读相关端口、参数和输出片段，不计全文。前三个中的
longitudinal、survival、multivariate 私有 ScientificComputationError mapper 与现有 common::computation_error
全部分支同义，删除三份重复规则并直接复用该 owner，README 补充职责。没有新增抽象、行为变化或
测试，不编造行为旧红。主代理复核取消、deadline、shape、parameter、其他 input 和 computation
六类分支对应关系一致。

保留经核对的不同业务约束：判别训练列共同对齐，新预测数据可以具有独立行域；竞争风险整数 cause
不转 Float64；生存预测 time/event/risk 作为同一关系交付；纵向 Gaussian mixed 与 GEE/GLMM 的工作区
估算不同；inference 成对比较依据真实类别对数量准入。没有证据将这些差异机械合并，也未将适配层
核查扩称为 SCI 算法全面验证。Kernel 四项与 Application 四项不同用例通过，后者包括实际十五个
生存节点及 PCA 到 limit 分页二十列的消费者。Kernel Clippy 无本 crate 警告，依赖 SCI 保留既有六条。
三个 Rust 文件定向格式/check、Kernel README 格式/check 及四文件 diff 检查通过。

```powershell
pnpm test:rs:package -p yss-node-kernel --lib longitudinal_adapter
pnpm test:rs:package -p yss-node-kernel --lib survival_
pnpm test:rs:package -p yss-node-kernel --lib multivariate_kernels_
pnpm test:rs:package -p yss-application --test numeric_execution longitudinal_
pnpm test:rs:package -p yss-application --test numeric_execution survival_category_
pnpm test:rs:package -p yss-application --test numeric_execution multivariate_coordinate_schema_
pnpm lint:rs:package -p yss-node-kernel --all-targets
```

Graph 分路完成 parameter.value、input.literalOverride、input.protocolDefault 三个普通 JSON 边界的
直接候选。它们是已验证 JSON，并非 SerializedDataValue 常量；各自允许合法自有字典键。临时候选
只替换这三个共享调用及原 typed 比较/回填，继续使用同一次 produce 和已有 restoreEqualBranches；
具有自有 `__proto__` 的字典在 draft 访问前回退到现有 helper。没有新增 produce、生产策略参数或
永久 benchmark 导入。候选通过九文件八十九项及 check:ts，三条路径分别验证内容、旧值隔离、
根和深枝引用、自有 `__proto__`/constructor、删除/新增键及已共享 delta。原生产 focused 十三项通过。

```powershell
pnpm test:ts src/features/core/dataStore/graphProjection.test.ts src/features/domain/editorProjection/editorProjection.test.ts src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphEditorSync.test.ts src/features/core/state/readonlySnapshot.test.ts src/features/core/dataStore/nodeView.test.ts src/features/application/editor/setNodeParameters.test.ts src/modules/details/internal/ui/node/parameterEditors/NodeParameterEditor.test.tsx src/modules/details/internal/ui/node/parameterEditors/RelationalParameterEditors.test.tsx
pnpm bench:graph --testNamePattern='^500 nodes with JSON values:'
```

基准经真实公开 prepareGraphSessions 入口，不为私有 helper 添加 API。500 个合成节点各有一个参数
及一个输入端口，三条 JSON 各独立分配，总共 1500 根；每值含两段十六项对象数组、嵌套字段及特殊
自有键元数据。仅第一节点的三值发生嵌套变化。输入物化、冻结、parser 与断言在计时外，计时包含
相同 typed 外壳、校验、实体派生和发布冻结，不含 Store 通知、IPC、Rust 或 UI。规模合法但不声称
是线上典型分布。A/B/B/A 四新进程各测同值、嵌套变化、已共享增量三场景，共十二任务，各 N=30、
time=0、warmupTime=100ms；完整 patch、环境、每轮十三源码 hash 及 stdout 见
[三 JSON 边界原始对照](../benchmark/probes/graph-projection-json-2026-10-02.txt)。

| 轮次            | 同值均值 ms / RME | 嵌套变化均值 ms / RME | 共享增量均值 ms / RME |
| --------------- | ----------------: | --------------------: | --------------------: |
| A1 当前实现     |  25.8446 / ±8.76% |      28.4182 / ±8.68% |       0.7206 / ±4.01% |
| B2 三 JSON 候选 |   267.07 / ±1.67% |       260.42 / ±2.35% |      0.6882 / ±10.98% |
| B3 三 JSON 候选 |   257.85 / ±2.01% |       257.92 / ±2.02% |      0.7409 / ±10.38% |
| A4 当前实现     |  32.7321 / ±4.08% |      34.2264 / ±3.47% |       0.7420 / ±8.49% |

20:22:05.03–20:23:36.15 总计 91.1567 秒，十二任务全部完成。每轮 finally 精确恢复原字节，最终
graphProjection.ts SHA256 为 C548467F6F27D8C2DE11DF6948E8110C92D14E8D2EE25EC2C006EB5DD6DB9E41，
候选为 F7D44FE78B51ECF6FB7DC8A42B781F0EFDF4E3ADD4AEC633C9D049CCE0D711BE。
行为证据和 fresh 场景的明显差异支持这三个边界整体保留现有 draft 外共享，拒绝所测候选；它与
上一批 dataValue 对照各有独立 fixture 和证据，没有互相外推。当前实现 A1/A4 有明显进程间漂移，
delta 不推断胜负或加速，也不将三个合并替换的差异归因单独某一路。候选仍含特殊字典 fallback，
不是纯 Immer 不可行的证明。OS、其他进程及 GC 未隔离，不把 prepare 总成本写成递归 self-time。

整体目标继续开放。资源输入 `remapDocumentInputs` 内部 remap 通知后的 buffer 搬迁，以及
fileTextInput 的 change/flush 自身 setState 通知后的 dirty 写，仍是只读发现、未实测的候选；本批
外层 guard 不代表这些内部尾部已修复。Results null Pin 发布与运行失效后的刷新续接仍需独立核查。
未覆盖的 Rust/Application/Project/SCI 内部和 TS 模块继续逐模块审查；不把本批 benchmark 的四个
Graph 边界保留决定扩称为全项目已合规。原生桌面、Tauri/FlexLayout 交互、Julia 全链和完整 CI 未运行。

上批收尾在接续时完成：20:27:27 文档契约六项通过，包含模块映射；十六个 TS/Markdown 文件定向
格式检查仅 createFileActions.test.ts 发现格式差异，定向格式后该文件复检通过。全工作树 diff 检查
通过，基准原始文件为 57848 bytes、严格 UTF8、零 NUL，四轮十二任务对应二十四条重复数值行；
另外四个排名标题不是数值样本。主代理重验恢复后的生产 SHA256 与前文一致。

### 并行接续：刷新归属、输入注册与统计配对（2026-10-02）

本批继续四路，开始工作树已有 1104 项改动。上一批属于有生产修复、测试与基准证据的进展，
接续首先补齐其尚未完成的文档/差异检查；不重新运行无变化的十二项性能任务。

Results 的空 Pin 回执与运行失效通知各自仍有旧尾部刷新：监听器切换同 ID 项目的 epoch、重开图
帧或安装新摘要并发起查询之后，旧入口重新排入摘要读取，取消后继请求。两个纯 Application
回归各覆盖这三种实际依据替换；20:32:35 两失败、十七通过，十二个 soft 断言均来自多发一次
getGraphState 和后继返回 stale。首跑复用相同 RunId 触发了真实事件去重，修正每阶段的项目
fixture 后才取得上述完整旧红，不将去重行为误报为缺陷。

两个入口在通知前复用同一个私有只读检查，捕获原项目、图帧、摘要和执行会话；通知之后仍属
原操作才继续撤销/排入摘要查询。它只组合原有 owner 的身份，图帧检查复用 currentGraphStateRequest，
没有新 token、状态表或公共接口，未使用 session 整体引用拒绝 saving 变化。20:33:27 十九项通过；
20:34:15 原十一文件 Results、运行、卸载、编辑、发布、资源与 Service 消费者七十项通过，包含
本轮十九项，不与旧六十八项重复累加。README 同步空 Pin 和运行失效的刷新资格。

```powershell
pnpm test:ts src/features/application/results/runtime.test.ts
pnpm test:ts src/features/application/results/runtime.test.ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/results/resultLeases.test.ts src/features/application/results/graphPresentation.test.ts src/features/application/results/pinResultSearch.test.ts src/features/application/editor/observeGraphRunEvent.test.ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/core/resource/resourceStore.test.ts src/services/result/resultService.test.ts
```

资源分路将上批内部候选变成实际证据。原 remap 在通知后按届时的 activeKey 写/删注册表，新项目
或同项目重建 source/target registration 时，旧尾部会覆盖或删除后继；新项目还继续安装旧快照、
提交旧 publication。现在固定原项目的 source/target key，先在同一注册表中转移捕获的条目，再通知
仍属于原 target Map 和原 input 的成员。项目失效抛现有 ProjectLifecycleError，自然停止两个外层
调用方的旧发布；同项目注册替换只跳过失去归属的成员，不把合法已提交 rename 改判失败。

fileTextInput 的 change/flush 只在既有 setState 通知后增加原 isCurrent 检查；释放或项目换代后，
旧输入不再标记后继 dirty。这次没有新建 Zustand owner，也没有修改 createFileActions 生产逻辑。
两个纯回归分别覆盖真实 rename 的项目/注册替换，以及 change/flush 的项目/释放替换；20:33:09
两失败、十一通过，十一 soft 失败断言。早期试跑错误地假设 Mind 安装保留传入根，已改为捕获实际
安装根后重跑旧实现；不计这些 fixture 断言为生产失败。20:34:23 十三项通过，20:36:12 十一文件
六十二项 affected L2 通过，包含上述十三项，不重复累计；与 Results L2 的公共发布消费者也不相加。
五文件定向格式和 diff 检查通过，Resource README 更新。主代理全文复核两个生产 owner 的发布
顺序；真实 release 仅设置 released，没有将接口上可想象的任意重入当作另一个已证实缺陷。

```powershell
pnpm test:ts src/features/application/resource/createFileActions.test.ts src/features/application/resource/fileTextInput.test.ts
pnpm test:ts src/features/application/resource/createFileActions.test.ts src/features/application/resource/fileTextInput.test.ts src/features/application/resource/docActions.test.ts src/features/application/resource/mindActions.test.ts src/features/application/resource/resourceActions.test.ts src/features/application/resource/fileManagement.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editor/workbenchPanelClose.test.ts src/features/application/editor/confirmDirtyEditorClose.test.ts
```

共享分路清点全生产调用：shareProjection 只有已取得独立证据的四个 Graph JSON 调用；测试参考
与特殊键 fallback 单独排除，不将其计为未核销生产边界。全文核对十一个 createReadProjection
构建器：Core 的 graph/read、resource/read、database/read、chart/read、execution/read、editor/ui、
sidebarDrag/ui、settings/read，Workbench internal/state/ui，以及 Results resultProjection 和
graphPresentationRead。它们复用既有业务 owner，未发现另一套可写模型或通用递归 COW。

另外全文读取十一份相关基础实现：Core state/readProjection、applicationStore，shared/types/deepReadonly，
Resource resourceDocumentProjection/resourceSnapshotProjection，DataStore graphMeta/valueTypeProjection/
graphResultProjection，Database databaseProjection，Settings settingsStore，Service graphEditorChanges。
DTO 有限字段共享和既有 Immer 更新不被按名称误判为通用不可变引擎。自有只读 port 还全文核对
Application log/logBuffer 与 Workbench layout 的 workbenchRead/workbenchLayoutProjection/
workbenchLayoutInternal，以及 useLiveLogs/useStatusBarItems 两个消费者；以上为二十八份不同生产文件。

Workbench hydration 的同步发布尾部经真实调用链核对，不构造无消费者回归：beginHydration 的唯一
生产调用 controller.bind 在返回后立即重验 bindingGeneration/bound；completeHydration 之后的
finishCycle 仍重验 currentCycle。底层 read.whenHydrated 没有生产调用，关闭时的 flush 等待 controller
自身 cycle.promise。当前证据不支持增设第二套防护或测试，也未启动新 benchmark。

进一步按最小接口规则清理此处既有冗余：底层 read.whenHydrated 无公开独立导出、实际或动态消费者，
其 hydrationWaiters 已被 controller 的恢复周期取代。删除底层等待表、读方法及两处结算，并从两个
read contract 和 projection Omit 中删除该成员；仍使用的 hydrated gate、epoch、FIFO 与 idleWaiters
保留，controller 的 cycle.promise 继续处理恢复成功/失败以及关闭等待。Workbench README 明确这个
归属，没有添加测试来维持失效 API，也没有改变 controller 或真实消费者。
20:44:40 既有通知、恢复、浮窗、激活、关闭 flush、保存和项目清理八文件四十三项通过；四 TS 和
Workbench README 的定向格式及 diff 检查通过。主代理全仓精确符号复检只剩 controller 的声明、
实现和原测试调用。该组与前述资源/Results 检查有重叠消费者，分别报告范围，不相加。

```powershell
pnpm test:ts src/modules/workbench/internal/layout/workbenchNotifications.test.ts src/modules/workbench/internal/application/workbenchLayoutController.test.ts src/modules/workbench/internal/layout/workbenchFloatingLayout.test.ts src/modules/workbench/internal/layout/workbenchActivation.test.ts src/features/application/editor/workbenchPanelClose.test.ts src/features/application/editor/useWorkbenchWindowCloseGuard.test.tsx src/features/application/editor/useProjectOperations.saveActiveFile.test.tsx src/features/application/project/projectWorkbenchLifecycle.test.ts
```

Rust 分路新增全文覆盖 Kernel statistics 的 anova、association、descriptive、classical、spatial 五个
生产文件，以及 yss-sci-runtime/src/hypothesis.rs；Catalog 同名声明、注册表及指纹消费者只补读相关
区段。配对 t 与 McNemar 原先分别读取 before/after，绕过已有读取入口的共同关系行域检查：即使
长度相同，两个独立关系也不能代表同一批观测。两处改为一次 columns 调用，按原顺序移动返回列，
没有再复制数值向量；等长物化列表仍按声明的位置配对，独立样本 t 的分别读取保持不变。

原 Application ANOVA 关系用例重命名为 paired_statistics_reject_independent_relation_domains_and_mixed_materialized_columns
并扩写，保留全部原断言，未新增测试函数。旧实现实际零通过、一失败、六十一 filtered：先验证
同域及物化正例，再累计非法成功，确实运行到 t.paired/unrelated、t.paired/mixed、mcnemar/unrelated、
mcnemar/mixed 四分支。首次 fixture 的 KernelParameterKey FromStr 编译错误已纠正，不当作行为旧红。
修复后同例一通过，测试本身 0.03s；旧红构建 24.10s、绿构建 26.91s，不把构建耗时计作统计耗时。

仅两个配对内核 revision 从 3 增至 4，进入原 KernelRegistry::freeze 指纹，图计划、包准备与 dispatch
继续沿原能力版本检查，不增迁移或第二份缓存身份。ANOVA 与 Spatial 六分支 ScientificComputationError
转换也分别与现有 common::computation_error 完全同义，删除重复 mapper；没有错误语义变化。
Kernel README 更新配对准入、revision 和公共错误映射归属。另一代理及主代理只读复核配对顺序、
关系检查与保留的独立样本行为通过；另一代理逐项核对 mapper 及扩写测试通过。

最终七项不同 Rust 用例通过：Kernel ANOVA 两项、Spatial 两项，Application 配对关系一项、十个
Spatial 节点消费一项、能力版本一项。原 ANOVA wide labels、重复测量、参数/shape 与 workspace
控制测试未改，本轮两项实际执行；没有新增或宣称 two-way 专项验证。Clippy exit 0，Kernel 无新增
警告，依赖 SCI 仍六条既有警告。五个 Rust 文件定向 rustfmt/check、Kernel README 格式/check 和
六文件 diff 检查通过。

```powershell
pnpm test:rs:package -p yss-application --test numeric_execution paired_statistics_reject_independent_relation_domains_and_mixed_materialized_columns
pnpm test:rs:package -p yss-node-kernel --lib anova_
pnpm test:rs:package -p yss-node-kernel --lib spatial_
pnpm test:rs:package -p yss-application --test numeric_execution spatial_category_all_ten_nodes_execute_with_typed_weights_and_aligned_tables
pnpm test:rs:package -p yss-application --lib numeric_extension_uses_actual_capabilities_and_rejects_old_artifacts
pnpm lint:rs:package -p yss-node-kernel --all-targets
```

有语义差异的实现保留：ANOVA 的数值预留与 common numeric 不同，Association 多列共享水平编码
与 common 单列编码不同；Association/Descriptive 对参数错误的映射也不同，不机械用公共 mapper
替换。独立秩样本当前等长/同域限制在节点 help 已明确记录，本批不按方法名称擅自扩契约。
经典检验 Runtime 四入口没有 ScientificExecutionControl 参数；Kernel 现有物化/序列化前后检查
不能证明 SCI 内部协作取消或峰值工作区完整受控，这个 owner 的内部边界仍待核查，没有修改 SCI。
原生桌面、Tauri/FlexLayout 交互、Julia 全链和完整 CI 仍未运行，整体目标继续开放。

本批交叉复核已闭合：Graph 分路确认 Results 两处通知检查及正常 saving 变化的接纳；Database
分路确认资源注册转移和 released/epoch 检查；Plugin 分路确认配对行域、列顺序、版本和等价 mapper。
20:45:58 最终 TypeScript、lint、文档契约六项、十个 TS/五份 Markdown 定向格式及全工作树 diff
检查通过；TS 仍三条既有 lint 警告，Rust 复用本批定稿后的定向格式和 Clippy 结果。不重复运行未变
的性能任务，不将这些受影响范围检查称为全 CI 或全模块完成。

```powershell
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts
git -c core.safecrlf=false diff --check
```

### 并行接续：组合边界、目录扫描与经典检验控制（2026-10-02）

本批开始已有 1110 项工作树改动，继续主代理加三分路。前端分路完成后转为 Rust 接口和独立复核，
算法文件按归属拆开，Cargo 由同一分路统一执行；不并行编辑同一 owner。上一批属于已取得修复和
受影响范围验证的进展，未变化的基准与用例结果继续复用。

主代理全文读取 App 的十七个生产文件：App、main、i18n/index，SettingsEffectsProvider、
ChartThemeProvider、UIHost，WorkbenchComposition、editorRendererRegistry、rootPanelRegistry、
rootPanelTabRenderer、menuContributionRegistry、statusBarContributionRegistry，以及 integrations 的
workbenchCommandCoordinator、PluginProvider、panelActivationCoordinator、activityEditorDndOverlay、
activityEditorDndCoordinator。另一代理独立全文复核同组文件，不把重复读取累加为覆盖量。
两份 locale payload 只检查结构和声明，没有全文读取或新增本地化专项测试。

组合层经 public UI 与 Application 入口组织行为；Plugin Context 传稳定 registry，实际投影经
Zustand selector 读取；主题 Context 传低频 memo 配置；UIHost 分别订阅弹窗和进度。拖拽 ref
持有监听器清理，完成前先撤销旧 ref，真实 drop 在 finish 后及 activate 等待后重验原项目。
菜单 close 的实际唯一消费者和 Application 关闭链未证实后继污染；不为可想象的未来重入增加
另一套代际。这个结论限定于已核对的当前调用链，不是任意监听器重入安全性的证明。

Domain 分路全文覆盖当前十四个生产文件：Resource 的 resourceTypes/resourceQueries，NodeCatalog
的 catalogItem/creationDescriptor/localizedCatalogTree/searchDocument，logDomains、nodeDiagnostics、
graphConstants/valueInput，EditorProjection 的 index/graphRuntimeTypes/displayLabels，
databaseEditor/gridSelection，以及 result/inspectableResultRef。未见 Domain 反向引用 UI、Services
或 Tauri；派生目录、搜索和诊断数据没有成为第二份可写 authority。

另全文读取十四个直接衔接文件：Core resource/resourceSelectors，NodeCatalog 的 nodeCatalogStore、
localizedSearchIndex、resourceCatalogIndex，execution/pinViewTarget；Application NodeCatalog 的
catalogTreeBrowser/createNodeFromDescriptor/useLocalizedNodeCatalog，sidebar/buildSidebarDragData，
Results inspectableResult/pinResultSearch，databaseEditor/useSelection，editor/resolveExecutionGraphPath；
Database Editor 的 useDatabaseGridSelectionAdapter。其他 UI 消费者只核对真实调用区段，诊断模板
JSON 只核对导入和生成归属，不扩称全文覆盖。

catalogTreeBrowser 在搜索时对同一棵树收集分类 ID 两遍，现仅收集一次，并将同一 ReadonlySet 用作
分类集合和搜索展开集合。NodePalette 只读取、遍历和 has，修改时另建 Set；未引入可变集合共享。
20:52:11 既有四文件五十项通过，未新增或修改测试，也未把减少一次遍历称为实测加速。

```powershell
pnpm test:ts src/features/domain/nodeCatalog/localizedCatalogTree.test.ts src/features/domain/nodeCatalog/searchDocument.test.ts src/features/core/nodeCatalog/localizedSearchIndex.test.ts src/modules/graph-editor/internal/ui/NodePalette.test.tsx
```

外围 Services 分路全文读取十二个生产文件：Platform 的 settingsEvents、pathDialog、opener、clipboard、
appWindow、platformTypes、referencePdf、webviewWindow、mainWindowGeometry，Workbench 的
presentationService/activityPanelService，以及 devHmrIpc。当前窗口 scaleFactor 包装、接口成员和
readWindowScaleFactor operation 没有实际或动态调用方，删除三处及既有窗口 mock 的对应方法。
保留仍有契约意义的 failure 类别，不按当前生产者数量机械缩减错误模型。20:53:11 既有七文件
十八项通过，没有新增测试；真实桌面窗口交互未运行。

```powershell
pnpm test:ts src/features/application/window/useCurrentWindowActions.test.tsx src/features/application/editor/useWorkbenchWindowCloseGuard.test.tsx src/features/application/window/createPersistedWindow.test.ts src/services/workbench/presentationService.test.ts src/services/workbench/activityPanelService.test.ts src/services/devHmrIpc.test.ts src/services/platform/referencePdf.test.ts
```

两项前端精简共四文件定向格式检查及 TypeScript 检查通过，lint exit 0，仍三条既有 warning。
其中 projectEventStream 的监听器展开是通知快照，不仅按 lint 提示删除。两组定向用例保持分别
报告范围；没有将旧 UI 测试的执行解释为新增 UI 测试或原生交互验收。

Rust 接续全文核对 SCI hypothesis 的 sample_mean、categorical、nonparametric、variance，Contract
execution/hypothesis 及 Runtime hypothesis。确认原四入口没有执行控制，Kernel 外层检查无法中断
内部精确枚举；分类表还在按实际 R×C 分配前缺少工作区准入。阶段一新纯 Kernel 回归只用九十六
级输入，实际矩阵约 72 KiB，不使用可能耗尽内存的 GB fixture。旧实现零通过、一失败、四十九
filtered，首个 BudgetExceeded 断言失败；后续正例未运行，未记录实际返回值，因此不称其显式成功。
修复后一通过、四十九 filtered：96×96/32 KiB 拒绝，256 KiB 成功，九十六行但两类/32 KiB 成功。
旧构建 4.72s、新构建 5.11s，两次测试本身均 0.01s；构建耗时不算统计计算性能。

```powershell
pnpm test:rs:package -p yss-node-kernel --lib classical_count_tables_admit_their_actual_workspace_before_computation
```

阶段二继续贯通 typed error、四算法内部 checkpoint、其他线性工作区估算及受影响消费者；最终结果见本批末。
预算继续归现有 Kernel owner，ScientificExecutionControl 继续只拥有协作取消及 deadline；不新增
预算上下文、固定行数上限或并行状态模型。本段阶段一证据不代替下述阶段二的独立验证。

主代理另外全文核对 Canonical Hash、Display Naming、Project Progress、Project Layout 四个基础
crate 的 manifest 与全部生产源码；Math Expr 的独立只读复核继续并行。这些小型模块分别拥有
字节摘要、显示名称分配、任务取消与进度 port、磁盘布局常量及索引输入分类，没有反向业务依赖。
Progress 通过同一 registry 与 Arc 身份识别任务，旧任务结束不清除后继；Application 的 PickerTask
守卫拥有完成/放弃时的结束，Project Registry 消费同一个 cancellation，IPC 只适配输出 port。
Layout 的实际消费者使用 Filesystem 已验证的 RelativePath，不把分类函数当作文件访问授权。
这里只全文补读了 Project file_changes，Application registry 只读取生产部分和测试入口，其他
消费者限定在实际调用区段；不扩称 Project/Registry 整体内部已复核。

hash_canonical 原先序列化得到 JSON Vec 后，又复制 prefix、domain 和整个 payload 到第二个 Vec。
现在复用原 Sha256 的 update 接口依次输入同一 u64 大端域长度、域字节及 JSON 字节，再 finalize。
JSON 编码、字段顺序、错误和公开签名保持原契约；没有新增摘要模型、兼容路径或 writer 抽象。
这是删除可避免的整份缓冲复制，没有申请默认规则例外，也未宣称已测量的速度或 RSS 改善。

既有三项 Hash 用例通过，包含固定摘要、域隔离与未改的流读取错误语义；Graph Document 的有效
连线顺序、Node Registry 的呈现字段排除/参数约束身份各一项通过，两文件实现与原测试已全文读取。
共五项不同用例，没有新增测试或用编译失败充当旧红。Hash 的 all-targets Clippy 及 package 格式
检查通过；消费者已实际执行，不只编译改动 crate。

```powershell
pnpm test:rs:package -p yss-canonical-hash --lib
pnpm test:rs:package -p yss-graph-document -p yss-node-registry --lib semantic_identity
pnpm lint:rs:package -p yss-canonical-hash --all-targets
pnpm format:rs:package -p yss-canonical-hash -- -- --check
```

前端另全文检查 Core 的 statusBar 四文件、theme 两文件、keyboard 三文件，以及 Application 的
两个 statusBar 文件；后者 useStatusBarItems 是之前已读文件，不当作新增全局覆盖量。状态栏只订阅
当前活动图的计数和选择，实时坐标由所属 span 的订阅及 requestAnimationFrame 更新，清理取消
未执行帧；主题 selector 返回既有稳定预设，不复制整个设置。Pin 颜色键实际来自已验证 ValueType
的固定映射，未把可想象的任意字符串输入误报为当前 IPC 缺陷。补读 pinVisual/pinSemantics 及两个
Workbench StatusBar 呈现文件，未新增代码或测试。

Math Expr 分路全文核对 manifest 和三个源文件（lib、ast、adapter），并全文读取 Bayes expression
及十项原测试；SCI linear_hypothesis、regression/models/nonlinear 与 Julia extension 命令只核对
真实解析和解释区段。Math 拥有语法、中立 AST 与解析预算，SCI 拥有参数映射/线性化/数值求值，
Bayes 拥有分布参数个数及领域转换，没有将科学语义搬进公共 parser。

发现外部可达的 LaTeX 深度漏计：operatorname 的参数递归回解析入口时没有携带深度，随后 convert
从一重新计数。仅三十三层 AST、低于节点和输入字节上限的关系因而返回成功。只在原 adapter
贯通私有 depth，公共入口和每侧关系从一开始，函数参数加一，convert 复用传入值；相同 DepthLimit
检查由原 owner 内的小函数复用，没有第二次 AST 校验或新的预算模型。同文件两份完全相同的
函数白名单也改为复用已有 is_allowed_call，保留 q_9 placeholder 处理与原函数名范围。

新增一个纯解析回归，独立于 Kernel 内存和协作取消回归：普通叶子与常规 exp(x) 混合叶子分别验证
总 AST 深度三十二成功、三十三拒绝。实际旧红零通过、一失败、十四 filtered，超限关系错误返回
Ok；修复后一通过、十四 filtered。完整 Math 库十五项及 Bayes 直接 expression 消费者十项通过，
后者十项 filtered。仅修改 adapter.rs，无新文件、依赖或公开 API；公共既存限制未变，因此不为修补
漏执行新增 README 契约。包定向格式和 scoped diff 检查通过。SCI 两个真实消费者由同一 Cargo
执行分路继续定向检查，不重复运行全 SCI。

```powershell
pnpm test:rs:package -p yss-math-expr --lib latex_operator_calls_share_the_expression_depth_limit
pnpm test:rs:package -p yss-math-expr --lib
pnpm test:rs:package -p yss-bayes-model --lib expression::tests
pnpm format:rs:package -p yss-math-expr
```

经典检验阶段二已完成。Contract 的 HypothesisError 新增透明 Execution 分支；四个 Runtime
入口接收并透传同一 ScientificExecutionControl，直接返回原 typed error。SCI 四个算法文件复用
hypothesis 内唯一 checkpoint，在观测扫描、rank/tie 扫描及回填、精确检验枚举和 Mann-Kendall
观测循环中检查取消/deadline；排序、分布库 CDF/SF 及相邻调用间也检查。私有 Fenwick 单次迭代
有对数界，执行控制放在逐观测入口。没有将外层 run 的首尾检查冒充内部协作取消。

原来唯一绕过 Runtime 的 psychometrics/items 继续复用已有 control，只拆回 Execution 原错误，
其余输入/计算失败仍按原 failed 映射，样本不足和零方差的 None 条件保持。该文件是本批开始前
已有的 untracked 用户内容，只改必要调用与错误转换，没有重排心理测量算法。Kernel 复用原
computation_error 映射，输入和科学失败仍为 InvalidNumericInput；classic 调度共用原预算 owner。
四份 canonical README 同步这些边界。

Kernel 数值检验在 SCI 工作区分配前合计输入、排序/rank/tie、方差偏差、重复条件行与结构化输出；
分类列编码删除完整 List scalar clone，第二列读取计入已保留第一列，lazy callback 优先返回捕获的
原 KernelError，不再把预算/取消/超时降成普通 Relation InvalidInput。分类索引在分配前准入，
密集计数表按实际 R×C 计算并检查整数溢出，不以 N² 误拒少类别输入。

独立审查进一步发现 Arrow 物理内存倍数不能代表 Dictionary/Utf8View 展开后的整列文本。最终
仅对这两种间接编码逐行 slice，并沿用原 materialized_values；切片前、转换前合计原数组物理
内存、单 scalar 暂存、前列及当前编码列，新增编码格式化前再次准入。普通 dtype 保留原批量路径，
未扩大通用 Arrow/Relation 公共契约。私有批适配器仅向 statistics 父模块可见，以供原兄弟测试访问。

原预算回归扩写真实 DictionaryArray：两条 512 字节标签被引用 256 次，32 KiB 拒绝且尚未编码完，
256 KiB 成功并核对原 s: 编码。它直接调用同一生产批适配器，不是完整 Relation 端到端测试；
没有单列 Utf8View fixture 或分配器/RSS 测量。源码中逐行转换的结构是避免整批展开的依据，不把
一个 BudgetExceeded 结果单独当作峰值内存证明。独立代理最终新鲜复核该边界和错误保真通过。

新增第二个纯 Rust 回归直接调用 Wilcoxon 的私有精确枚举 helper：正常小样本 p 值为 0.5，已取消
及已到期控制分别返回原 Cancelled/DeadlineExceeded。它不经过 run 的首尾检查，实际观察内部
checkpoint；没有用新签名导致的编译错误充当旧行为红灯，也没有用脆弱耗时阈值宣称实时延迟。
既有 Kernel scale 用例扩写 t.one_sample 与 Wilcoxon 的统计量/p 值，保留 Bartlett 十七组和 Kappa
三百类原断言。经典内核 revision 推进到 4，先前已修订的配对 t/McNemar 推进到 5，继续通过原
冻结注册表指纹使旧能力产物失效，没有兼容版本或新缓存身份。

最终二十一项不同 Rust 用例通过：Kernel classical 两项（四十八 filtered），SCI hypothesis 十三项
（九十二 filtered），nonlinear 直接 Math 消费者一项（一百零四 filtered），psychometrics_category
三项（零 filtered），Application 配对关系一项（六十一 filtered），Application 能力产物一项
（一百四十八 filtered）。前述私有枚举和普通结果断言包含在这些用例内，不重复累计。Math 的
十五项、Bayes expression 十项及 Hash/指纹五项分别保留各自范围。

最终 Kernel 构建 5.18s、测试 0.06s；SCI hypothesis 构建 13.39s、测试不足 0.01s；nonlinear
构建 0.31s、测试 0.02s；psychometrics 构建 13.88s、测试不足 0.01s。Application 两个目标的
构建分别 1m21s 和 56.31s，测试分别 0.03s 和 0.20s。这些是构建/测试耗时记录，不是算法基准。
没有运行完整 SCI 库或完整 workspace CI；新公共接口的实际受影响消费者已独立选择并执行。

```powershell
pnpm test:rs:package -p yss-node-kernel --lib classical_
pnpm test:rs:package -p yss-sci --lib hypothesis::
pnpm test:rs:package -p yss-sci --lib regression::models::tests::nonlinear_formulas_curves_deming_and_splines_fit_actual_models
pnpm test:rs:package -p yss-sci --test psychometrics_category
pnpm test:rs:package -p yss-application --test numeric_execution paired_statistics_reject_independent_relation_domains_and_mixed_materialized_columns
pnpm test:rs:package -p yss-application --lib numeric_extension_uses_actual_capabilities_and_rejects_old_artifacts
```

本批预算是保守工作区估算，不是进程 RSS 硬界；控制是协作检查，单次转换、排序与分布库调用
内部不能硬抢占，未测实时取消延迟。只核对本次数值逻辑未误改，不宣称全部经典统计公式已有
独立参考验证。另记录未证实的旧候选：Mood median 和 multiple proportions 的计数表构造顺序与
chi_table 的行主序消费需独立核对，本批没有改它们的公式或把静态怀疑写成已修复。

五包 all-targets Clippy exit 0，耗时 11.90s；当前 SCI 有七条 warning，分别位于生产 categorical
两处、nonparametric/sample_mean 各一处，以及 anova/tests、association/tests、time_series_category
各一处，后面三处来自本次较宽 target 范围。其他选中包无 warning。十一份本批 Rust 文件定向
rustfmt（edition 2024、skip_children=true）及相同范围 --check 通过，保留其他 SCI/Meta 等文件；
Math 与 Hash 包另有各自 package 格式检查，不用它们代替这十一份子模块的检查。

```powershell
pnpm lint:rs:package -p yss-node-kernel -p yss-sci -p yss-sci-runtime -p yss-sci-contract -p yss-math-expr --all-targets
pnpm format:rs:package -p yss-math-expr -- -- --check
```

四份 README 初次定向 Oxfmt 检查中 Kernel/SCI 两份有格式差异，已仅格式化这两份文件。
整体目标仍开放：本轮没有完成全项目逐模块复核、原生桌面/Tauri/FlexLayout 人工交互或 Julia
完整计算链，也没有运行全 CI。未变化的已核销边界和基准继续复用，后续优先补清单中尚未读完
的基础 crate、Rust 内部编排及前端剩余 owner，避免将相同只读复核反复当作新增覆盖。

21:35:48 文档契约六项通过（含模块映射），四份 TS 与四份模块 README 定向格式通过；主报告
因覆盖表更新产生格式差异，已单独格式化并复检通过。全工作树 diff 检查通过。TypeScript 和前端
lint 复用本批四份 TS 定稿后的通过结果，随后没有再修改前端源码，仍三条原警告；没有新增 UI
测试、暂存或提交，也未清理无关用户改动。

```powershell
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts
git -c core.safecrlf=false diff --check
```

### 并行接续：资源契约、数值所有权与基础呈现（2026-10-02）

继续使用主代理和三个子代理，各自负责前端、Graph、Database/Linalg、Harness；文件写入按
owner 划分，Cargo 依次交接。只读审查并行，受影响消费者和局部格式检查跟随实际变更；不把
未变化模块的历史测试重新累计为本轮通过，也不以测试数量证明全项目合规。

**基础呈现与主题。** 本批全文核对 `components/ui` 二十三个、`components/ui-presentation`
十四个、`shared/theme` 五个生产文件，以及 `components/data-grid/agGridTheme.ts` 和
`lib/utils.ts`，共四十四个；这是本批读取范围，并非声称四十四个都从未审查过。另完整阅读
当时的报告页面容器、StructuredResult、WindowTitleBar 和 globalEvent，按需追踪线性报告、表格和
页面安装消费者。既有 shared/charts、Application 页面状态及 Service 审查不重复累计。

基础控件继续透传 Radix/DOM 能力。Tooltip Context 只传稳定操作接口，拖动标记和活动提示归 ref，
清理检查原登记身份；Dialog Context 仅携带低频层级，焦点仍交原组件恢复。ToggleGroup 传样式配置，
唯一实际调用方使用默认水平方向，没有高频完整模型广播。没有为了统一 Zustand 镜像这些框架和
局部展示状态。`cn` 只组合现有 clsx/tailwind-merge，没有另造样式引擎。

StructuredData 的分页和折叠是组件局部展示状态，
引用读取由调用方注入；真实 StructuredResult 继续通过 Results 的一百行分页，未新增 IPC owner。
系数图表与公式消费原系数页，格式化和坐标缩放不重新计算统计模型。主题预设冻结且复用，AG Grid
两个真实消费者均按主题引用 memoize 构造；没有证据要求新增缓存、store 或 benchmark 例外。
本范围无源码修改、无新增 UI 测试；焦点、窗口拖动、分页与真实绘制仍未作原生桌面验收。

**Graph 契约与无效扫描。** 全文核对 Function Editor Projection、Graph Type Mapping、
Graph Analysis Contract、Graph Resource Contract 的四份 manifest 和九个生产 Rust 文件，追踪
Project signature、Application 捕获、Analysis/Runtime 缓存、Editor/IPC 投影及 Execution 准备。
声明类型仍复用原 parser；节点语义仍只有 GraphSemanticSnapshot 一个权威。

ResourceCatalogSnapshot 的全目录 fingerprint 只被自身测试读取，生产 Application 却每次对全部
函数与数据库列执行四路哈希。真实解析缓存和执行继续使用各自的实际依赖指纹，包括 absent lookup
及缓存命中后的依赖回放。现删除无消费者的 fingerprint 类型、字段、getter、构造参数和该计算，
连同只服务此哈希的 ProjectGraphResourceSnapshot 两个私有身份字段。Application 捕获及交付的
ProjectAuthorityBasis/session 重验保留，未修改真实缓存键或执行身份。

同批删除只有自身测试使用的字符串类型映射包装与专属错误，保留 typed ValueType 映射；删除没有
Rust/TS/JSON/bench/example 消费者的 NodeDiagnostic 泛型 DTO，现有 GraphDiagnosticFact →
EditorDiagnosticModel → IPC 路径和基础诊断值类型保留。另一个代理独立核对了删除路径的身份和
序列化影响。没有新增兼容层、依赖或用来证明旧符号消失的测试。

本批 Graph 净变化为七个生产文件、二十五个仅 fixture 变化文件和 Runtime README；一些文件
原本已有其他修改或未跟踪，不能用整体 git diff 或 dirty 数量当作本批净变化。构造 fixture 只去掉
废弃参数；公开内部构造 API 的真实消费者均已编译。

```powershell
pnpm test:rs:package -p yss-graph-resource-contract -p yss-graph-type-mapping --lib
pnpm test:rs:package -p yss-graph-analysis -p yss-graph-runtime -p yss-graph-editor -p yss-graph-execution --lib
pnpm test:rs:package -p yss-application --lib graph::inputs::tests
pnpm test:rs:package -p yss-application --lib graph::catalog::tests
pnpm test:rs:package -p yss-application --test numeric_execution decompose_returns_lazy_typed_columns_before_the_data_file_exists
```

以上依次通过 5、104、1、14、1 项，共一百二十五项不同测试。Graph Runtime 原有一个 manual
timing 用例仍 ignored，未执行。后两条在删除 NodeDiagnostic 后重新编译依赖及 Application；
此前 Graph 行为测试对应已删 catalog fingerprint/name wrapper、尚未删该无人使用 DTO 的源码，
没有将它们冒称为后置变更后的重复运行。numeric_execution 编译全部子 fixture，实际只运行所列
一项。Graph 四包构建 20.03s、测试合 2.47s；Application 三条构建分别 67s、36.55s、52.96s，
这些仅为验证耗时，不是性能 benchmark。

**Dataset Profile 与 Linalg。** 新增全文核对 Dataset Profile 的 lib、column_distribution、
column_stats、dataset_overview 四个文件，SCI Linalg 的 lib、error、dense、backend 四个文件，
以及两包 manifest 和 Linalg 契约/既有测试。Database Contract 九个、Source 七个文件已有完整
覆盖，本轮直接复用。Engine profile 与 Application database/query 全文及 Runtime、IPC、Harness
真实消费者显示 Profile 只承载统计 DTO；计算、固定查询和项目/Schema 重验仍归原 owner。

Linalg 继续独占 faer 依赖，借用 view 保留逻辑 stride，shape/RHS 的程序员前置条件与 README
一致。删除 Lu 私有 size 副本，直接读取既有 U 的维度。真实 GLS 消费者原先将 lower() 已返回的
拥有矩阵再次 to_owned，随后又复制 betas 和置信区间输入；现直接使用拥有值、借用系数和标准误，
最后移动结果。每次成功 fit 少六次显式复制，计算顺序、公式和错误边界保持。没有宣称已测加速比。
保留 lower() 的拥有 API：其他调用方会从临时分解对象取出矩阵并继续持有；strided view 的
to_owned 与本来需要可写工作区的副本同样保留。

```powershell
pnpm test:rs:package -p yss-sci-linalg --test contracts
pnpm test:rs:package -p yss-sci --test weighted_statistics
pnpm test:rs:package -p yss-sci --test rank_failures
pnpm test:rs:package -p yss-sci --test spatial_category spatial_all_seven_models_match_scipy_estimates_information_predictions_and_impacts
pnpm lint:rs:package -p yss-sci-linalg -p yss-sci -p yss-graph-resource-contract -p yss-graph-type-mapping -p yss-graph-analysis-contract --lib
```

数值相关依次 5、3、1、1 项通过，共十项不同既有测试；spatial 另四项 filtered。没有新增同构测试
或制造重构前失败。两份源码局部格式和 diff 检查通过，公开契约未变，不扩大 lower API 或修改
无变化 README。五包 lib Clippy exit 0，7.73s，仅 SCI 四条既有生产警告；Linalg 与三个 Graph
叶子包无警告。已对照 HEAD 确认四条旧警告，其中 sample_mean 旧函数已有八个参数，现九个，
不是本轮首次越过阈值。未执行全部 SCI、跨平台数值验证或全 CI。

**Harness Rig。** 全文核对 manifest 和八个生产文件：lib、error、driver、arguments、messages、
provider、stream、tools/mod；此前明确全文记录 stream/driver，本轮补齐其余六个及 manifest。
Provider 锁内捕获 driver、锁外 await；配置替换只影响后续准入。活动工具调用及结果按 ID/名称
严格配对，实时和重放复用结果编码；参数由 Serde 决定接受，schema 遍历仅生成有界诊断。
取消/期限/致命失败停止模型流，已登记任务在返回前完成收尾；真实 Core manager/worker 等待
driver，worker 取消后仍等待原工作。未发现需新增 ledger、框架或修复的实证问题，无文件变化、
无 Cargo 和新增测试；历史 Rig 二十一项不能计为本轮新通过。远端模型和网络故障未作真实验收。

**概览查询的中断分类。** 上述 Profile 消费者复查发现 Application automation 原先用同一个
`map_err` 将查询的所有 DatabaseError 改成 DatabaseUnavailable。Runtime 已通过真实的
RelationError → DatasetStoreError → DatabaseError 链保留取消/期限分类；提前 `?` 会绕过
Application 后置 control.check。现由该用例的私有 mapper 保留 Cancelled/DeadlineElapsed，
其他错误仍使用原 DatabaseUnavailable 和 databaseId，不新增通用错误框架或重复输入校验。
更新 Database README 的当前用例契约，其他既有改动保留。

新增一个非 UI 回归，以真实错误类型包装分别输入取消、期限和 QueryFailed，断言生产 mapper 的
分类、details 及 wire。先仅提取保持旧行为的 mapper：实际 0 pass、1 fail、149 filtered，首个
Cancelled 断言得到 DatabaseUnavailable；随后才改生产分支并取得 1 pass。该旧红没有执行到
后两项断言，不能宣称三分支都在旧版独立失败；新版本实际覆盖全部三项。测试是类型桥验证，
不是完整查询或计时竞态端到端复现。

```powershell
pnpm test:rs:package -p yss-application --lib dataset_profile_preserves_typed_query_interruptions
pnpm test:rs:package -p yss-application --lib command_harness::gateway::tests
pnpm test:rs:package -p yss-harness-core --lib tool_failures_close_the_ledger_and_emit_the_identified_terminal_event
pnpm lint:rs:package -p yss-application --lib
```

最终新类型桥一项、既有 Gateway 真实 profile/取消/timeout/panic 两项、Core 工具失败终态一项，
共四项不同测试通过。Core 在 Inspect 的返回末尾、ledger.finish 之前仍会重新检查控制和 wallclock
期限，因此本问题限定为 Application/Gateway 的错误分类丢失，不能声称已证明模型续跑或账本
异常。Database 代理独立只读核对真实错误链、测试边界及这项兜底。

Application lib Clippy exit 0，23.38s，同时覆盖本批 Graph inputs/catalog 生产变化；Application
和 Graph 无新 warning，依赖 SCI 仍为上述四条。automation 文件 rustfmt、Database README 的
Oxfmt 和两文件 diff 检查通过。本批只有这一项新增测试，没有新增文件或依赖。

Graph 全部三十二个本批 Rust 路径已显式执行 `rustfmt --edition 2024 --config skip_children=true
--check`，exit 0；这次是实际检查结果，没有以先前格式化命令冒充。Graph 代理另独立核对
Linalg/GLS 的 shape、LU 原主元约束、拥有边界和乘法后加减的次序，未发现实质问题，没有重复
运行数值测试。各代理均已释放 Cargo；本批没有并行争抢构建锁。

本批 Rust 实际通过一百三十九项不同测试（Graph 125、数值 10、Application/Harness 4），未执行
完整 Application/SCI、全仓 CI、原生桌面交互或 Julia 完整链。前端本批无源码变更，复用上一批
最终 TypeScript/lint 结果，不把只读审查当作新增测试通过。整体 goal 继续开放，尚未完成清单中的
其余内部编排、边界和人工验收；后续沿更新后的覆盖表推进，保留已有四个 Graph JSON 边界基准
证据，不重复审计未变化范围或重新计算其测试总数。

22:01:01 本批文档契约六项通过（含模块映射），三份变更 Markdown 的定向格式检查及全工作树
diff 检查通过。没有暂存、提交或清理无关用户改动。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src-tauri/crates/yss-graph-runtime/README.md src-tauri/crates/yss-application/src/database/README.md
git -c core.safecrlf=false diff --check
```

### 并行接续：原始路径、回复边界与基础契约（2026-10-02）

继续使用主代理和三个子代理按独立 owner 分工；只读核查并行，Cargo 按 Registry、Project、
Graph 等分路顺序交接，不同时争抢构建目录。各分路复用已读且未变化的源码与有效检查结果，
不把再次读取累计为新增覆盖。本轮起点已有 1143 项工作树变化，未暂存或提交。

**Registry 原始路径。** 全文核对 Project Registry 的 lib/discovery、Registry Contract 的 lib、
Registry SQLite 的 lib、三份 manifest 与既有测试，以及 Application project/registry 和 IPC
command_project/registry。初始化、create/saveAs/delete 与前端选择器只追踪相关调用段。
Contract 持有记录和 port；Registry 拥有发现、路径、排序与协作取消；SQLite 负责持久化；
部分提交结果与发布仍归 Application。阶段边界的协作取消不等于抢占文件系统调用或撤销已提交工作。

SQLite 原先把原生路径拼成 URL，再由 SQLx 解析；目录中的字面 `%23` 会被当成 URL 编码。
现直接使用 `SqliteConnectOptions::new().filename(&db_path)`，不再损失路径身份；WAL、同步级别、
连接数及事务语义保持。Application Project README 同步这一契约。扩展原有 round-trip 测试，
临时目录包含 `%23` 并断言文件确实位于原路径；未新增测试函数。旧实现实际 0 pass、1 fail，
SQLite code 14 无法打开文件；修复后同项通过。

```powershell
pnpm test:rs:package -p yss-project-registry-sqlite --lib sqlite_store_round_trips_updates_and_removes_canonical_records
pnpm test:rs:package -p yss-project-registry-sqlite --lib
pnpm test:rs:package -p yss-application --lib created_project_name_agrees_with_manifest_and_registry
pnpm lint:rs:package -p yss-project-registry-sqlite --all-targets
```

最终 SQLite 三项与 Application 真实创建消费者一项通过，共四项不同测试；Clippy 无警告，
两份 Rust 定向格式及三路径 diff 检查通过。没有把 Windows 本地验证扩称为跨平台验收。

**Project 文档和历史接口。** 全文核对 Model 的 lib/file/doc/mind/patch 五个生产文件、README
和 manifest，History 的 lib 与 manifest，以及 Project minds/docs/resource_patch 和 Application
file_resources/events。其他 IO、writer、Graph history 与 IPC 发布路径限定为相关段落。
Mind/Doc apply 修改的是 Project 克隆的候选；完整编辑批次成功后才安装驻留状态，普通编辑
不写文件，持久化由 Save 等显式文件命令完成。现有生命周期测试已证明先修改候选、后遇
无效父节点时原驻留文档保持，未另建事务框架或新增同构测试。

全仓核销后删除 History 中无人使用的 ResourceKind、两个 kind 方法及六个 inverse 方法，
净删 85 行；保留实际 wire DTO、serde 字段和 FunctionDocumentPatch 构造。Graph 的真实撤销
仍使用 GraphDocumentPatch，由 Project 拥有历史；没有新增项目级撤销栈。Project README
同步最小接口契约。另一个代理独立复核身份和序列化影响；同文件更早的 ChartType 改动不计本批。

```powershell
pnpm test:rs:package -p yss-project-history -p yss-project-model --lib
pnpm test:rs:package -p yss-project --lib file_resources::tests
pnpm test:rs:package -p yss-project --lib project_state::function_mutation::tests
pnpm test:rs:package -p yss-application --lib committed_resource_fact_preserves_wire_fields_and_required_identity
pnpm lint:rs:package -p yss-project-history -p yss-project-model --lib
```

依次 1+5、3、3、1 项通过，共十三项不同既有测试；Application 最后一项实际编译并消费最终
契约。两包 lib Clippy 无警告，History 定向 rustfmt 和两路径 diff 检查通过。纯未消费接口删除
没有声称存在重构前行为失败。

**IPC 和 Assistant 回复。** 新补齐 IPC Contract 全十二个生产文件及 manifest/README：lib、
editor_projection、error、graph_editing、graph、execution、harness、project、project_progress、
event/mod、event_project、event_resource。追踪真实 Application command/error、Event/Channel
与前端对应 parser；未引入第二份状态或通用校验框架，也未修改 Rust wire。

Harness TurnResult 的 finalText 是 String，成功完成允许空正文；固定版本 Rig 的 reasoning-only
既有测试及本仓 consume_text_stream、Core、Application 路径支持这一情况。前端 turn_completed
事件已接受空字符串，提交回执却复用非空标识字段 parser，导致同一成功结果进入错误分支。
现仅在 parseHarnessTurnResult 检查字符串类型，继续拒绝缺失、null、数字、布尔和对象。
新增一个纯 parser 回归同时检查两种边界；实际旧红为 1 fail、5 pass，修复后 L2 五文件十四项通过。

```powershell
pnpm test:ts src/services/assistant/harnessContract.test.ts src/services/assistant/harnessService.test.ts src/features/application/assistant/assistantHarnessSession.test.ts src/features/application/assistant/assistantHarnessProjection.test.ts src/features/application/assistant/assistantHarnessRuntime.test.tsx
```

未新增或改写 UI 测试；沿用的 runtime 用例不等于真实远端模型、原生订阅或历史记录端到端验收。

**共享呈现与窗口。** 全文核对 shared/ui 当前二十四个生产文件、Application/window 十三个、
config-default 四个、constants 两个、formatStat、appLinks 及 shared/platform/tauriWebview；
后续补读 utils 四个生产文件。window、Markdown、Flow 和日志已有部分或完整历史覆盖，不能
将这些再次阅读当成五十个新覆盖文件。窗口恢复、Results 租约、日志生成与传输、Radix 的
焦点/关闭等仍沿既有 owner；本轮未创建新的业务 store 或生命周期抽象。

Select 原先用固定字符串代表空选项，而 ChartDetailPanel 直接把合法数据库列名作为 option
value；同名字面列名会被回调误变成空字符串。现所有选项统一加一个内部前缀，并在回调移除
恰好一个字符，使任意原值与空值一一对应；无空选项时，原受控空值仍使用 Radix placeholder。
外部 API 继续读写原值，另一个代理独立核对八处生产调用，无编码值泄漏。没有新增 UI 单测；
此问题由真实输入契约及调用链确认，没有伪称已执行特殊列名的交互旧红。

appLinks 的前端显示版本改为直接读取根 package.json，About 是实际消费者；src README
同步版本 owner。未改原生发布配置，也不宣称两个发布版本已经共用同一来源。

```powershell
pnpm test:ts src/modules/details/internal/ui/panels/ChartDetailPanel.databaseIdentity.test.tsx src/modules/details/internal/ui/node/NodePinInterfacePanel.test.tsx
pnpm test:ts src/app/windows/workbench/menuContributionRegistry.test.tsx
pnpm check:ts
pnpm lint:ts
pnpm format:check:ts src/shared/ui/Select.tsx src/shared/appLinks.ts src/services/assistant/harnessContract.ts src/services/assistant/harnessContract.test.ts
```

Select 实际消费者两文件七项、菜单实际导入链一文件三项通过；后者不是 About 版本呈现断言。
加上 Harness 十四项，前端第一阶段共二十四项不同测试；Rust 第一阶段共十七项。最终四份 TS
变化后的类型检查和 lint 通过，仍为 projectEventStream、useLogPanelVirtualList.test、
useChartContainerSize.test 三条既有 warning；四文件格式通过。之后仅修改 Markdown，未重复
整套前端检查。原生窗口、特殊列名交互与桌面流式内容仍需人工验收。

只读补齐的 Services 包括 nodeSystem 的 activity、editing、projection、subgraph、catalog、
function mutation、connection candidates，以及 projectWireParser、fileResourceService 和
executionChannelDrain。共享同步/补丁入口与其历史回归继续复用；候选响应的身份绑定、
终态排空、HMR 清理和命令回执解析没有另加平行 owner。日志批次保留队列拥有权与失败隔离，
未根据没有真实调用依据的重入假设增加框架。未变化路径没有为此次只读核查重复运行测试。

**Graph Document。** 本轮补齐 manifest 及 lib/model/identity/resource_path/change/constant_value
六个文件的全文；semantic_hash 未变化，复用上轮全文与 semantic_identity 验证。补读
document-edit validation/patch 的生产区，其余 Project、Runtime、Application、IPC、Harness
和 Analysis 消费者限定为相关段落。文档与可逆 change 归 Document，结构校验和原子 apply
归 document-edit，实际历史归 Project；GraphSemanticSnapshot 仍是唯一解析语义权威。
路径规范化不被当作文件系统授权，tabular 的精确错误与重复列检查也保留原边界。

全仓核销后删除仅有专属测试的 default_value_for、导出和该测试；真实 GUI 创建及类型变更
已经在请求携带 dataValue，Rust normalize/validate 不需要第二套默认值来源。Graph README
同步调用方责任。port_address_map 序列化改用现有 Serde collect_seq，直接消费 BTreeMap
迭代器，保持精确长度、顺序和 wire 形状，省去临时 Vec；未宣称已测量加速比。

```powershell
pnpm test:rs:package -p yss-graph-document --lib -- -- --skip semantic_hash::tests
pnpm test:rs:package -p yss-graph-editor --lib constant_edits_preserve_reference_identity_and_reject_duplicate_names_atomically
pnpm test:rs:package -p yss-project --lib fixture_writes_graph_identity_without_renaming_from_display_name
pnpm lint:rs:package -p yss-graph-document --lib
```

实际 6、1、1 项通过，共八项不同既有测试；未变的 hash 一项被显式过滤，未计为本次通过。
第一次命令少一层 pnpm 分隔符，被 Cargo 拒绝且没有运行测试，随后才用上列命令成功执行。
Clippy 无 warning，三源定向 rustfmt 与四路径 diff 检查通过；没有新增回归或 benchmark。

**Node Registry。** 补齐 manifest、lib/model/validation 全文；fingerprint 无变化，复用先前
全文与 Hash 消费者检查。注册表保持冻结声明事实；Project 状态、内核运行与图语义没有移入。
删除无人使用的 TypeRegistry::constructor、CategoryRegistry::get，以及
NodeRegistrationError::InvalidProtocol 和专属转换/格式化分支。真实错误仍经
RegistryValidationError::InvalidNodeProtocol 保留 node 和原 ProtocolError；class_members、
类型枚举与角色读取有实际消费者，保持原接口。NodeCatalog README 同步现有错误链。
本批生产净删十五行，不把同文件此前新增共享角色定义算入当前变化。

```powershell
pnpm test:rs:package -p yss-node-registry --lib
pnpm test:rs:package -p yss-node-catalog --lib numeric_type_class_contains_the_numeric_semantic
pnpm test:rs:package -p yss-graph-analysis --lib registered_function_roles_drive_dependencies_and_abi
pnpm test:rs:package -p yss-application --lib conflicting_registrations_and_node_kernel_contracts_are_rejected
```

依次 4、1、1、1 项通过，共七项不同既有测试，最后 Application 编译 56.62s、用例 0.32s。
两源定向 rustfmt、README Oxfmt 与三路径 diff 检查通过；未新增测试、未声称旧红，也未另跑
Registry Clippy。Graph 代理独立核对真实错误 source、角色、默认引用与重命名语义，没有重复
Cargo。

**Data Contract 和列身份。** 全文核对 aggregation、column_semantic、conversion、data_value、
lib、table、tabular、value_type 八个生产文件、manifest/README 和两份 wire 测试。共享值与
语义继续归 Data Contract；Arrow schema、Protocol、Graph Analysis、Kernel、Engine 等真实
消费者按相关调用段核对，未将其余包推定为本轮全文覆盖。

Arrow schema 允许非全空白的原始列名，数据库 rename 也保留两侧空格；但 Protocol 的选列、
过滤以及 TableJoin 另行要求无两侧空白，导致合法、可选中的列在操作入口被拒绝。现把既有
TabularColumnName 的判断公开为无分配 is_valid，TryFrom 和三个入口共用。trim 仅借用来
判断全空白，不改写身份；空列表、重复值、左右键组形状、后缀、类型以及过滤操作和值约束
保持原样。Data Contract README 记录唯一列名 owner，没有添加 wrapper、依赖或迁移层。

既有 DATAFRAME_NOMINAL_CODEC_VERSION 从 1 升至 2：接纳语义已经变化，该版本由 Catalog
登记到两个 nominal validator，并参与 Registry 现有指纹。没有新增兼容分支或迁移版本字段。
独立代理确认前端 ProjectColumns、FilterPredicate、Join 参数编辑链原样传递 option.name；
其中数值草稿的 trim 不涉及列名。根代理也核对实际五文件差异，保留 Engine 原有其他测试改动。

扩写 Protocol 既有 round-trip/无效输入用例，以及 Engine 原有 table_composition 用例，未新增
测试函数。实际旧红 Protocol 为 4 pass、2 fail，分别拒绝合法投影名和过滤名；Engine 为
0 pass、1 fail，在带空格键的真实 join 处返回 InvalidInput。修复后 Engine 第一次到达新增
结果断言时，夹具误把页面正整数写成 Integer；改为既有解码语义 Unsigned 后通过，该中间
失败不计为另一个生产缺陷。最终用例实际执行 DataFusion join/page，并检查精确列名和值。

```powershell
pnpm test:rs:package -p yss-node-protocol --lib dataframe::tests::
pnpm test:rs:package -p yss-database-engine --lib table_composition_preserves_order_alignment_and_combined_sources
pnpm test:rs:package -p yss-data-contract --lib --tests
pnpm test:rs:package -p yss-node-catalog --lib numeric_type_class_contains_the_numeric_semantic
pnpm test:rs:package -p yss-graph-analysis --lib composed_schemas_track_input_order_join_keys_and_mixed_series
pnpm lint:rs:package -p yss-data-contract -p yss-node-protocol -p yss-database-engine --all-targets
```

依次 6、1、3、1、1 项通过，本分路十二项不同测试。三个包 all-targets Clippy 无 warning，
四源定向格式及五路径 diff 检查通过。Catalog 该项此前已为 Registry 运行，跨分路只计一次；
本批 Rust 合计四十三项不同测试，不把四十四次绿色执行冒称四十四个不同用例。Registry、
Graph 和 Project 的前述测试先于最终 codec 改动；最后 Catalog/Analysis 检查对应最终语义，
没有声称所有 Rust 用例都在最后一步之后再次运行。未执行全部 Protocol、Engine、Application、
全仓 CI 或 GUI 的投影/过滤/连接端到端验收。

ValueType 的文本 FromStr 经 FunctionEditorProjection 和 Application catalog 的推断式 parse
真实消费，保留该接口，没有因为仅搜索显式泛型就错误删除。其递归类型输入预算仍开放：
真实 FunctionSignature.type_name 路径未见统一深度限制，但本轮没有复现栈溢出，也未擅定
协议限额。此前数值公式候选、其余内部编排和原生/Julia 验收继续按各自证据推进。

本批没有新增性能规则例外，继续保留四个 Graph JSON 边界的已有公平对照证据。三个代理均已
释放 Cargo；主代理汇总文档并执行文档契约、七份变更 Markdown 格式和全工作树 diff 检查。
整体 goal 保持开放，不把本批基础契约闭合推定为全项目合规。

22:34:36 文档契约六项通过，七份 Markdown 的定向格式及全工作树 diff 检查通过。本批汇总为
四十三项不同 Rust 测试、二十四项前端行为测试和六项文档测试；检查范围、阶段先后及未执行
验收如上，不等同完整 CI。没有暂存、提交或清理其他用户改动。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src/README.md src-tauri/crates/yss-application/src/project/README.md src-tauri/crates/yss-project/README.md src-tauri/crates/yss-node-catalog/README.md src-tauri/crates/yss-application/src/graph/README.md src-tauri/crates/yss-data-contract/README.md
git -c core.safecrlf=false diff --check
```

### 并行接续：标识准入、关系组装与诊断声明（2026-10-02）

上一轮有实际修复及验证进展。本轮起点 1157 项工作树变化，继续按三个独立分路推进；主代理
处理前端剩余边界和交叉核查，Cargo 按 Protocol、Relational、Diagnostics 顺序交接。原工作树
中的其他改动保留，不把之前已全文检查或通过的未变范围重新计为本轮新增证据。

**Node Protocol 的反序列化入口。** 补齐 manifest 及 lib/identity/types/model/parameter/value/
typing/validation/data_series 九个生产文件的全文；dataframe 是上轮已修复且未变版本，复用
完整阅读和六项测试。tests.rs 全文只作为证据，不算生产文件；至此十个生产文件累计已读。

semantic_id 宏的 new/FromStr 已验证拼写，但派生 Deserialize 直接写入私有 Box，绕过相同
不变量。真实 Project 文件 parse_graph_resource_document 直接解码图文档，外层结构校验
不重新检查节点类型标识或参数键；typed IPC 字段也走 Serde。现让宏的 Deserialize 直接
调用既有 new，合法 wire 仍是原字符串，不在 Project 或 IPC 添加第二轮同构校验。
NodeCatalog README 原参数段补充统一入口，未增加兼容或迁移分支。

扩展既有 semantic_ids 用例和 Project fixture：先验证合法扩展节点，再验证非法 node_type
和参数 map key 在真实文件 parser 被拒绝。实际旧红分别为 1 fail/1 pass、1 fail；初次测试
编译中的路径和 lifetime 修正不算行为旧红。最终验证如下，没有新增 Rust 测试函数。

```powershell
pnpm test:rs:package -p yss-node-protocol --lib semantic_ids_
pnpm test:rs:package -p yss-project --lib fixture_writes_graph_identity_without_renaming_from_display_name
pnpm test:rs:package -p yss-application --lib port_address_dto_preserves_tagged_camel_case_wire_and_typed_errors
```

依次 2、1、1 项通过，共四项不同测试；最后 Application 编译 58.68s。两源 rustfmt、README
Oxfmt、三路径 diff 检查通过。IPC 项验证既有 DTO wire 和 typed error，并编译消费者；不
声称已模拟所有非法 IPC 请求或完成原生打开项目验收。TypeDomain 的派生 Deserialize 暂未
发现实际外部读取调用，不将它当成已证实故障；ValueType 输入预算仍沿前述开放项继续核对。

**Relational Contract。** 新全文核对 lib/dataset/series/transform 四个生产文件及 Cargo，
crate 没有独立 README，复核 canonical Database Runtime README。Store 的快照租约、
Runtime 捕获、Engine 行域/物化/流、Kernel 控制传递、Application 授权和 Execution 资源
校验只回看实际 owner 函数，不重复计其他包的全文覆盖。关系请求保持中立契约，不新增
第二份表达式 IR 或结果存储；预算执行和协作取消仍由原适配器负责。

公开 assemble_series 原先只拒绝空串，接受全空白输出名，直到分页转换才因列名非法失败。
现复用 TabularColumnName::is_valid，在组装入口返回 InvalidInput，保留合法名称的原始
空格。Kernel Assemble 本来已拒绝空白，因此此处是公共 Relation API 的延迟失败，不声称
存在 GUI 故障，也未改 Kernel revision。已有 Data Contract README 已覆盖这项命名契约。

扩写 Engine 原 composition 用例，保留原组装和连接断言，再检查空白拒绝及带空格名称的
实际分页和值。真实旧红为 0 pass、1 fail，得到 Ok(RelationHandle) 而非 InvalidInput；
旧红未运行到后面的正例。单点修复后同项新绿，未新增函数或改动 transform 的其他在途内容。

```powershell
pnpm test:rs:package -p yss-database-engine --lib table_composition_preserves_order_alignment_and_combined_sources
pnpm test:rs:package -p yss-relational-contract --lib
pnpm lint:rs:package -p yss-relational-contract -p yss-database-engine --lib
```

Engine 和 Relational 各一项，共两项不同测试通过；两个包 lib Clippy 无 warning。两路径
定向 rustfmt 与 diff 检查通过，未重复无变化 Database 全套或 SCI 检查。

**Document Edit。** 补齐 Cargo、lib/error/constant_references 全文及 patch 既有测试；
validation 和 patch 生产源码与上轮未变，复用全文阅读，五个生产文件累计已核对。
Editor 导入/导出与 Project 整图复制共用常量引用重映射；显式值与协议默认值有原有优先级，
替换先收集再写入，不让早期修改影响后续依据。结构校验、协议/解析校验各有边界，真实
Graph inverse 和候选原子提交保持原 owner。此范围无修改、无新增测试，也未重复 Cargo。

**前端结果身份。** 全文补读 shared/types/report 十个生产文件、settings 六个、ui 三个，
以及 result/resultReport/domain Chart、Result DTO parser、ResultService、ProjectService、
ProjectEventParser、LogService/index 和线性系数分页适配。shared/utils 三文件与此前边界
审查重叠；其他 UI source、Rust ResultStore/IPC/report 和实际消费者只核对有关段落。原生
线性系数查询明确拒绝不可表示数值，没有凭可选呈现类型猜改统计数据或放宽 parser。

结果 ID 的范围原来由多处分别维护：普通描述符和分页接纳 0，引用和 Graph 结果接纳超过
u64 的任意长十进制文本，而 UI 页面/意图另行检查上限。Rust 从 1 分配 ResultId，IPC
读取也明确要求非零、规范 u64 文本。现由原 domain/result 拥有 isResultId，统一非零、
无前导零、最多二十位及 u64 上限；同长十进制字符串直接比较，不转成可能丢失精度的
JS number。isResultReference、结果 DTO、报告引用和两个 UI source 入口复用该 owner，
删除重复范围判断；exact keys、UUID、结果身份关联和原 published WeakMap 缓存均保留。

新增一个纯 Service/parser 回归，覆盖合法宽整数、u64 上界、0、非规范文本和超界输入。
22:43:31 实际旧红 1 fail、8 pass，十二个 soft 断言失败分别来自不同边界的错误接纳；
修改后七文件二十九项通过。两个代理独立确认 Rust 契约及原对象/缓存身份保持；一个末尾
换行正则疑点经实际 V8 核验否定，未为错误假设修改正则。Results README 同步唯一校验来源。

```powershell
pnpm test:ts src/services/result/resultService.test.ts src/shared/types/report/report.test.ts src/services/workbench/presentationService.test.ts src/features/application/presentation/parsePresentationWindowQuery.test.ts src/features/application/presentation/loadPresentationWindow.test.ts src/modules/workbench/internal/layout/workbenchPanelModel.test.ts src/modules/workbench/internal/layout/workbenchLayoutPersistence.test.ts
```

另外两处同源收敛保持现有行为：设置 parser 直接读 LanguageSettings 的 SUPPORTED_LANGUAGES；
Chart 类型与 guard 归已有 domain/chart，项目索引和 Chart 正文校验共用该定义，删除本地重复
枚举。未新增状态、schema 框架或依赖，也没有为纯去重制造旧红或新增 UI 单测。

```powershell
pnpm test:ts src/shared/types/settings/parseSettings.test.ts src/features/core/settings/settingsStore.test.ts
pnpm test:ts src/services/chart/chartService.test.ts src/services/project/projectService.test.ts src/shared/types/domain/resourceMutationValidation.test.ts
```

分别两文件七项、三文件三十六项通过，覆盖实际设置安装和 Chart/项目回执消费者。前端上述
三组共十二文件七十二项不同测试；原生界面、窗口恢复和统计计算端到端验收仍未完成。

**Graph Diagnostics。** 全文核对 Cargo、README、lib、现有生成脚本和 Domain
nodeDiagnostics/既有测试，追踪 Analysis → Runtime → Editor → IPC → Problems/Node/Pin/
Details 显示链。生成 JSON 依据原声明和生成器核对，不宣称逐项人工审读全部生成文本。

原模块手写三十二种 runtime kind 与 code match，又在宏里维护七十四条模板；其中四十二条
无生产者，仅一条在旧 function-editor-projection fixture 中留有字符串。现删除这些失效
词汇，由现有宏按同一声明同时生成原三十二种 Kind、定义表和查询方法。definition 使用
同源序号直接索引；Kind 没有 serde 数字表示或其他 discriminant 消费者。没有新 schema、
生成器或模板专用第二类枚举，也未扩大公开 kind 集合。

原生成脚本及 fixture 不变；重跑现有生成入口更新 JSON。两个独立只读比较确认保留的
code、message_key、参数顺序、severity、blocking、中英文逐字段不变，JSON 对应值也一致。
唯一旧 fixture 没有当前生产生成入口，但仍由 golden 测试用于结构解析；parser 不以模板
词汇作为输入白名单，未知代码继续走现有通用诊断显示。未把该旧 fixture 冒称为本轮重新
生成的真实当前输出，也未为删除模板篡改其内容。

```powershell
pnpm generate:diagnostics
pnpm generate:diagnostics:check
pnpm test:rs:package -p yss-graph-diagnostics --lib
pnpm test:rs:package -p yss-graph-analysis --lib editor_projection_reports_
pnpm test:ts src/features/domain/graphDiagnostics/nodeDiagnostics.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts
pnpm lint:rs:package -p yss-graph-diagnostics --lib
```

Diagnostics 两项、Analysis 实际未绑定/必填/缺资源/环四项，以及前端两文件三十九项通过。
局部 Clippy 无 warning，生成一致性、Rustfmt、README/JSON Oxfmt 和三路径 diff 检查通过。
一次误用未安装 Prettier 的命令没有格式化任何文件，随后使用仓库的 Oxfmt 入口完成检查，
没有安装依赖。该三文件净增三十五行、删除六百九十八行，没有新增测试函数或性能例外。

本批前端行为验证合计十四文件一百一十一项不同用例。Rust 前述十二项均已通过；由于
Semantic ID 的 Deserialize 影响整组类型，收尾另检查同包其余现有契约用例，复用已通过的
两项身份用例，不运行全仓 CI。所有分路保持冻结，主代理统一类型、lint、格式和文档检查。

收尾的 Protocol 其余二十八项通过，与先前两项合计覆盖本包三十个库用例；本批 Rust 合计
四十项不同测试。Protocol lib Clippy 同样无 warning。最终前端类型检查通过；lint 退出 0，
仍为 projectEventStream、useLogPanelVirtualList.test、useChartContainerSize.test 三条既有
warning。最初九文件格式检查指出 resultParser 和 parseLinearRegression 两处排版，已仅
格式化这两份源文件并复检，没有改变语义或重复行为测试。

22:59:00 文档契约六项通过，九份 TS、一份生成 JSON、四份 Markdown 的格式检查和全工作树
diff 检查通过。本批前端行为一百一十一项，加文档六项共一百一十七项；没有新增 UI 单测、
完整 CI、原生窗口或 Julia 验收。没有暂存、提交或清理其他在途修改。整体 goal 继续开放，
其余内部编排、共享类型与已列人工验收仍需完成；本批没有新增规则例外或重跑未变 benchmark。

```powershell
pnpm test:rs:package -p yss-node-protocol --lib -- -- --skip semantic_ids_
pnpm lint:rs:package -p yss-node-protocol --lib
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts src/shared/types/settings/parseSettings.ts src/shared/types/domain/result.ts src/shared/types/dto/resultParser.ts src/shared/types/report/parseLinearRegression.ts src/shared/types/dto/uiPresentation.ts src/shared/types/domain/uiPresentation.ts src/services/result/resultService.test.ts src/shared/types/domain/chart.ts src/services/project/projectService.ts src/features/domain/graphDiagnostics/diagnosticTemplates.generated.json src/features/application/results/README.md src-tauri/crates/yss-node-catalog/README.md src-tauri/crates/yss-graph-diagnostics/README.md docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git -c core.safecrlf=false diff --check
```

### 并行接续：语义投影、委派准入与共享 wire（2026-10-02）

本批主代理与三个既有 sub agent 分别处理前端、Graph、Harness、SCI 契约。阅读和独立修复并行，
Cargo 按 SCI → Harness → Graph 交接；主代理独占本文，没有暂存、提交或清理其余在途修改。

**覆盖核销。** 当前 Services 共五十二个生产 TS。本批全文补齐 Database 两文件，结合前批
明确全文审查且未变的五十文件，完成 Services 源码清单。Shared/types 共八十五个生产 TS：
domain 三十九、dto 二十三、report 十、settings 六、ui 三、根级三、插件生成类型一。
本批补读其余 domain/DTO/公共类型，合并前批 report/settings/ui、Result 契约的全文证据；
schema JSON 仍由原 Rust 插件协议及生成检查拥有，不冒称本批逐字检查了整个生成 JSON。

Runtime 三个生产文件（lib、connections、semantic_cache）与 Editor 十一个（lib、
compatibility、mutation、mutation/connection、subgraph、subgraph/clipboard、
subgraph/instantiate、projection 的 mod/model/mapper/connections）累计全文复核。
Application open/edit/catalog、Project editing 和 Analysis 的关联消费者仅按实际链读片段。
Harness Core 二十二个生产 Rust、manifest、七个 prompts 与内嵌报告 Skill 业务资源已全文覆盖。
SCI Contract 五十五个生产 Rust 累计全文覆盖：本批补齐五十三个，execution 重读但沿用既有
证据，hypothesis 沿用前批记录；SCI Prais 实现全文，Kernel 解码和 Runtime 报告只核对相关片段。
这些文件数量不代表所有算法、原生服务或模型链均已端到端验收。

**两个前端边界。** RunEvent 原用无上界正整数正则接纳 resultInspectionRequested.resultId，
与 Rust ResultId(u64) 和现有结果引用边界不一致。扩原纯 parser 用例后，23:10:00 实际一项旧红，
u64 溢出和一百位文本的两条软断言失败。现复用 domain/result.isResultId，运行 ID、事件字段和
source 规则保持原义。Parser、Execution channel drain、Rust golden wire 与 Results runtime
四文件八十八项通过，Results README 同步说明，未声称 Rust 实际生产过溢出 ID。

Rust ClipboardLastKnownPortMetadata 原样携带 Option<TypeExpr>，合法变体包含 Class；
前端剪贴板的私有递归 parser 漏掉 Class，文档/字面量却已有完整 isTypeExprWire。
原完整 fixture 加入 Applied 内嵌 Class，23:15:32 实际旧红；现删除私有 parser，复用同一 owner，
保持精确字段和原对象身份。Clipboard parser、Service、文档 wire parser 三文件三十项通过。
Database agent 独立复核两项 TS 改动，没有额外放宽；Features README 同步。
没有新增 TS 测试函数，也未声称完成原生剪贴板或全部内置 Class 生产路径验收。

**Harness 排队依赖。** 原 delegate 准入时复制 Completed 依赖，等待 slot/access 后仅查显式
资源版本。合法空资源 Review 只消费依赖报告时，可在写任务已令依赖失效后继续启动模型。
新增一个纯编排回归用 Notify 和一次 poll 固定顺序，无 sleep 竞态；实际旧红返回 Completed，
预期 Blocked（一项失败、二十六项 filtered）。现取得执行门后重验原 tasks 表；原 RAII guard
保持到依赖失效、终态事件和 outcome 登记完成，同时关闭读任务先释放门再登记结果的反向窗口。
没有新锁、状态 owner、DTO 或恢复层。编排六项及 Host 并发三项共九项通过，lib Clippy 无 warning；
Graph agent 独立检查锁顺序与取消/失败释放通过。回归使用内存 ports 和模拟 Gateway 的 typed
Save receipt，不等同真实 Project 保存 E2E。Core README 原依赖段同步，完整 Core、Application、
真实模型及原生订阅未作为本批验证。

**当前端口语义。** Analysis 按当前资源成员决定 orphan，Editor 原强制其与文档 Resolved/Orphan
标签相同。原 Runtime 函数成员用例扩展资源丢失及恢复，旧红同时出现两个 SemanticSnapshotMismatch。
现两种存储绑定都按当前语义的 orphan/can_remove 一致性接纳，继续校验节点、地址、backing、
绑定存在性与连接数量，不改写文档。另删除三个全仓无消费者的 EditorProjectionError 旧变体，
收窄唯一调用恒 Some 的 schema 参数。Editor 十五、Runtime 十三、Application 函数引用消费者
一项通过；Runtime 一个既有手动 benchmark ignored，不计通过数。Editor/Runtime lib Clippy
无 warning，Application Graph README 同步。相关连接操作的后续闭环见本节末尾。

**SCI 无消费者配置。** 删除仅定义的 CovParams::HacPanel/HacGroupsum，以及执行器从不读取的
单变体 RhoType/rhotype、唯一重导出和两处旧 fixture 字段。Prais 仍直接使用滞后残差回归，
OLS 的 hac-panel/hac-groupsum 文本仍明确返回 NotImplemented。仅三个 Rust 文件及 Contract README，
算法不变，不为纯删除声称行为旧红。Prais 两项、overall_inference 三项、Runtime 报告一项共六项通过；
三个包 lib Clippy 退出零，仍有未改 hypothesis 文件中的四条既有 warning。
SciInputViolation 与 ScientificInputViolation 的四变体重复保留为后续收敛项，不能把本批暂未
扩大到全部算法消费者说成永久规则例外。
该重复项已在下方“目录与存储边界、资源生命周期及 SCI 所有权”批次中完成收敛。

```powershell
pnpm test:ts src/shared/types/dto/runEventParser.test.ts src/services/project/executionChannelDrain.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/features/application/results/runtime.test.ts
pnpm test:ts src/shared/types/dto/clipboardSubgraphWireParser.test.ts src/services/clipboard/graphClipboardService.test.ts src/shared/types/dto/editorMutationWireParser.test.ts
pnpm test:rs:package -p yss-harness-core --lib orchestration::tests
pnpm test:rs:package -p yss-harness-core --lib host::concurrency_tests
pnpm lint:rs:package -p yss-harness-core --lib
pnpm test:rs:package -p yss-graph-editor --lib
pnpm test:rs:package -p yss-graph-runtime --lib resolution_tests
pnpm test:rs:package -p yss-application --lib renamed_unloaded_function_caller_keeps_bound_ports_in_semantic_projection
pnpm lint:rs:package -p yss-graph-editor -p yss-graph-runtime --lib
pnpm test:rs:package -p yss-sci --lib regression::linear::prais::tests
pnpm test:rs:package -p yss-sci --test overall_inference
pnpm test:rs:package -p yss-sci-runtime --lib prais_regression_report_preserves_autocorrelation_statistics
pnpm lint:rs:package -p yss-sci-contract -p yss-sci -p yss-sci-runtime --lib
```

**连接操作消费者闭环。** 继续跟踪同一恢复端口，确认 MoveConnections 只将 target 交给语义
准入、InsertReroute 完全不列原连接端点。扩同一 Runtime fixture 后，两者实际旧红分别为
GraphPortOrphan 和 reroute endpoints must not be orphaned。现原 referenced_ports 接收借用
document，列出 Move 的 source/target 及 Reroute 的两端；Runtime 和 Application 的三个调用
共同使用该入口，复用原语义准入与可逆绑定恢复。缺资源仍拒绝；成功补丁显式应用及 inverse
可恢复原 Orphan 绑定，计划阶段不改原文档。没有新增恢复规则或隐式保存。
Runtime 十三项重跑通过，新加检查的 Application 连接消费者一项通过；复用前述 Editor
十五项和另一 Application 用例，Graph 本批合计三十项不同测试。六份 Rust 定向格式、
README 格式和七路径 diff 检查通过，Cargo 释放。没有新增 Graph 测试函数。

**删除过期资源正文 delta。** 检查 SetConstant 时确认当前 Rust published ResourceDocumentPatch
只有 Function、Chart、ResourceLifecycle、ResourceMove、Database，已无 Graph 正文变体。
Graph 创建/删除仍交付 lifecycle、路径移动仍交付 move；真实 SetConstant 经 GraphCommitReceipt
和现有 GraphSession 同步返回。这不是当前 SetConstant 漏解析的生产故障，不能通过给旧协议
补一个操作来修复。前端四份生产类型/parser 仍保留无生产者的 GraphDocumentPatchDto、
GraphDocumentOperationDto 及两套专用校验，现直接删除；ResourceKey 的 graph、真实 Graph
document、编辑命令与同步协议均保留。原模块已有 UUID guard 同义复用，删除本地重复定义。

既有纯资源回执用例扩展旧 graph payload 的拒绝检查，23:25:49 实际一项旧红、另一项通过；
delta guard、wire parser 和 typed receipt guard 三条断言均复现错误接纳。清理后原两项通过，
十文件一百零二项 L2 消费者通过，IPC README 同步。四生产文件净删除二百八十七行，
不把前批同文件的签名、数据库和 Chart 修改算入本批。无新测试函数或兼容层。

本批前端三组检查去重后共十五文件、一百七十个行为用例：
88 + 30 + 102，减去重复的 golden 三十四项与文档 wire parser 十六项。
Rust 按 SCI 六项、Harness 九项、Graph 三十项合计四十五项不同测试。
变更未改变四处已测量的 Graph JSON 共享例外，没有重跑未变 benchmark 或增加性能例外。

```powershell
pnpm test:rs:package -p yss-application --lib connection_mutations_reject_model_results_even_through_resolved_generic_outputs
pnpm test:ts src/shared/types/domain/resourceMutationValidation.test.ts src/shared/types/dto/editorMutationWireParser.test.ts src/services/project/projectEventParser.test.ts src/services/project/projectEventStream.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/services/nodeSystem/functionMutationService.test.ts src/services/chart/chartService.test.ts src/services/database/databaseService.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts
```

**末轮统一检查。** 三个 agent 的源码已冻结后，根 agent 重跑前端类型检查通过；全量前端 lint
退出零，仍是 projectEventStream 的既有监听器快照提示及两份既有 UI 测试的 this-alias 提示，
没有本批新增 warning。文档契约六项通过，连同行为用例共一百七十六项不同 TS 测试。
本批九份 TS、七份 Markdown 定向格式检查和全局 git diff --check 通过；没有扩大格式化范围。
Rust 格式和 Clippy 复用各独占窗口的最终结果，SCI 四条既有 warning 保留可见，不混入前端三条。

本批未新建 UI 测试、未执行完整 CI 或原生构建，也未完成 Tauri/FlexLayout、Julia、真实模型
和真实 Project 保存的端到端验收；没有性能提升声明、新性能例外或 Git 提交。
规则审查 goal 继续保持 active，余项仍按开头覆盖表与本节列出的消费者推进。

```powershell
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts
git -c core.safecrlf=false diff --check
```

### 并行接续：目录与存储边界、资源生命周期及 SCI 所有权（2026-10-02—03）

上一轮有已落地修复和完整的局部验证，归类为实际进展。本轮沿用用户授权的三个子代理，
分别独占 Project、Application、SCI；根 agent 负责前端与本记录。起点工作树已有 1175 项变化，
不暂存、提交或覆盖其他分路。Cargo 依次交接独占窗口，源码阅读和独立前端检查并行。

**目录发布与停用订阅。** NodeCatalog 的资源水位更新原先手动复制整张请求表，即使新水位
所属项目没有请求。扩既有纯状态用例后，23:45:49 实际一项旧红、十一项 skipped：未变化的
requests 被替换。现复用 Store 已使用的 Immer，在一个 produce 内推进水位及相关请求失效，
保留 minimum、fresh、advanced、缓存和请求代次语义；没有变更的请求表及响应表继续共享。
原十二项通过；目录、两个查询 hook、创建入口、菜单与项目发布八文件七十二项 L2 通过。

NodePalette 同时调用普通及兼容目录 hook，但停用入口原仍订阅项目、请求或图版本。现普通
目录停用时项目 selector 返回 null；兼容目录停用或缺图/起始端口时，项目、版本、语义哈希及
发布版本 selector 返回稳定空值。恢复启用仍读原 owner，语言与请求准入不另建模型。
兼容目录复用 portAddressKey，删除另一个 JSON 序列化身份。没有新增 UI 测试，既有查询和菜单
用例不作为停用重渲染次数或实际桌面帧率的证明。Features README 已同步。

**视口存储信任边界。** editorViewStateMemento 原将 JSON.parse 的结果断言为坐标记录，
只检查根对象。扩既有 resolveInitialGraphViewport 用例后，23:52:41 实际一项旧红：
一个同时包含 Infinity、字符串坐标及零缩放的缓存原样进入恢复结果；不能计作三个独立回归。
现使用本模块 Zod record/strict object，在 localStorage 读取时一次校验有限坐标和正有限缩放；
坏缓存返回原空记录，恢复入口使用既有默认视口。没有格式迁移、逐帧解析或第二个持久化 owner。
存取与恢复六项通过；视口 session、画布、问题定位、项目 hydration 和快照消费者七文件
二十三项 L2 通过。与目录组重叠的快照四项去重后，本轮共十四文件、九十一项 TS 行为用例。
两项前端修复均扩既有纯用例，未新增测试函数；Plugin agent 最终只读复核通过。

**Chart 重命名版本。** Project 的 MoveChart 原只用 source + 1 覆盖目标 revision，
忽略曾删除目标路径的保留版本。新增一个真实 Project writer 回归先创建/保存旧目标、
删除到 tombstone，再将低版本源改名到该路径。实际旧红中旧目标 revision=1、删除版本=2，
重命名后又变为 1；携旧 revision 的保存返回成功并写穿当前正文。

现资源发布 owner 的 chart_move_revision 统一以 max(source, retained target) 的 checked_next
计算目标版本，delta 准备与 patch 安装复用；源路径继续保留 source + 1 tombstone。
回归核对旧基线拒绝、两路径版本、回执及磁盘/驻留正文，没有新计数器或兼容层。
Project chart 十项、Application 旧基线 writer 消费者一项通过；Project lib Clippy 无 warning，
三 Rust 格式、README 和四路径 diff 检查通过。首次测试写法的 Into 泛型编译错误已修正，
不将编译失败计作行为旧红。

**另存为的当前文档。** save_project_as_transaction 已捕获并重验完整 ProjectData，
copy_mutations 却只覆盖 Graph、Chart 和 metadata，Mind/Doc 仍复制旧磁盘字节。
第二个纯 Project 回归通过真实 Create/Edit 准备未保存正文，旧红实际读到目标 Doc 空串及
Mind 创建时标题。GUI 先 Save 再等待目录对话框，不能替代后端捕获后的当前正文契约。

现仅在既有 copy_mutations 中复用两类 FileContent::encode，把捕获的 Mind/Doc 正文写入目标；
源文件、源文档版本与 dirty 状态不变，目标激活后读到当前正文且为 clean。
文件 lease、事务、authority generation 重验沿用原流程。修复后 focused 一项通过；
file_resources 四项和 Application save_as_ 两个生命周期消费者共六项不同用例通过。
Project lib Clippy、两 Rust 定向格式、README 与局部差异检查通过。
本轮 Project 新增两个不同回归，分别保护路径版本防回退与当前正文复制，没有新增 UI 测试。

**Application 精确读取及中断分类。** InspectDatasetSchema/Profile 为确认一个声明曾复制
完整 ProjectData，Graph results 同样只为查驻留成员而复制聚合。现分别复用
read_database_declaration 和 has_resident_graph，保留原项目身份、operational 准入及错误类别；
不宣称已测量的性能收益。受控结果分页原将 Relation Cancelled/DeadlineExceeded 全部映射为
ResultUnavailable。扩既有真实 List 结果消费者后实际旧红得到两个 ResultUnavailable；
现该分页分支保留 Cancelled/DeadlineElapsed，其余失败与无控制的 report-table 分支保持原义。
五项不同 Application 用例及 lib Clippy 通过，没有新测试函数。
控制在查询前已触发，未把该回归描述为运行中取消延迟或 Core ledger/模型故障。

**SCI 单一错误与拟合元数据。** 上轮保留的 SciInputViolation / ScientificInputViolation 重复
已核实为相同四变体、derive 且无 serde 身份。现统一使用原 execution::ScientificInputViolation，
删除旧定义、重导出及 Runtime 逐变体转换，保留 AcfPacf | Regression operation gate；
Kernel 的既有分类复用同一类型，不添加兼容 alias 或 wire 变化。
linear_test 也复用 crate::error::invalid_input，删除相同错误构造器。

linear_fit 的三个实际调用现在直接传入原 config.constant，删除首列猜测及两个调用层的
立即覆盖；OLS 沿用已有准确元数据，WLS 的重复长度拒绝由原入口同一条件覆盖。
扩既有报告与 weighted_statistics 用例，确认 constant=false 时全一预测列仍是用户 predictor。
另删除上轮 rhotype 移除后 Kernel Prais literal 遗留的无效 Default update。
这些是等价收敛，不伪造行为旧红或数值加速声明。SCI/Runtime/Kernel 二十二项不同既有测试通过，
L1 重跑的一项不再累加；四包 lib Clippy 退出零，最终只剩未改 hypothesis 文件中的四条既有警告。
二十四 Rust 文件定向格式、二十七路径差异检查通过，三个规范 README 同步；Plugin 独立复核通过。

```powershell
pnpm test:ts src/features/core/nodeCatalog/nodeCatalogStore.test.ts src/features/core/nodeCatalog/localizedSearchIndex.test.ts src/features/application/nodeCatalog/useLocalizedNodeCatalog.test.tsx src/features/application/nodeCatalog/useCompatibleNodeCatalog.test.tsx src/features/application/nodeCatalog/createNodeFromDescriptor.test.ts src/modules/graph-editor/internal/ui/NodePalette.test.tsx src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts
pnpm test:ts src/features/core/viewport/editorViewStateMemento.test.ts src/features/core/viewport/resolveInitialGraphViewport.test.ts src/features/core/viewport/viewportSession.test.ts src/features/application/editor/useCanvasViewport.test.tsx src/features/application/editor/revealGraphProblem.test.ts src/features/application/project/projectHydration.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts --reporter=json --outputFile "$env:TEMP/yssbi-rules-viewport-20261002.json"
pnpm test:rs:package -p yss-project --lib chart
pnpm test:rs:package -p yss-application --lib chart_edits_persist_and_reject_an_old_baseline_at_the_writer_boundary
pnpm test:rs:package -p yss-project --lib save_as_copies_current_authored_documents_without_saving_the_source
pnpm test:rs:package -p yss-project --lib file_resources::tests
pnpm test:rs:package -p yss-application --lib save_as_
pnpm lint:rs:package -p yss-project --lib
pnpm test:rs:package -p yss-application --lib ai_reads_the_shared_result_json_and_follows_table_references
pnpm test:rs:package -p yss-application --lib command_harness::gateway::tests
pnpm test:rs:package -p yss-application --lib datasets_keep_rows_types_semantics_and_history_through_copy_export_and_lifecycle
pnpm test:rs:package -p yss-application --lib assistant_edits_current_graph_validates_runs_and_reads_actual_series_results
pnpm lint:rs:package -p yss-application --lib
pnpm test:rs:package -p yss-sci-runtime --lib acf_pacf_maps_results_and_rejects_invalid_requests
pnpm test:rs:package -p yss-sci-runtime --lib
pnpm test:rs:package -p yss-sci-runtime --test causal_operations
pnpm test:rs:package -p yss-sci --lib hypothesis::linear_test
pnpm test:rs:package -p yss-sci --lib design_and_covariance_keep_axis_order_at_report_boundary
pnpm test:rs:package -p yss-sci --test overall_inference
pnpm test:rs:package -p yss-sci --test weighted_statistics
pnpm test:rs:package -p yss-node-kernel --lib panel_nodes_select_each_implemented_estimator_and_reject_unsupported_combinations
pnpm test:rs:package -p yss-node-kernel --lib binary_and_prais_nodes_honor_options_and_predict_without_refitting
pnpm lint:rs:package -p yss-sci-contract -p yss-sci -p yss-sci-runtime -p yss-node-kernel --lib
```

Chart IO 的重定向判断另删除七行重复实现，直接复用 Filesystem 公开的
metadata_is_redirect；symlink、Windows reparse 与原错误语义不变。
最终 chart_io 六项通过，包含实际 Windows junction 用例；这六项已包含在 chart 十项中，
不重复累计。Project 最终 lib Clippy 和该源文件格式/差异检查通过。
Rust 本批不同测试合计四十四项：Project 及其 Application 消费者十七、Automation 五、SCI 二十二。

**本批源码覆盖核销。** Core 当前九十五个生产 TS/TSX 已累计全文审查：
前批明确的二十八文件，本批根读取 resource 十、nodeCatalog 三、editor 八、viewport 十二、
graphSession 一、projectLifecycle 一共三十五文件，两组重叠 nodeCatalog 三项；
子代理再补齐剩余三十五文件，不能把复读相加为九十八个新增文件。

剩余三十五的精确目录清单为 state 二、graph/read 一、database 二、dataStore 十一、
settings 三、ui 三、chart 二、execution 七，以及 canvas 的 index、edgePath、canvasNodeBounds、
canvasMutationContracts 四项。固定字段共享与当前 DTO 对照后未发现新增遗漏；
四处任意 Graph JSON 共享沿用已验证的边界，不进入动态 Immer key。
Core 不反向引用 Application/Services/Modules；Execution 的项目/运行准入仍归现有
observeGraphRunEvent 消费者，Results 查询和租约没有移入 Core。源码覆盖不是所有 UI 与
跨 owner 生命周期均已完成验收的证明。Application/nodeCatalog 五文件也已完整核对。

Project 当前四十二个 Rust 文件中，除三个独立测试/support 文件，三十九个含生产文件的
生产区已全部全文覆盖；其中三十二份文件连同内联测试完整读取，另七份仅保证生产区完整：
chart_io、file_resources、project_activation，以及 project_state 的 function_mutation、
graph_editing、graph_lifecycle、graph_operation。Application 的生命周期、Chart 保存、
Graph 执行、数据库快照及资源查询按真实调用链核对片段，不冒称 Application 全量阅读。

Application 本支全文十八个生产文件：lib、automation、automation/graph、automation/resources
及其 content/database、graph 的 mod/resources/editing、events、file_resources、docs、minds、
chart/resources、project 的 change/failure、ipc/schema/application_event 及
ipc/commands/command_harness/gateway；内联测试仅按实际验证范围读。
SCI 补全文六文件：error、regression/design、hypothesis/linear_test、time_series/models、
regression/linear/fit、causal/iv/fit；Runtime 五文件：error、regression/linear/mod、
time_series/models、diagnostics/serial_correlation、causal/iv/mod。
其余类型路径迁移仅相关片段，Contract 五十五文件沿用前批，不能据此核销全部科学算法。

根另完整读 Shared 的 appLinks、config-default 四、constants 二、platform 一、stats 一、
utils 三，共十二份辅助 TS，以及 App.css、workbench-layout.css、shared/ui/markdown.css。
这些是静态配置、平台适配与呈现，不新增数据 owner。i18n 入口与适用规则已读；
两个 locale 用项目已安装的 Babel parser 全文解析，均为单个导出的纯静态对象，没有函数、
调用、展开或计算键。该证据只核销加载/结构，不声称逐句校对四千余行翻译文案或验收实际语言切换，
也没有新增翻译单元测试、键清单 fixture 或依赖。

Settings 两处原始本地异常日志经实际契约核对，不能直接套用仅针对 Application 的 IPC
异常格式要求；没有实测原生 localStorage 异常泄露设置值，不将其冒称已证缺陷或擅建错误框架。
Project 的两个仅测试消费的 Graph API 仍是后续接口可见性核对入口，不因无生产搜索命中直接
推定可删；其消费者与 test-support 边界继续按实际使用确认。
本批未新增 benchmark、性能例外、UI 测试或 Git 提交，未运行完整 CI/原生构建、Julia 与真实模型链。

**最终统一检查。** 全部源码冻结后，pnpm check:ts 通过；pnpm lint:ts 退出零，仍为原来的
两条既有 UI 测试 this-alias 与 projectEventStream 监听器快照提示，没有本批新增 warning。
文档契约六项通过，连同行为用例共十五文件、九十七项不同 TS 测试。
六份 TS 与七份 Markdown 共十三路径定向格式检查、全局 git diff --check 通过；
三十二份变更 Rust 的定向格式复用各 agent 的最终结果，没有扩大格式化范围。
本批整体 goal 仍 active，未以 Core/Project 源码阅读或局部绿灯替代其余模块和必要原生验收。

```powershell
pnpm check:ts
pnpm lint:ts
pnpm test:ts src/tests/documentationContract.test.ts
git -c core.safecrlf=false diff --check
```

## 2026-10-03：投影与项目发布准入、View 观察契约及 VAR 边界

继续使用三个既有子 agent：Graph Execution、Rust Application 和 SCI/Node Kernel 分别独占
源码范围，根 agent 核对前端 Application 并维护本记录。阅读与独立前端检查并行，Cargo
依次交接窗口；保留混合工作树，不暂存或提交。

**前端投影刷新。** `hydrateGraphProjection` 原先在检查项目/图生命周期后发布 stale，
却未在同步通知后重验。新增一个纯 Application 回归，在该通知中替换项目并安装同路径图：
干净分支继续发出旧项目 RPC，dirty 分支则由 resolve 入口重新捕获后继项目，真实发出
successor-project 请求并改变后继 dirty 状态。00:24:21 的旧实现一项失败、九项过滤，
同时确认两条分支；它不是 UI 渲染测试。

现有刷新 owner 在 stale 通知后重验原项目、图生命周期与保存状态；失效即停止，不创建
新计数器、队列或状态源。dirty resolve 后的重复清除 stale 已删除，原会话安装本来就在
同一次 ResourceStore 发布中完成此事；完成回调只确认本次身份。异常分支同样拒绝过期
生命周期。Editor README 同步合同，另一 agent 只读交叉核对通过。

编辑队列十项先通过，随后五文件三十一项不同测试全部通过，覆盖图卸载、加载和项目发布
消费者。最终 TS 源码的类型检查通过，lint 仍为原三条警告，没有本批新增 warning。

```powershell
pnpm test:ts src/features/application/graphEditing/graphEditCoordinator.test.ts -t 'stops projection refresh when stale-state notification replaces the project'
pnpm test:ts src/features/application/graphEditing/graphEditCoordinator.test.ts
pnpm test:ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/project/projectIOStore.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts
pnpm check:ts
pnpm lint:ts
```

**前端源码核销。** 当前 Application 清单为二百五十个生产 TS/TSX，另有一份纯 typecheck。
根本批全文核对 Editor 当前六十八文件，包括打开/激活/关闭、剪贴板/历史、画布交互与投放、
保存/执行和 Workbench 适配；并完整核对 GraphProjection 三、Sidebar 七、Menubar 三、
Execution 二、DataManagement 八、DatabaseEditor 六，共另外二十九份生产文件。
GraphEditing 的 coordinator/refresh 两文件按真实刷新链重读，其二十六文件的全文范围
沿用前批。上述范围与历史批次存在重复，不把再次读取累计为新增覆盖。
领域索引、ResourceStore 发布、Graph FIFO、FlexLayout authority 和输入边界继续由原 owner
承担；代码阅读与纯用例不能替代原生分屏、语言切换、窗口和交互验收。

**VAR 工作区与报告形状。** IRF/FEVD 的 Kernel 准入按请求 horizon 和变量数估算，SCI 却先按
模型最大 lag 分配稠密矩阵。原有 golden 用例扩展后，真实拟合的 sparse [1,8] 模型先确认
短长 horizon 的共同前缀一致，再把解码模型中的远端 lag 改为 usize::MAX：旧代码在
p_model + 1 溢出，一项真实失败。该极值来自解码边界，不声称实际拟合出了这种 lag，也没有
用大内存分配复现。原 lag_matrices 现接收调用方所需上限，IRF 只构造不超过 horizon 的部分；
FEVD 复用 IRF，stability 继续使用全模型并保留其 Kernel 预算。

另一条真实 Kernel 链先生成合法 VAR Fit，再只给模型 Record 的 sigma 加一列，请求仅含
model summary 的报告。此路径不调用 SCI 的 checked_fit，旧 Runtime decorate 在 names[2]
越界；现由原报告 owner 在贴标签前检查 sigma 与变量名构成同阶方阵，返回原 ScientificFailure。
新实现还实际执行了额外行的拒绝断言；旧例在第一处 panic 后未执行该分支。最初测试 fixture
误用 Arc 容器导致的编译错误已修正，不计为行为旧红。两个问题均扩展既有测试，没有新增函数。

同一审查删除 VAR fit/后估计中只为重新借用而构造的矩阵/向量副本，varsoc 各阶复用同一输入
owner；仍需输出原矩阵的拷贝保留。只有 var.summary、timeseries.irf、timeseries.fevd 的现有
Kernel revision 从五升为六，反映对应行为变化。无新模型、通用校验器、依赖或性能例外；
不声称测得加速、内存峰值下降或完成全部 SCI 数值验收。

最终两组 SCI 目标五项、实际 Kernel 消费者一项共六项不同测试通过，均无 ignored。
三包 lib Clippy 退出零，仅原 SCI 四条警告，Runtime/Kernel 无新增。七份 Rust 定向格式和
九路径 diff 检查通过，SCI/Runtime README 同步，根 agent 只读交叉核对实现通过。

```powershell
pnpm test:rs:package -p yss-sci --test linalg_backend_golden var_full_fit_preserves_coefficients_and_stability_spectrum
pnpm test:rs:package -p yss-node-kernel --lib time_series_nodes_preserve_multivariate_postestimation_results
pnpm test:rs:package -p yss-sci --test linalg_backend_golden --test time_series_operations
pnpm lint:rs:package -p yss-sci -p yss-sci-runtime -p yss-node-kernel --lib
```

SCI 本支全文覆盖 time_series/var.rs 及 var 的 types、estimate、varsoc、postestimation 五文件；
Runtime time_series 的 mod/report 两文件，Kernel statistics/time_series 一文件，共八份生产实现。
Kernel diagnostics 和登记文件仅核对相关调用段，其余科学包的全文范围不因此扩展。

**Application 单项声明、观察身份与取消分类。** 插件 data.snapshot 只为判定一个数据库存在，
原实现复制整份 ProjectData；现复用 Project 的 read_database_declaration，缺失仍映射
plugin_dataset_invalid，其余读取失败仍为 plugin_stale_context。原插件预算用例补缺失声明
断言，另跑未发布声明消费者；这是行为保持的 owner 复用，未宣称旧红或实测性能提升。

Execution 正常为两个不同 View 保留同一结果的两条观察意图，Application finalization 却仅按
result_id 判重。扩展原真实 ExecuteGraph 用例为同一输出连接两个 View，约 00:37 的旧实现
返回 failed / FinalizationFailed，一项失败、一百四十九项过滤。现按 result_id 及 requester
的 graph/node/port 共同判重，完整覆盖其原身份字段，使用借用键；已提交结果的唯一性、存在性
检查和相同请求的重复拒绝保留。修复后实际交付两个 requester 的同一结果，原后继编辑及已提交
结果仍可读取的断言继续通过。Graph README 同步，没有增加另一份结果模型或改变结果存储。

Project Registry 的 scan/cleanup 已产生类型化 Cancelled，但 command adapter 统一转成 internal。
扩原 IPC 错误用例后，约 00:38 旧实现实际产生 internal_error 与 incidentId，一项失败。
两个命令现共用具体 mapper，将 Cancelled 转成前端既有 picker_task_cancelled，details/incidentId
均为 null；其他错误和进度 worker 失败继续保留原 incident。前端有本地取消兜底，不能据此
声称普通取消按钮必然显示错误；本修复保证后端取消经过公开 wire 时不丢分类，也不撤销已提交
工作。IPC README 同步，原 ProjectManagement 协作取消消费者通过。

本支七项不同既有用例均通过，无新测试函数。Application lib Clippy 退出零，仅依赖 SCI 的
既有四条警告；五份 Rust 定向格式与八路径 diff 检查通过。根 agent 和 Graph/SCI 同伴分别
只读核对声明读取、完整观察身份及取消 wire，没有新增状态权威。

```powershell
pnpm test:rs:package -p yss-application --lib plugin_data_boundary_enforces_the_granted_snapshot_and_aggregate_result_budget
pnpm test:rs:package -p yss-application --lib project_database_query_rejects_an_unpublished_declaration
pnpm test:rs:package -p yss-application --lib execute_graph_returns_its_committed_results_after_a_later_graph_edit
pnpm test:rs:package -p yss-application --lib picker_failures_preserve_delivery_incidents_and_expected_cancellation
pnpm test:rs:package -p yss-application --lib committed_explicit_inspect_maps_once_and_ordinary_result_maps_nothing
pnpm test:rs:package -p yss-application --lib picker_cancellation_targets_current_task_after_an_old_future_is_dropped
pnpm test:rs:package -p yss-application --lib assistant_edits_current_graph_validates_runs_and_reads_actual_series_results
pnpm lint:rs:package -p yss-application --lib
```

本支 Application 三十三份生产区全文核对：activity_panel、plugins、result_encoding、runtime，
project 的 query/registry/mod，graph 的 results/retention 与 finalization；IPC 的 runtime、mod、
activity_panel_sync、error/mod，schema 的 activity_panel/mod/project/statistics；commands 的
command_activity_panel、command_plugin、execution_dto、command_node_system 的 leases/results/reports、
command_project 的 query/path/mod/registry/lifecycle、project_failure、file_resource、command_doc、
command_mind、mod。execution_dto 保证生产区全文，测试模块只读片段；project/registry 与 ipc/runtime
属鲜活复读，不和前批十八文件简单相加。其他关联消费者只计实际调用段。
IPC 生命周期 watcher 的异步尾部归属仍是未验证候选，不称已证故障或已完成修复。

**View 准入与 Execution 阅读范围。** View 原先继承通用 data_port 的 literal Allowed，
Analysis 因而允许直接常量输入进入 Ready；实际观察计划只接受已求值输出的 Value 绑定。
新增一个纯 Graph Preparation 用例，使用真实 builtin View 和数值 42 literal，要求产生既有
literal_forbidden 诊断且不可准备计划；再改接真实 Constant 输出，确认 Ready 与 Value 观察。
旧实现一项失败、三十项过滤，失败点是 literal 仍为 Ready。现仅由 Catalog 原输入策略声明
Forbidden/default None，让 Analysis 和 Editor 原准入链执行约束，不新建结果或输入模型。
Catalog README、英文/中文 View 文档和 Execution README 同步契约。

Execution 的 ports/mod、ports/resources 两个无消费者的空模块及导出已移除。
PlanValidationControl 按 COMPONENT_REFACTOR 中既有明确决定保留；仍未接入执行流程，
不能将保留接口描述为 deadline 已生效。Project 两个测试消费的 Graph API 仍有真实跨包
Application 调用，没有因为缺少生产命中就删除。

最终 Graph Preparation 七项，加 Catalog fanout、Analysis 诊断、Editor 两项和真实
Application 双 View 执行各一项，共十二项不同测试通过。末项与上支七项中的执行用例重复，
全批统计只计一次。Execution/Catalog lib Clippy 退出零，仅 SCI 四条既有依赖警告；
三份现存 Rust、四份 Markdown 的定向格式与九路径 diff 检查通过，同伴只读核对通过。

```powershell
pnpm test:rs:package -p yss-graph-execution --lib view_requires_an_output_binding_before_plan_preparation
pnpm test:rs:package -p yss-graph-execution --lib graph_preparation::
pnpm test:rs:package -p yss-node-catalog --lib builtin_output_pins_support_unbounded_fan_out
pnpm test:rs:package -p yss-graph-analysis --lib imported_connection_and_literal_errors_are_canonical_and_locatable
pnpm test:rs:package -p yss-graph-editor --lib output_fan_out_preserves_other_branches_when_an_input_is_replaced_and_undone
pnpm test:rs:package -p yss-graph-editor --lib editor_projection_preserves_canonical_diagnostics_and_builds_node_indexes
pnpm test:rs:package -p yss-application --lib execute_graph_returns_its_committed_results_after_a_later_graph_edit
pnpm lint:rs:package -p yss-graph-execution -p yss-node-catalog --lib
```

本支完整读取 Execution 当前三十二份生产文件（原三十四份减两个空模块）：顶层 error、
finalization、graph_preparation、identity、kernel_invocation、lib、package_preparation、
resource_preparation、result、result_store、run_registry、state；plan 的 basis、identity、mod、
model、observation、package、parameter、result_category、validation 及 validation/control；
result_store 的 cache/projection/publication/retention；state 的 admission/control/dispatch/
run_lifecycle/scheduler 及 scheduler/selection。另完整读取 manifest、README、
graph_preparation 测试与 cache 测试；state/result_store 测试只计实际片段。
Application 的 run/finalization/query/leases/recovery 只计实际消费者范围。

**快照发布的同步重入。** 前端同伴完成资源与发布用例复核后，扩展既有纯集成用例的 snapshot
阶段：在真实 ResourceStore.setSnapshot 通知中切换 project-b，并写入后继项目焦点和视口。
旧回调只在开始时检查身份，通知返回后仍把旧候选的 focus/viewports 写回。00:41:37 的旧实现
一项失败、九项过滤，实际后继焦点被置 null、视口变空；00:43:40 同一用例修复后通过。
Sidebar 自身已检查绑定，不能把该复现夸大成已证 Sidebar 污染。

commitPreparedProjectSnapshot 在每次可能通知订阅者的原操作后重验 prepared 的项目身份，
覆盖输入重映射/释放、资源快照、Sidebar、Results、图会话、视口、路径映射和 Details 清理。
图路径非视口映射复用原 ProjectIdentitySnapshot，在 Details 通知前后检查同一身份；唯一
生产调用点传入原 prepared，没有添加新事务、队列或状态源。Features README 同步，
根 agent 全文复核受影响实现、唯一调用及既有消费者通过。

最终冻结前端源码后，扩大至十个直接受影响文件的 L2 检查，五十二项不同测试全部通过，
包括刷新、卸载、项目 IO、快照/文件发布、元数据、hydration 与文件输入/资源动作。
这里包含前面的五文件三十一项，不重复相加。check:ts 通过；lint:ts 退出零，仍仅原三条
警告：两个既有 UI 测试的 this-alias 和 projectEventStream 的监听器快照展开。

```powershell
pnpm test:ts src/features/application/editorMutation/projectPublicationIntegration.test.ts -t 'retains successor publications when synchronous revision and panel observers replace the project'
pnpm test:ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/project/projectIOStore.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editorMutation/projectPublicationSnapshot.test.ts src/features/application/editorMutation/projectFilePublication.test.ts src/features/application/editorMutation/projectSnapshotMetadata.test.ts src/features/application/project/projectHydration.test.ts src/features/application/resource/fileTextInput.test.ts src/features/application/resource/resourceActions.test.ts --reporter=json --outputFile "$env:TEMP/yssbi-rules-publication-20261003.json"
pnpm check:ts
pnpm lint:ts
```

**Application 全文范围闭合。** 根本批在前列范围之外完整读取 GraphDocument 一、Observability
二、Settings 四、Stats 一、UI 一及顶层五文件（errorReference、projectCommandContext、
projectLifecycleReceipt、projectLifecycleReceiptDependencies、userErrorSummary）。
连同 Editor 六十八、前列二十九和 GraphEditing 两文件，根本批全文范围为一百一十三。
同伴完整读取 EditorMutation 六与 Resource 九共十五份生产实现：editorLayoutPublicationCommit、
functionSignatureCoordinator、projectPublicationCoordinator、projectPublicationSnapshot、
projectSnapshotResources、resourceMutationResult；createFileActions、docActions、documentInputs、
fileManagement、fileTextInput、mindActions、resourceActions、useFileManagement、useFileTextInput。

当前其余一百二十二文件沿用已完成的全文范围：assistant 五、chart 五、GraphEditing 其余
二十四、initialization 四、log 六、nodeCatalog 五、plugins 五、presentation 十一、project
十七、results 二十五、statusBar 二、window 十三。113 + 15 + 122 = 当前生产清单 250，
排除测试和单独 typecheck 文件；既有 Core 95 与 Domain 14 亦已全文覆盖。上述是去重阅读
范围的闭合，重复阅读没有冒充新文件，未把源码阅读等同于所有行为或原生界面已验收。

全批 Rust 不同用例为 SCI/Kernel 六、Application 七、Graph 支十二减重叠一，共二十四项。
只有投影刷新与 View literal 两个新增纯测试函数；其他反例均扩展既有纯测试，未新增 UI 测试。
本批没有新的默认规则例外、benchmark、性能结论或 Git 提交；未运行完整 CI、原生构建、
Julia/真实模型链或原生 UI 验收。整体 goal 保持 active，后续范围以本文覆盖表和未验收边界为准。

**统一收尾。** 文档契约六项通过，含模块索引检查；合计十一文件、五十八项不同 TS 测试。
五份 TS 与十二份 Markdown 共十七路径定向格式检查通过；十五份 Rust 的定向格式复用各支
冻结后结果。全局 git diff --check 退出零。未因双端改动扩大为完整 CI，也未将工作树既有
改动计作本批新增。上述受影响消费者验证、旧红和限制均保留于本节。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src/features/README.md src/features/application/editor/README.md src-tauri/crates/yss-sci/README.md src-tauri/crates/yss-sci-runtime/README.md src-tauri/crates/yss-application/README.md src-tauri/crates/yss-application/src/graph/README.md src-tauri/crates/yss-application/src/ipc/README.md src-tauri/crates/yss-graph-execution/README.md src-tauri/crates/yss-node-catalog/README.md src-tauri/crates/yss-node-catalog/src/docs/en/view.md src-tauri/crates/yss-node-catalog/src/docs/zh/view.md src/features/application/graphProjection/graphProjectionLifecycle.ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/editorMutation/projectPublicationSnapshot.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts src/features/application/editor/cascadeGraphPathReferences.ts
git -c core.safecrlf=false diff --check
```

## 2026-10-03：缓存批次、项目 watcher 与插件撤销后的收尾

前轮属于有进展轮：已证修复和受影响检查均落在当前工作树。本轮继续原四路分工，
根 agent 负责前端与统一记录，三支分别负责 Application watcher、SCI/VEC、Plugin Runtime；
源码读取与前端验证并行，Cargo 按 SCI → Application → Plugin Runtime 依次交接。
沿当前代码验证前轮候选，不把全文阅读或历史绿灯直接当作行为已经完成。

**缓存淘汰的批次身份。** 原 enforceGraphDocumentCacheLimit 只保存路径列表，每次等待卸载后
重新读取当前资源数量并继续调用 unloadGraphDocument；后者会捕获调用时的项目身份。
在旧项目第一张图的卸载 IPC 等待期间替换项目并装入五张同路径图，00:59:38 的纯回归
实际观察到旧批次发出第二次卸载，并删除后继的 Cached-1 图；一项失败、四项过滤。
现由原缓存入口捕获已有项目生命周期，在每次卸载前后重验；失效即结束，也不删除后继的
访问时间记录。01:00:36 同例通过，随后后继项目自己的新批次仍可按原上限正常淘汰。
测试模拟后端卸载答复，实际调用缓存/卸载/资源 owner，不是 UI 测试或原生卸载验收。

**加载状态通知的同步重入。** ProjectIOStore 原本在发布 loading 后才捕获图加载身份，
ready 通知后也未重验就启动缓存清理。第二个纯用例分别在这两种通知中切换项目，安装
五张后继图，其中包括同一路径 Main。01:04:00 最终 fixture 的旧红显示：loading 分支
用 project-successor 发出原加载 RPC、重写加载状态并卸载后继 Main；ready 分支也卸载
后继 Main，两分支均错误返回 true。一项失败、五项过滤。此前不含同路径 Main 的初版
fixture 已修正，结论以此合法同路径图的最终反例为准。

loadGraph 现先捕获原项目，在通知后及开始清理前重验原项目/图生命周期；异常与完成
同样按原身份结算。缓存命中的 ready 通知后也检查原项目。原 pending 表、图生命周期、
ResourceStore 和卸载准入继续拥有其状态，没有新计数器、队列、框架或第二份图。
Editor README 同步契约。01:05:16 整份卸载/加载文件六项通过。

源码冻结并格式化后，最终 L2 九文件五十九项不同测试全绿：函数签名协调八、卸载/加载六、
可见面板同步二、图编辑队列十、项目恢复四、项目读取上下文三、结果摘要内容二、
结果查询协调五、结果 runtime 十九。类型检查通过，lint 退出零，仍仅既有三个 warning。
这里包含前面的聚焦红绿用例，不重复计数；并沿实际 loadGraph 调用核对 UI intent、
可见图与结果摘要入口。前端生产文件的历史全文覆盖不因本次重读再次累加。

```powershell
pnpm test:ts src/features/application/editor/graphDocumentUnload.test.ts -t 'stops a cache eviction batch when the project changes during an unload'
pnpm test:ts src/features/application/editor/graphDocumentUnload.test.ts -t 'stops graph loading when loading-status observers replace the project'
pnpm test:ts src/features/application/editor/graphDocumentUnload.test.ts
pnpm test:ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/project/projectIOStore.test.ts src/features/application/graphEditing/graphEditCoordinator.test.ts src/features/application/project/projectHydration.test.ts src/features/application/editor/synchronizeVisibleGraphPanel.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/results/runtime.test.ts src/features/application/results/resultQueryCoordinator.test.ts src/features/application/results/addLinearSummaryContents.test.ts --reporter=json --outputFile "$env:TEMP/yssbi-rules-cache-20261003.json"
pnpm check:ts
pnpm lint:ts
```

**VEC 样本准入与局部借用。** SCI 的两个公开 VEC 入口先检查 lags >= 1，却把样本数不足
交给共享 Johansen stage。旧 stage 在检查之前先差分并计算 n_full - p，空样本会先访问
首行，超过样本数的 lag 则先减法溢出。扩展既有 invalid-config 用例后，真实旧红在
两行样本、三阶 lag 的 vec_estimate 减法处发生：一项失败、两项过滤。旧运行未到达
vecrank 与空样本分支，修复后同一用例的四条对应调用均实际返回错误。
stage 现于差分/减法前以 n_full <= p 返回原错误，不新建校验框架。
Kernel 原有准入已经挡住这类样本，所以不声称 GUI 可达 panic，也不提升未改变的 Kernel revision。

VEC 的 stage、estimate、stats、postestimation、vecrank 去掉只为重新借用而复制的局部矩阵和
向量，输出移动及真正原地计算所需的副本仍保留。公式、数值后端和领域 owner 不变，
没有测量或声称耗时/RSS 改善。SCI README 更新；根 agent 只读核对五份生产 diff、
完整 stage 与两个公开入口准入，确认检查位置和必要副本边界。

最终 SCI time-series operations 三项和 linalg golden 两项、真实 Kernel 多变量消费者一项，
共六项不同测试通过。三包 lib Clippy 退出零，只有 SCI 原四条 warning；六份变更 Rust
定向格式和七路径 diff 检查通过。没有新增测试函数。

```powershell
pnpm test:rs:package -p yss-sci --test time_series_operations test_vec_estimate_rejects_invalid_config
pnpm test:rs:package -p yss-sci --test time_series_operations --test linalg_backend_golden
pnpm test:rs:package -p yss-node-kernel --lib time_series_nodes_preserve_multivariate_postestimation_results
pnpm lint:rs:package -p yss-sci -p yss-sci-runtime -p yss-node-kernel --lib
```

本支全文覆盖 SCI 九份生产文件：time_series/vec.rs、vec 的 types/stage/estimate/vecrank/
stats/linalg/postestimation 和 vec_vecrank_cv.rs（含其内联测试）。Runtime 转发与 Kernel
准入只核对实际相关段，不把它们的其余实现计为本轮全文阅读。

**watcher 的迟到启动与停止。** 前轮未证候选已用真实 Application load/clear 和注入的 FS
监听源复现：01:06 的旧实现一项失败、一百五十项过滤，迟到 A 启动覆盖已安装的 B 源，
迟到关闭又停止重新打开 A 的源。测试不依赖睡眠竞态，也不将注入源描述为真实 notify 投递。
原 FS watcher 的 epoch 只序列化文件监听会话，不知道哪个应用项目仍是 owner；
只在 command 外层先检查一次仍有检查与替换之间的竞态。

现有 Application watcher 适配将原 WatcherState 包在一个 Mutex 内，并由 project/change
用例在同一锁内核对当前项目再 watch；关闭尾部只有当前已无加载项目时才 stop。
Runtime 组装和三个 IPC 消费者统一使用该入口，FS 不接收项目身份，关闭回执与应用
session 身份保持原契约，没有第二个登记表或新通用管理器。
锁顺序为 watcher → 短暂捕获 session；capture_session 返回前已释放 slot 锁，FS drain
不会持有该锁。先显式释放 watcher guard 再析构捕获的 session；实际 sink 仅 reconcile
与事件交付，已检查的桌面/Application 调用中无同步反向获取 watcher 锁的路径。
此保证是监听源操作的准入与串行，不宣称 session 发布和 FS 是同一个原子事务。

Project README 同步，根和 SCI 同伴只读交叉核对通过。最终新增用例一项、既有 close
一项、save_as 两项、published-session observer 一项，共五项不同测试全部通过。
Application lib Clippy 退出零，只有 SCI 既有四条依赖 warning；四份 Rust 定向格式、
Project README 格式与五路径 diff 检查通过。未执行真实 notify 或原生桌面并发操作验收。

```powershell
pnpm test:rs:package -p yss-application --lib late_watcher_lifecycle_tails_preserve_the_current_project_source
pnpm test:rs:package -p yss-application --lib closing_project_releases_session_before_registered_deletion
pnpm test:rs:package -p yss-application --lib save_as_
pnpm test:rs:package -p yss-application --lib replacement_observer_can_capture_the_published_session
pnpm lint:rs:package -p yss-application --lib
```

save_as_ 实际匹配 save_as_registry_failure_preserves_source_and_disk_with_exact_receipt 和
save_as_activation_failure_and_success_return_exact_direct_receipts，均实际执行。
本支 Application 完整读取十一份生产区：project/change、project/lifecycle、session/slot、
runtime、ipc/commands/command_project/lifecycle；ipc/schema/catalog，command_node_system
的 catalog/common/connections 及 ipc/channel 的 presentation/mod。前五包含复读，
后六补充边界阅读，不能与旧三十三/十八简单相加。另全文读取 FS watcher 的
mod/lifecycle/drain/notify 与相关契约，Project 路径/session owner 只计实际片段。

**Plugin Host 读取的撤销收尾。** bridge.host_request 在开始时检查 context，原来直接返回
同步 HostServices::invoke 的结果。Application 的 data.snapshot 在耗时写入、哈希之后
登记 sources/leases；如果 detach 或进程故障先撤销 context 并清理，迟到登记会落在第一轮
release_context 之后。原生反例复用实际插件进程与可暂停 HostServices 的 data.list 调用，
撤销旧 view、建立后继 view 后才放行旧调用；旧实现实际返回 Ok，一项失败、四项过滤。
这个用例验证共同 host_request 出口和受控迟到资源登记，不冒称完整 Arrow snapshot 竞争。

现仅为 data.list/data.snapshot 在宿主调用返回后重验原 context；失效时调用原 release_context
再拒绝答复。view 与 task 的 context 都使用独立 UUID，Application 精确删除对应 source
以及 lease.context 相等的文件，文件删除在锁外；后继 view/task 和 export grant 不受影响。
results.commit 与 data.release 不进入新增后置分支，已提交回执和释放确认保持原语义。
这是同步调用返回后的检查，不是 Rust await。根 agent 及两位同伴只读核对实际 Application
HostServices 和两端调用链通过，没有新增资源登记 owner。

同一审查排除了“闲置 view 必须阻止维护”的推断：实际维护准入检查活动 lease、启动、
其他 mutation 与宿主发往插件的请求，随后撤销闲置 view；前端卸载成功后移除面板。
仅修正 Runtime README 的过强旧文案，没有擅加 view-busy 行为。任务归档并发尾部仍是
未取得独立失败证据的候选，没有据此改写状态机。

最终新增原生用例绿，随后四个 view_ 原生用例全部通过（含该新增用例）；库五项、安装
四项通过，共十三项不同测试。Runtime lib/tests Clippy 退出零且无 warning；两份 Rust
定向格式、README 格式与三路径 diff 检查通过。最初一次少了 pnpm 参数分隔符，仅为
Cargo 参数错误，没有运行测试，不计旧红。复用现有打包产物，没有重打包或启动 Julia。

```powershell
$env:YSSBI_PLUGIN_TEST_PACKAGE = 'G:\006RustProject\YssBI\target\plugin-packages\yssbi.julia-0.2.0-dev.1790939008718.b62dc5e7d339-x86_64-pc-windows-msvc.yssplugin'
pnpm test:rs:package -p yss-plugin-runtime --test native_extension view_host_reply_rejects_revocation_and_releases_late_context_resources -- -- --ignored --nocapture
pnpm test:rs:package -p yss-plugin-runtime --test native_extension view_ -- -- --ignored --nocapture
pnpm test:rs:package -p yss-plugin-runtime --lib --test installation
pnpm lint:rs:package -p yss-plugin-runtime --lib --tests
```

Runtime 当前十份生产源码已全文覆盖：lib、activation、bridge、installation、process、storage、
tasks、ledger、package、diagnostics，另读取 manifest/README。实际消费者包括 Application
plugins 的完整 HostServices 实现、command_plugin 入口；Runtime 组装、前端卸载顺序和
Protocol 清单/路径约束只计实际相关段。既有拆分、授权或 SDK 复核不能代替此次内部全文阅读，
也不把这些读取简单累加成更多文件。原生 fixture 使用自己的新临时目录；没有重试此前
被拒绝的四个旧目录删除。

全批 Rust 去重为 SCI 六、Application 五、Plugin Runtime 十三，共二十四项。新增四个测试
函数分别覆盖缓存批次、加载通知、watcher 尾部、插件读取撤销，触发与责任入口各不相同；
VEC 只扩展既有用例。前三个为纯应用逻辑/注入源验证，后一个为实际插件进程验证，
均未新增 UI 单元测试。两端规则保持原默认，没有新的性能例外、benchmark 或测量结论。
未运行完整 CI、原生桌面/notify 验收或 Julia 完整计算链；未提交 Git。整体 goal 仍 active，
下一轮以当前覆盖表、尚未读完的 Rust 内部和实际跨 owner 消费者继续推进。

**统一收尾。** 文档契约六项通过（包含模块索引），连同前端行为验证为十文件、六十五项
不同 TS 测试。三份 TS 与五份 Markdown 共八路径定向格式检查通过，十二份 Rust 的
局部格式复用各分路冻结后的结果；全局 git diff --check 退出零。前端类型检查及 lint
沿用本轮冻结源码后的结果，没有为了汇总再次运行未变化的行为检查。前端同伴复核确认
有效缓存命中和 pending 去重仍保留；Rust 装配注册的 watcher 类型也与三个 IPC 消费者一致。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src/features/application/editor/README.md src-tauri/crates/yss-sci/README.md src-tauri/crates/yss-application/src/project/README.md src-tauri/crates/yss-plugin-runtime/README.md src/features/application/editor/graphDocumentCachePolicy.ts src/features/application/project/projectIOStore.ts src/features/application/editor/graphDocumentUnload.test.ts
git -c core.safecrlf=false diff --check
```

## 2026-10-03：Application 源码核销、合法列名与取消会话身份

本批继续复用三个子 agent，按 Application、Kernel/Analysis、SCI 分路；主线程补 IPC，
SCI 完成后转读 IPC 命令及取消消费者。源码阅读、修改与交叉复核并行，Cargo 验证依次交接，
避免不同进程争抢同一构建缓存。原工作区混合改动保持原位，没有 Git 提交或暂存。

**Application 全文范围。** 当前清单有 113 个含生产代码的 Rust 文件。此前各批按明确路径
能核销 64 个，本批补齐另 49 个，累计覆盖当前全部生产区。内联测试只按必要片段阅读，
外置测试不计入生产数量；全文阅读不代表跨模块并发、桌面或原生生命周期已全部验收。
补齐清单相对 `src-tauri/crates/yss-application/src/`：

- Session 七文件：`session/{mod,application_session,components,database,factory}.rs`、
  `session/slot/{replacement,recovery}.rs`；Harness 两文件：`runtime/harness.rs`、`harness.rs`。
- Graph 十文件：`graph/{run,open,inputs,edit,catalog}.rs`、
  `graph/results/{mod,structured,structure,report}.rs`、`graph/results/report/presentation.rs`。
- Database 九文件：`database/{mod,samples,mutation,import,export,error,edit}.rs`、
  `database/mutation/project.rs`、`database/edit/operation.rs`；Chart 三文件：
  `chart/{mod,query,projection}.rs`。
- IPC 主线程九文件：`ipc/schema/{result,graph_mutation,graph_editing,graph_clipboard,editor_projection,database}.rs`、
  `ipc/graph_editor_sync.rs`、`ipc/channel/{graph_activity,execution}.rs`。
- IPC 子分路九文件：`ipc/commands/{command_presentation,command_node_system,command_harness,command_chart}.rs`、
  `ipc/commands/command_node_system/{resources,execution,editor}.rs`、
  `ipc/commands/command_dataframe/{mod,error}.rs`。

Presentation 的 main-window claim/settle、Chart 当前覆盖保存契约、Database 快照重验与行投影、
Harness 订阅清理等入口没有因重复验证而新建第二个 owner。其已有提交和错误契约继续保留。

**只读与所有权收敛。** Application 的数据库事实按值移交 Runtime，后者继续通过
`DatabaseSessionOpenRequest::into_validated_parts` 完成唯一完整准入；删除 Application 对
同一声明和 observation 的预扫描及其不可再产生的私有错误分支，保留空会话检查及外层失败契约。
Graph run 的资源读取只使用请求自带文档，删除未参与读取的整份 ProjectData 克隆、恒不走的
可选文档分支及对应 ProjectSnapshot 错误映射；后续 Project 执行授权仍在原入口验证。
IPC result-state mapper 直接借用原结果投影，更新编辑会话与查询命令两个生产调用方；查询的
Option 通过 `as_ref` 借用，只构造最终拥有所有权的 DTO。字段、顺序、错误和 wire 未改变。

SCI Logit/Probit 的拟合投影直接借用模型拥有的 y/x；IRLS 的交叉乘积借用当前 xw/zw，
计算出的系数和协方差直接移动，置信区间继续复用同一算子。仍保留需要原地行加权的设计
矩阵副本。没有改变公式、迭代配置、公开输入/输出、Kernel revision 或既有科学错误改动。
主线程与另一分路均核对了借用算子及消费者；等价收敛不伪造旧红，不新增同构测试或加速数字。

该 SCI 分路全文读取八份生产源码：`regression/discrete/{mod,fit,logit,probit,postestimation}.rs`、
`regression/postestimation/evaluation.rs`，以及 Runtime 的 `regression/discrete/mod.rs`、
`regression/report.rs`。Kernel、Runtime 测试和 Linalg 只核对本次消费者相关段，不扩计全文。
最终七个不同既有测试通过：binary postestimation 三、Logit 两、Runtime 报告一、实际 Kernel
Binary/Prais 消费者一，均无失败或 ignored。三包 lib Clippy 退出零，仅原 SCI 四条警告；
三份 Rust 定向格式与四路径 diff 检查通过。

```powershell
pnpm test:rs:package -p yss-sci --test binary_postestimation
pnpm test:rs:package -p yss-sci --lib regression::discrete::logit::tests
pnpm test:rs:package -p yss-sci-runtime --lib regression::tests::binary_regression_reports_preserve_likelihood_statistics
pnpm test:rs:package -p yss-node-kernel --lib binary_and_prais_nodes_honor_options_and_predict_without_refitting
pnpm lint:rs:package -p yss-sci -p yss-sci-runtime -p yss-node-kernel --lib
```

**列名复用原契约。** Data Contract 允许周围含空格的非空白列名，并保留原拼写；
Analysis 聚合/变换和 Kernel 聚合此前另行要求 `trim == 原名`，使真实目录选择的合法列
在分析或执行阶段被拒绝。三处现在复用 `TabularColumnName::is_valid`，列名仍按原字节
匹配，不裁剪、不另存规范化名称；空白、重复、缺失列的拒绝保留，operation 值继续精确匹配。
只有 Describe/GroupBy Kernel 的行为变化，implementation revision 从 1 升为 2；
Frequency/SeriesDescribe 仍为 1，Catalog 参数与端口没有改动。

扩展两个既有 Analysis 用例后，旧实现的聚合和变换各有一个实际失败；修复 Analysis 后，
四个 `schemas_` 用例通过。此时真实 Application 描述/分组执行链在旧 Kernel 上仍以
InvalidParameter 失败；修复后同一消费者通过。最初一次 fixture 端口笔误只产生 NotReady，
已修正，不计作缺陷旧红。最终 Analysis schema_resolution 二十、Application 描述/分组一、
变换一，共二十二项不同既有测试通过，未增加测试函数。Kernel/Analysis all-targets Clippy
退出零，仅 SCI 原四条警告；五份 Rust、两份 README 格式及七路径 diff 检查通过。

```powershell
pnpm test:rs:package -p yss-graph-analysis --lib schemas_
pnpm test:rs:package -p yss-graph-analysis --lib schema_resolution::tests::
pnpm test:rs:package -p yss-application --test numeric_execution descriptive_nodes_execute_graph_constants_and_feed_downstream_operations
pnpm test:rs:package -p yss-application --test numeric_execution transformation_graph_keeps_series_lazy_through_conditional_projection_and_row_operations
pnpm lint:rs:package -p yss-node-kernel -p yss-graph-analysis --all-targets
```

该分路全文重读 Kernel 的 alignment、series、aggregation、distribution、relational、transforms
及 statistics/common/models 七文件；随后核销历史证据，发现七个均已在前批全文清单中，
所以不算新增覆盖。Catalog 的 dataframe/aggregation、families、distribution/mod 三文件
在本批补充全文。Kernel 77 个生产文件中，本轮开始可从主报告定位 35 个全文证据；随后
补充全文读取 statistics 的 regression、panel、panel/models、diagnostics、diagnostics/models
及根 linear_summary 六文件，累计明确覆盖 41/77。其余 36 个仍须逐项核销，不能把相关片段
阅读等同全文。原有无关 Meta/科学新增保持原位。

**取消使用执行会话身份。** 原先 IPC 与 Service 只传 runId，而每个新 Execution session 的
RunRegistry 都从 1 分配；旧请求迟到后，Application 再捕获当前会话就可能误接纳后继的同号
运行取消。现在沿原取消链传递已有 executionSessionId；IPC 校验 UUID，Application 捕获
一次会话、比较身份后，只对该 captured execution 执行取消。之后即使换代，也不重新查找
当前 owner。身份不匹配使用原 stale-session 错误路径，没有旧参数兼容入口或新增身份模型。

Core GraphExecutionState 用事件现有 GraphRunIdentityDto 替换单独 runId，读投影仍派生
原 UI runId。取消读取这份完整身份，不依赖输出列表，所以空输出运行同样可取消；终态
必须同时匹配 session 与 runId，旧会话同号终态不能清掉后继。submit/clear 仍在原 Store
更新入口清除运行身份，未增加第二个状态 owner 或 UI 单元测试。

一个新增 Rust 回归在两个真实 ApplicationSessionSlot 会话中登记同号 RunId(1)。旧 API
实际返回 `Ok(Requested)`，未拒绝跨会话取消；修复后同场景传原身份被拒绝，传当前身份
仍可请求取消。fixture 登记没有 control token，因此旧红证明的是误接纳取消，不声称测试
观测了 AtomicBool 或实际线程停止。最终 graph::run::tests 四项通过，包含该回归及原三项。
前端扩展的三个既有 Service、caller、Store 用例分别实际旧红；修改后六文件二十八项通过，
类型检查与 lint 退出零，lint 保留既有三条警告。

```powershell
pnpm test:rs:package -p yss-application --lib cancellation_rejects_a_replaced_execution_sessions_same_run_id
pnpm test:rs:package -p yss-application --lib graph::run::tests
pnpm check:ts
pnpm lint:ts
```

前端实际目标如下；另外两个测试文件和 benchmark 只更新内部运行状态夹具/读取字段，
benchmark 没有运行，没有生成新的性能数字。十一份 TS 的定向格式检查通过，保留的三条
lint 警告仍在 projectEventStream、useLogPanelVirtualList.test.tsx、useChartContainerSize.test.tsx。

```powershell
pnpm test:ts src/features/application/editor/cancelActiveGraphRun.test.ts src/services/project/projectService.execution.test.ts src/features/core/execution/useExecutionStore.lifecycle.test.ts src/features/core/execution/read.test.ts src/features/core/execution/graphRunArtifacts.test.ts src/features/application/editor/observeGraphRunEvent.test.ts
```

**Application 合并验证。** 结果 wire 四项、真实 Session 候选、Dataset 导入/编辑/保存/重开、
Graph 打开、Assistant 编辑/执行、数据资源授权及断连后结果状态查询各一项，共十项通过；
复用已冻结的 run 组四项，本批 Application 为十四项不同测试。结果查询命令没有独立
原生 invoke 测试，依据是共享 mapper 四项、真实查询消费者和 crate 编译，不能描述为
实际桌面命令验收。Application lib Clippy 退出零，无本包新警告，依赖仍为 SCI 原四条。

```powershell
pnpm test:rs:package -p yss-application --lib ipc::schema::result::tests
pnpm test:rs:package -p yss-application --lib candidate_build_binds_one_session_identity_and_rejects_mismatch
pnpm test:rs:package -p yss-application --lib project_import_edit_cast_undo_save_and_reopen_use_committed_dataset_snapshots
pnpm test:rs:package -p yss-application --lib graph_open_replacement_respects_final_materialization_commit_boundary
pnpm test:rs:package -p yss-application --lib assistant_edits_current_graph_validates_runs_and_reads_actual_series_results
pnpm test:rs:package -p yss-application --lib agent_graph_execution_requires_authority_for_its_real_dataset_dependencies
pnpm test:rs:package -p yss-application --lib disconnecting_a_decompose_view_preserves_other_consumed_branches
pnpm lint:rs:package -p yss-application --lib
```

Application 全文核销后仍有一个明确待验证的跨 owner 候选：旧绑定的 Harness create/list/open
请求可能在 `load_active_sessions().await` 后，将后继绑定的 Active 会话写成 Stale。已核对
实际 IPC 调用、Core reconciliation 与 SQLite 非 CAS 更新，没有观察到能排除此调度的外层
排序；但尚未运行确定性回归，不声称已复现。下一步是在原 SessionStore 测试替身中用 Notify
停住旧读取，建立后继会话后放行，验证后继仍 Active，再由原会话 owner 解决排序与当前性。
不能只在 await 后增加一次检查并忽略后续写入窗口，也不能因源码已全文阅读就核销此项。

**Panel 类型化结果及输出边界。** Kernel Panel Fit/Compare 原先把 Runtime 的完整 JSON
结果立即反序列化为 PanelFit；现在两处直接使用已有 `fit_model`，删除仅被它们消费的 Runtime
JSON wrapper，SCI 算法入口不变。Panel Models 与 Diagnostics Models 的两份错误映射六个
分支与既有 `common::computation_error` 相同，直接复用。Runtime `src/panel/mod.rs` 在这段
也已全文读取；这些调整不引入依赖、平行模型或另一套报告 owner。

交叉复核没有将 JSON 往返删除直接认作完全等价：必需 f64 的非有限数此前可能在反序列化
时报 ScientificFailure，可选字段也可能被 JSON 变成 null 后作为 None 接受。直接 Fit 出口
已有 typed finite 校验；Compare 却先将模型拼进 JSON，之后再检查已丢失数值事实的 Value。
一个真实八行完美拟合输入，`x=[-1,-1,-1,-1,1,1,1,1]`、`y=1+2x`，每实体两个时间点，
Between/Entity/nonrobust，实际产生无限 F；扩写原测试确认候选 Compare 成功输出 null，
而 Fit 已报告 NonFiniteResult。该失败是实际输出边界证据，不与另一个零响应 panic 混为一谈。

Compare 现于 JSON 拼装前复用同一个有限数验证器，仅调整原验证函数的内部可见性；
合法 None 仍保留，非有限结果明确拒绝。Fit/Compare 的真实 implementation revision
在 `statistics/mod.rs` 从 5 升为 6；输出数量仍分别为三和一，没有改变端口契约。
扩展既有用例先失败再通过，没有新增测试函数或恢复完整 JSON 往返。

本阶段七项不同测试通过：Kernel `panel_` 实际四项（包含一项 spatial panel 消费者），
diagnostic 一、Runtime comparison golden 一、Application 全部非 deferred 内建目录装配一。
选择最后一项是因为它通过真实会话构造改过的内建 Registry；不以自定义数值扩展装配冒充
该消费者。Kernel/Runtime lib Clippy 通过，仅原 SCI 四条警告；八份 Rust、两份 README
格式及十路径 diff 检查通过。随后 OLS 小修的最终受影响验证另列，阶段结果不替代后续验证。

```powershell
pnpm test:rs:package -p yss-node-kernel --lib panel_
pnpm test:rs:package -p yss-node-kernel --lib diagnostic_adapters_validate_paired_inputs_and_propagate_execution_limits
pnpm test:rs:package -p yss-sci-runtime --test diagnostics_models_golden model_comparisons_reject_mismatched_samples_nonnesting_and_invalid_execution
pnpm test:rs:package -p yss-application --lib all_non_deferred_builtin_nodes_are_available_in_catalog_and_sidebar
pnpm lint:rs:package -p yss-node-kernel -p yss-sci-runtime --lib
```

**OLS 未定义推断返回原错误。** 上述数值探针另外实际观察到：八行零响应、predictor 为
行号、entity 为两行一组、time 为组内 0/1 的 Between 输入，进入 OLS 时零系数/零标准误
得到 NaN t。OLS 随后把它交给 Student-t CDF，本地 statrs 0.19.1 在 beta 函数内 panic。
这是与无限 F/null 输出不同的真实失败，先用同一现有 Kernel case 再次复现，再修复。

修复只在原 OLS inference 的 CDF 调用前拒绝 NaN t，返回已有 OlsFitError::Inference；
不制造 p=0/1，不改变必需结果字段，不拒绝原来的正负无限 t 路径。Panel 沿既有错误映射
报告 ScientificFailure，没有新增错误类型。原完美拟合非有限输出断言继续保留并通过。
本段完整读取 OLS 的 mod/fit/inference、linear/mod 与 SCI/error 五文件；Panel、Runtime、
协方差、Contract 和 statrs 只追踪相关调用，不因此扩计整模块全文。

最后三个实际目标均一项通过、无 ignored：Kernel 原面板回归、SCI OLS golden、Application
节点拥有 OLS 参数的实际执行。前者与本批 `panel_` 重叠，后者已在 App run 组计数，因此
只新增一项不同的 SCI golden；不把复跑重复累加。四包 lib Clippy 退出零，仅 SCI 原四条
警告：categorical 两、nonparametric 一、sample_mean 一。两份 Rust 格式和三路径 diff
检查通过。WLS、GLS、Prais 同形 CDF 调用尚只有源码线索，未改动、未冒称验证完成。

```powershell
pnpm test:rs:package -p yss-node-kernel --lib panel_estimation_predictions_and_comparison_failures_are_explicit
pnpm test:rs:package -p yss-sci --test regression_golden test_ols_golden
pnpm test:rs:package -p yss-application --lib node_owned_ols_parameters_change_the_prepared_plan_and_results
pnpm lint:rs:package -p yss-sci -p yss-sci-runtime -p yss-node-kernel -p yss-application --lib
```

**共享算法的实现版本。** Kernel 的既有契约要求行为变化推进 implementation revision，
失败行为也不豁免。继续追踪实际 OLS 调用后，在原有三个注册 owner 精确更新以下十九个 ID；
ID 前缀均为 `yssbi.statistics.`。没有新增版本框架或批量改变其他默认分支。

| ID 后缀                                                                                                                                                                                              | revision |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| `linear.fit`                                                                                                                                                                                         | 7 → 8    |
| `econometrics.panel.fe`、`re`、`fd`、`between`（同一前缀，四个）                                                                                                                                     | 5 → 6    |
| `regression.hierarchical`、`regression.stepwise`、`regression.curve`、`workflow.regression.baseline`、`workflow.regression.univariate_multivariable`、`workflow.regression.grouped`、`transform.rcs` | 2 → 3    |
| `workflow.moderation`、`workflow.moderation_advanced`、`workflow.mediation`、`workflow.moderated_mediation`、`sem.path`、`doe.response_surface`、`timeseries.ecm`                                    | 3 → 4    |

Panel Fit/Compare 保持本批已推进的 6，不在同一变更中重复递增。Summary/Predict 不因只读
模型而随改；未改 WLS/GLS 算法。`cluster_robust` 也读取 OLS，但当前零响应先被 Wald 协方差
检查拒绝，其他可达差异尚未闭合，不混入此次已核准组。这里核对的是共享调用和注册相关段，
不把这些消费者整文件都计为新全文。

最终注册源码冻结后，真实 Application OLS 参数/计划消费者与全部非 deferred 内建目录装配
各一项再次通过；Kernel/Application lib Clippy 通过，仅 SCI 原四条警告。三份注册 Rust、
Kernel README 格式及四路径 diff 检查通过。该复验不重复累计不同测试数。

```powershell
pnpm test:rs:package -p yss-application --lib node_owned_ols_parameters_change_the_prepared_plan_and_results
pnpm test:rs:package -p yss-application --lib all_non_deferred_builtin_nodes_are_available_in_catalog_and_sidebar
pnpm lint:rs:package -p yss-node-kernel -p yss-application --lib
```

本批 Rust 去重为 51 项：Binary 七、列名链二十二、Application 合并十四、Panel/诊断及
装配七、OLS 最终额外 golden 一。前端行为检查六文件二十八项，类型与 lint 通过。
只新增取消身份一个测试函数；其余行为修复扩展既有用例，所有 UI 交互仍留人工验收。
没有新性能例外、benchmark 测量或未测量的加速比，也没有运行全量 CI、原生桌面、
真实 notify 或 Julia 完整计算链。源码全文核销和这些定向绿灯均不代表全项目已合规。

下一批优先闭合 Harness 绑定并发、OLS 同形 CDF 候选及其真实消费者，再继续 Kernel 尚缺
明确全文证据的 36 个生产文件：visualization 两；causal 两、power、psychometrics、quality、
regression_models 及其 budget、time_series/forecast、survey 两；decision 十、doe 四、meta 五、
path 五。此次 registration 片段核对不替代 regression_models 全文；保留原有科学新增和用户改动。
SCI/Database 其余内部与原生消费者继续按前表核销，整体 goal 保持 active。

**统一收尾。** 文档契约六项通过（包含模块索引）；连同前端行为检查为七文件、三十四项
不同 TS/文档测试，Rust 为五十一项不同测试。本文和七份改动 README 共八路径定向格式
检查通过；Rust 与十一份 TS 复用分路源码冻结后的局部格式结果。全局
`git -c core.safecrlf=false diff --check` 退出零。最终文档补齐后再检查本文格式与文档契约，
并再次执行同一全局差异检查；没有提交 Git，也没有把人工验收标为完成。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src-tauri/crates/yss-sci/README.md src-tauri/crates/yss-application/README.md src-tauri/crates/yss-application/src/graph/README.md src-tauri/crates/yss-application/src/ipc/README.md src-tauri/crates/yss-node-kernel/README.md src-tauri/crates/yss-graph-analysis/README.md src-tauri/crates/yss-sci-runtime/README.md
git -c core.safecrlf=false diff --check
```

## 2026-10-03：Harness 会话写入与提交身份、数值错误边界及 Kernel 全文核销

本批继续使用三个已获用户授权的子代理：分别负责 Harness 并发、SCI 推断边界和 Kernel
适配层；主代理负责输入适配、Catalog、交叉审查及本文。源码按 owner 划分，Cargo 按实际
依赖交接；Runtime 单包验证期间可以编辑其下游 Kernel/Application，不在 App 编译期间
改变这些依赖。没有提交 Git、清理其他工作树改动或重试删除此前被拒绝的插件目录。

**Harness 的旧请求不能覆盖后继绑定。** 原 Application 在异步协调旧会话后才重验当前
项目。实际暂停 SessionStore 的 load/update 后，旧 list/create/open 扫描会把后继新会话
写成 Stale；旧整条 UPDATE 也会恢复已经被新请求替换的绑定。返回前发现 Changed 不能
撤回这些写入。另一个实际旧红证明，submit 的首次标题更新同样会把已经过期的记录写回 Active。

Core 现有 HarnessHost 新增一个不保存项目事实的异步会话访问锁，具体
HarnessSessionAccess guard 提供协调、创建、列表和打开操作，原四个无门 Host 入口改为
私有实现。Application 捕获原会话，取得锁后重验，协调后再次重验，持锁完成会话选择和
最终重验。当前项目仍只有 Application slot 这一事实源，其同步锁不跨 await。
原 create_session 也取得同一锁；submit 只在读取、准入和标题写入阶段持锁，在创建 turn、
运行模型之前释放。取消不等待 turn 结束，没有引入会话锁与模型执行锁的等待环。

同一保存项目重开后，对话 ID 可以不变，但 project binding 已变化。第三项实际旧红证明，
只传 session ID 的排队 submit 会使用后继绑定开始执行。现在 Core submit_turn 必须接收
已有 ProjectSessionBinding，在上述锁内读取记录后核对 Active 和预期绑定；不匹配沿用
SessionNotActive。唯一生产调用方 IPC 保留 Application open 返回的记录并传入其 project，
不重新捕获后继身份，不增加旧签名兼容入口或新 wire 字段。

三个新增纯 Application 测试分别验证不同回归：旧扫描覆盖新会话、整记录迟到写回及
排队选择副作用、同 ID 对话重绑定后的旧提交准入。第二项扩展同类标题写回路径，第三项
独立验证拒绝后事件不变及当前绑定正常提交；因此超过两个测试的新增项有独立失效边界。
三项均实际先红后绿；没有新增 UI 测试。Core 既有测试调用仅适配新的必填绑定。

**WLS、GLS、Prais 沿原错误契约拒绝未定义推断。** 扩展 Runtime 两个既有用例，实际
捕获四处 statrs XOutOfRange panic：WLS/GLS 的零响应 t 值、Prais 的零响应 F 值，以及
对称精确拟合中零截距的 t 值。开启 backtrace 后逐一确认 Student-t/Fisher CDF 调用，
不是从第一处失败推断其他分支。旧命令实际为 1 通过、2 失败；pnpm/Cargo 退出 101，
不能把恢复临时环境变量的外层 PowerShell 退出零视为测试成功。

修复只在三个 SCI 算法已有计算位置加入四个 NaN 检查，返回原 String/SciError，再由
Runtime 映射原 ComputationFailed。不新增平行校验模型、不制造 p 值、不拒绝原有无限
t/F 路径。Runtime 三项、SCI weighted_statistics 三项和 Prais 两项共八个不同用例通过。

**复用已有错误映射及实现版本。** Kernel regression_models、time_series/forecast、
causal/models 和 visualization 的六分支科学错误映射与 common::computation_error 完全
相同，现删除四份实现，直接复用该函数。可见性仅扩大至 crate::builtins，使相邻绘图
模块可用；错误行为不变，未为这项去重新增测试或版本。

本批只推进三个真实受影响的 implementation revision：

| ID                                       | revision | 依据                                                               |
| ---------------------------------------- | -------- | ------------------------------------------------------------------ |
| yssbi.statistics.linear.fit              | 8 → 9    | WLS/GLS 未定义推断由 panic 改为原错误                              |
| yssbi.statistics.prais.fit               | 5 → 6    | Prais F/t 两条已复现路径                                           |
| yssbi.statistics.panel.did.randomization | 5 → 6    | 既有用例实际命中上批 OLS 新 guard，再得到 Kernel ScientificFailure |

DID randomization 的 Runtime 断言精确得到 `OLS coefficient t-statistic is undefined`，
这是新行为触达证据，没有回退 SCI 冒称取得该路径旧红。TWFE DID 的默认 PanelOptions
实际是 FixedEffects/TwoWay/constant=true/cluster，当前零响应先被既有 Wald 校验拒绝；
因此保留 revision 5，不从调用 OLS 这一事实推定已证行为变化。Summary/Predict 没有重拟合，
版本保持原值。IV/DID 的 JSON 拟合包装仅列为待核实边界，没有未经证明套用此前 Panel 反例。

**输入适配与目录 owner。** 主代理全文复核 Database IO 三文件（lib/csv/excel）、Arrow
七文件（lib/schema/edit_type/temporal/semantic/scalar/conversion）和 ChartDocument/lib，
共十一生产文件、3,383 行。原生 Arrow 批次、投影、类别字典及精度/时区转换仍归原适配层，
Chart 文档继续共用 ResourceName 和唯一当前 schema；本段未发现确定需要修改的职责边界。
不是因为未修改源码就推定所有输入与平台行为已通过验收。

Catalog 本批全文复核十一生产文件：根 lib/builtin/catalog_entry/project/localization，
以及 core_nodes 的 mod/debug/math/reroute/support/value。support::leaf 只是同签名转调
builtin::leaf，现用直接 re-export 删除无额外职责的包装；14 个既有 Catalog 库测试与
Clippy 通过。README 将 item_analysis 已接通 SCI、返回报告/题项表/观测分组的事实同步，
删除陈旧的“尚未接入执行内核”描述。未将这个局部清单称为全 Catalog 全文覆盖。

**源码覆盖与后续入口。** Kernel 从上一批明确的 41/77 补齐剩余 36 个生产文件，当前
77/77 均有全文证据；本批没有增加 Kernel 生产文件。首先补齐 regression_models 及 budget、
time_series/forecast、causal 及 causal/models、visualization 及 overview、power、
psychometrics、quality 十文件；再补齐 survey 两、doe 四及 decision 十、meta 五、path 五。
前三组适配分别持有权重/概率转换、设计生成与预算、各方法的输入和输出投影；算法仍归 SCI。
SCI Runtime 的 doe/survey/decision/meta/path 直接转发文件也已全文核对。

上述后续文件的完整相对路径以 `builtins/statistics/` 为根：

- survey：mod.rs、weights.rs。
- doe：mod.rs、design.rs、models.rs、range.rs。
- decision：mod.rs、columns.rs、ranking.rs、systems.rs、preferences.rs、market.rs、matrices.rs、fuzzy.rs、experts.rs、conjoint.rs。
- meta：mod.rs、registration.rs、effects.rs、analysis.rs、output.rs。
- path：mod.rs、moderation.rs、mediation.rs、recursive.rs、reports.rs。

最后二十文件合计 1,745 行。Path 的 json!→value 路径只有静态线索，现有 SCI 模型已检查
系数、协方差、RSS、R² 和区间，且不含此前 Panel 的无限 F 字段；没有已证的合法输入绕过
有限值校验，不将其当作同一缺陷批量修改。

另补齐 Runtime diagnostics/mod、diagnostics/residual、report_display 三文件的明确全文证据；
serial_correlation 等历史已读文件只计复核。SCI WLS/GLS/Prais 三文件本批全文重读，
其他算法与消费者片段不扩计为整包覆盖。源码阅读完成只核销阅读清单，不能替代行为验证。

下一循环明确候选是公开 Runtime/SCI 的 SerialTestsInput：breusch_godfrey 按首行宽度
分配设计矩阵，却没有验证其余行宽，末行较长可能越界。当前只有完整源码与调用证据，
尚未运行红例或修改。Kernel 的现有构造保持等宽，Application/IPC 没有对应输入反序列化
入口，不能把它描述为已证 GUI 故障；应在 SCI 原准入边界先复现，再考虑沿 Option 返回失败。
其余 SCI 算法、Catalog 声明、跨 owner 并发及原生桌面/真实监听/Julia 验收继续按总表推进。

本批没有增加规则例外或 benchmark，没有宣称未测量的性能加速。整体 goal 保持 active。

**本批最终 L2 验证。** 冻结源码后实际通过 64 个不同 Rust 用例：SCI/Runtime 八、Kernel
十三、Catalog 十四、Harness/Core 与实际 Application 消费者二十九。没有 ignored，
没有把复跑或零匹配累计进去。App `harness::` 实际匹配七项，包含三项新增、两项现有会话
及两项已有 gateway；Core host 十项含并发取消/暂停/恢复，orchestration 六项验证真实调用。
App 的四个统计/绘图消费者及能力身份、全部非 deferred 目录装配各一项通过。
目录装配最初误选 numeric_execution target 得到零匹配，随后定位实际 `--lib` 用例并取得
一项通过；零匹配不是验收。

```powershell
pnpm test:rs:package -p yss-sci-runtime --lib regression::tests
pnpm test:rs:package -p yss-sci --test weighted_statistics
pnpm test:rs:package -p yss-sci --lib regression::linear::prais::tests
pnpm test:rs:package -p yss-node-kernel --lib did_randomization_node_is_reproducible_and_reports_valid_permutations
pnpm test:rs:package -p yss-node-kernel --lib builtins::statistics::regression_models::tests::
pnpm test:rs:package -p yss-node-kernel --lib builtins::statistics::tests::causal_models::
pnpm test:rs:package -p yss-node-kernel --lib builtins::statistics::tests::time_series::
pnpm test:rs:package -p yss-node-kernel --lib builtins::visualization::tests::
pnpm test:rs:package -p yss-node-kernel --lib weighted_diagnostics_and_cluster_covariance_use_fitted_observations
pnpm test:rs:package -p yss-node-kernel --lib binary_and_prais_nodes_honor_options_and_predict_without_refitting
pnpm test:rs:package -p yss-node-catalog --lib
pnpm test:rs:package -p yss-application --lib harness::
pnpm test:rs:package -p yss-harness-core --lib host::
pnpm test:rs:package -p yss-harness-core --lib orchestration::tests
pnpm test:rs:package -p yss-application --test numeric_execution regression_category_nodes_execute_catalog_defaults_and_conditional_parameters
pnpm test:rs:package -p yss-application --test numeric_execution causal_category_all_thirteen_nodes_execute_catalog_defaults_and_typed_effect_connections
pnpm test:rs:package -p yss-application --test numeric_execution time_series_category_defaults_execute_relational_inputs_and_publish_reports_or_plots
pnpm test:rs:package -p yss-application --test numeric_execution visualization_nodes_resolve_execute_and_keep_plot_output_categories
pnpm test:rs:package -p yss-application --lib numeric_extension_uses_actual_capabilities_and_rejects_old_artifacts
pnpm test:rs:package -p yss-application --lib all_non_deferred_builtin_nodes_are_available_in_catalog_and_sidebar
pnpm lint:rs:package -p yss-sci -p yss-sci-runtime --lib
pnpm lint:rs:package -p yss-node-kernel --lib
pnpm lint:rs:package -p yss-node-catalog --lib
pnpm lint:rs:package -p yss-harness-core -p yss-application --lib
```

四组 Clippy 均退出零；SCI 仍有 categorical 两、nonparametric 一、sample_mean 一共四条
既有 warning，未将结果称为全仓无警告。22 个改动 Rust 文件的定向 rustfmt 检查均通过，
重复依赖验证复用本批冻结后的结果。没有执行完整 workspace CI、桌面构建或真实模型/UI 验收。

文档契约六项通过（包含模块索引），本文与五份改动 README 共六路径的 oxfmt 格式检查
通过，全局 `git -c core.safecrlf=false diff --check` 退出零。补入这一收尾记录后再次检查
本文格式、文档契约及最终差异。未运行无关前端行为测试；本批没有 TypeScript 源码变化。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src-tauri/crates/yss-sci/README.md src-tauri/crates/yss-node-kernel/README.md src-tauri/crates/yss-harness-core/README.md src-tauri/crates/yss-application/README.md src-tauri/crates/yss-node-catalog/README.md
git -c core.safecrlf=false diff --check
```

## 2026-10-03：报告类型、DID typed 边界与 Julia worker 回收

本批继续使用三个子代理并行处理 SCI、Catalog 与 Runtime，再将空闲代理用于 Julia native
生命周期和交叉审查。写入按模块分工，Cargo 按依赖顺序使用同一窗口；主代理负责模型校验复用、
消费者核对和本文。源码全文核销与行为验证分开记录，整体 goal 仍 active。

**SCI 的公开形状边界。** Runtime 公开的 SerialTestsInput 可以携带不等宽 exog。
SCI 的 BG 实现此前只用首行宽度分配矩阵，两种 nomiss0 分支分别实测出现
len=8/index=8 与 len=6/index=6 越界。现只在原 breusch_godfrey 准入条件中增加逐行等宽检查，
沿用 BG 不可用的 None；Runtime 仍可返回 DW 与 Ljung–Box。扩写既有 golden 用例同时验证两种
模式，没有新增测试函数。现有 Kernel 两入口分别传 None 或从同一设计列集构造等宽行，
不触达新增拒绝路径，因此没有递增相关 Kernel revision，也不将公开 API 缺陷描述为已复现的 GUI 故障。

**报告与模型的名义类型。** Catalog 曾将 IV 2SLS/LIML、Panel、VAR、VEC Summary，
Panel Compare 与 VAR Lag-order Selection 共七个报告输出标为对应拟合模型类型。
Analysis 依据名义 TypeId 相交，无法辨别报告中缺少 PanelFit/VarFit 等模型字段；执行时才会
解码失败。扩写既有 Analysis 连线用例实际得到七组 compatible=true/mismatch=false，
修正 Catalog 的五个 family 分支后，七个报告输出均使用已有 statistics.report；
错误连线得到定位到 connection 的 TypeConnectionMismatch，对应 Fit 连线仍兼容。
现有 Catalog Summary 用例和真实 Application Panel/VAR 报告消费也已验证。
Registry 指纹原本包含 ports.valueType，自然使旧声明失效，无新增类型、生成 schema 或 Kernel revision。

本批还删除 DataFrame SeriesSum 为两个相同 core.numeric 构造并归一化 Union 的两个私有 helper，
直接复用同文件已有 concrete 构造。归一化后的公开类型保持不变，不新增测试或宣称性能加速。

**DID 的中间表示。** Runtime fit_did 原先把 SCI 的 PanelFit 编成 JSON，唯一 Kernel
生产调用紧接着反序列化成同一类型。现直接重导出 SCI typed 入口，报告组合仍在原 Kernel，
在第一次 JSON 构造前复用 common::validate_finite。无消费者的 Runtime compute_fake_group_ri
转发删除，SCI 原实现与其测试保留；有执行控制的 randomization_test 直接重导出。
没有改变 TwoWay/cluster 数值算法或新增状态/schema。

当前真实 TWFE 无噪声 fixture 在旧、新实现都通过，不能称为 Inf 旧红。有限输出边界的
潜在非有限模型会使用既有 NonFiniteResult，故 TWFE 节点 revision 从 5 递增到 6；
这一非有限分支未取得实际数值复现，不把静态推断写成行为证据。既有 Kernel 用例实际包含 TWFE，
Application 原因果用例新增第十四类 TWFE 关系输入、ATT 系数及报告投影验证，去掉函数名中
过时的 thirteen。Runtime 根注释与 Kernel README 的旧 IPC/结果分析直接调用描述也按实际依赖修正。

**Julia worker 临时资源。** Extension 整个进程共用一个 adapter。成功结果复制到
bayes-results 后，adapter 原 tasks 表仍持有 CompletedTaskState 和原 worker 目录，
只能等 adapter 析构回收。扩写既有 Runtime 成功用例，使用受控 worker、真实临时文件及
实际队列/复制流程：终态 Completed、复制结果可读、队列已排空时，旧源目录仍存在，取得实际旧红。

现有 Worker Port/Client 增加一次真实的 release 边界；Runtime 保存已接纳句柄，在原
finish_worker_task 返回后的同一队列尾部释放。成功复制、worker 失败、取消与 materialize
失败的正常返回都经过该尾部。Adapter 在原锁内移出精确句柄，只移除匹配代际的 current，
锁外取消未完成工作并通过原 RAII 回收目录。迟到的 await 错误仅能更新仍匹配的 current，
原成功回执已有同等检查；没有第二份任务账本、永久墓碑或通用回收框架。

Root 与另一代理的交叉审查发现新增 release 可能与 Runtime::cancel 竞争：先设置取消 token
的队列已结算并释放句柄，迟到的 worker.cancel 将得到 StaleTaskHandle。修复仅在错误携带
本次准确句柄、原任务已终态、原 worker_handles 已移除时接纳该回执；其他取消错误继续返回。
唯一新增纯 Runtime 回归通过 Condvar 强制先释放再返回 Stale，兼顾取消早退尾部和这一竞争；
其余成功、失败与 adapter 旧代/后继隔离复用既有用例。等待队列排空后才检查目录，不把终态发布
与随后释放之间的调度间隙当成失败。上述是实际 Rust owner 与受控 port 验证，不是 Julia 进程端到端验收。

**模型校验复用。** Bayes draft 与 immutable-spec 校验各持一份完全相同的表达式符号遍历。
现扩大原 spec_validation 私有函数至父模块可见，让 draft validator 直接复用，删除重复实现；
模型、校验报告与数据绑定契约保持不变，没有新增测试函数或公开 API。

**源码全文清单。** SCI Runtime 当前 42 个 src Rust 文件中两个是纯测试文件，生产文件为
40/40。历史二十一份复用，本批补齐十九份：lib、anova、association、distribution、descriptive、
inference、longitudinal、multivariate、power、psychometrics、quality、spatial、survival、
visualization、causal/mod、causal/did、regression/models、regression/postestimation、
time_series/acf_pacf。这里的 postestimation 是 Runtime 入口，不与此前 SCI evaluation 的阅读混计。

Catalog 当前 69/69 个生产 Rust 文件全文已核销，含 statistics 四十九、其他二十。
复用上批 Root 十一份根模块/core_nodes，以及 dataframe/aggregation、distribution/mod。
本批先读 statistics 的 mod、families、ports、inventory、inventory/entries、analyses、
analyses/models、causal_models、panel_models、regression_models、time_series、spatial、
survival、longitudinal 十四份。续读四十二份如下，其中 dataframe/families 为消除旧报告
families 缩写歧义而重读，不能重复增加累计文件数。

| Catalog 范围 | 本批续读的完整相对路径（前缀 src-tauri/crates/yss-node-catalog/src/）                                                                                                                                                                                                                              |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 检验与推断   | statistics/classical.rs、statistics/association.rs、statistics/anova.rs、statistics/inference.rs                                                                                                                                                                                                   |
| 描述与多元   | statistics/descriptive.rs、statistics/multivariate.rs                                                                                                                                                                                                                                              |
| Decision     | statistics/decision/mod.rs、statistics/decision/parameters.rs、statistics/decision/ranking.rs、statistics/decision/systems.rs、statistics/decision/preferences.rs、statistics/decision/matrices.rs、statistics/decision/market.rs、statistics/decision/experts.rs、statistics/decision/conjoint.rs |
| Meta         | statistics/meta/mod.rs、statistics/meta/parameters.rs、statistics/meta/interface.rs                                                                                                                                                                                                                |
| Path         | statistics/path/mod.rs、statistics/path/parameters.rs、statistics/path/moderation.rs、statistics/path/mediation.rs、statistics/path/recursive.rs                                                                                                                                                   |
| DOE          | statistics/doe/mod.rs、statistics/doe/design.rs、statistics/doe/models.rs、statistics/doe/range.rs                                                                                                                                                                                                 |
| Survey       | statistics/survey/mod.rs、statistics/survey/parameters.rs                                                                                                                                                                                                                                          |
| Power        | statistics/power/mod.rs、statistics/power/parameters.rs、statistics/power/parameters/labels.rs                                                                                                                                                                                                     |
| 其他统计     | statistics/plot_overview.rs、statistics/quality.rs、statistics/psychometrics.rs                                                                                                                                                                                                                    |
| DataFrame    | dataframe/mod.rs、dataframe/transforms.rs、dataframe/labels.rs、dataframe/inventory.rs、dataframe/families.rs                                                                                                                                                                                      |
| 文档与绘图   | documentation.rs、plot/mod.rs                                                                                                                                                                                                                                                                      |

Julia native 当前 32/32 个含生产代码的 src Rust 文件全文已核对，两个独立 tests.rs 与
集成测试不计生产数。主代理十九份与子代理十三份互不重叠；expression.rs 含历史复读，
不将全部三十二份都称为本批新增覆盖。

| Native crate                  | 已全文阅读的 src 文件                                                                                   |
| ----------------------------- | ------------------------------------------------------------------------------------------------------- |
| yss-julia-extension           | main.rs、inputs.rs、commands.rs                                                                         |
| yss-bayes-runtime             | lib.rs                                                                                                  |
| yss-bayes-model               | lib.rs、model.rs、draft.rs、convert.rs、expression.rs、validation.rs、spec_validation.rs、validators.rs |
| yss-bayes-result              | lib.rs、result.rs、diagnostics.rs                                                                       |
| yss-bayes-artifact-contract   | lib.rs                                                                                                  |
| yss-bayes-artifact-datafusion | lib.rs、rows.rs、plots.rs                                                                               |
| yss-bayes-worker              | lib.rs、input.rs、control.rs、validation.rs                                                             |
| yss-bayes-worker-julia        | lib.rs、fit.rs、predictor.rs                                                                            |
| yss-julia-worker              | lib.rs、task_directory.rs、preparation.rs、error.rs、assets.rs                                          |
| yss-julia-runtime             | lib.rs                                                                                                  |

宿主 snapshot 的读取失败清理也沿实际调用链核对：monitor_task 的失败尾部调用
HostServices::release_context，Application 按 context 移除租约并释放路径，所以没有另加一套
插件快照清理状态。Native 模型与输入归插件、宿主仍拥有数据授权和结果提交，未引入宿主 SCI、
项目或数据库依赖。Catalog 的声明、Analysis 类型事实和 SCI 数值算法继续由原 owner 分工。

全部调整沿用默认所有权、typed 边界和原能力，没有新增需用 benchmark 反驳规则的例外。
本批未改 TypeScript，也未新增 UI 单元测试；源码阅读完成不替代未运行的真实 Julia、
桌面交互与原生监听验收。SCI 其余算法及跨 owner 行为继续按总表推进，整体尚未完成。

本批还完整读取 Julia 的 worker.jl，未据此扩称其余 Julia 脚本和插件网页已全读。
Bayes 物化期间取消又遇到读取失败时的错误分类仅有静态候选，尚未取得实际回归，继续保留为
后续核对入口；本批已实测的是 worker 成功/失败/取消的资源释放与旧句柄隔离。

**本批验证。** 最终不同 Rust 用例合计六十五项：SCI/Runtime serial 与 Kernel 消费者二项，
Catalog/Analysis/Application 十七项，DID 三项，Julia native 与模型四十三项
（Model 二十、Runtime 十一、Worker 八、JuliaAdapter 四）。均无 ignored；重跑的 Catalog
十四项只计一次。DID 旧 fixture 绿灯不计为旧红，Extension Clippy 只计实际组合编译。

```powershell
pnpm test:rs:package -p yss-sci-runtime --test diagnostics_serial_correlation_golden
pnpm test:rs:package -p yss-node-kernel --lib weighted_diagnostics_and_cluster_covariance_use_fitted_observations
pnpm test:rs:package -p yss-graph-analysis --lib connection_compatibility_preserves_union_choices_and_generic_container_shapes
pnpm test:rs:package -p yss-node-catalog --lib
pnpm test:rs:package -p yss-application --test numeric_execution panel_category_nodes_execute_defaults_and_connect_to_existing_summary
pnpm test:rs:package -p yss-application --test numeric_execution migrated_var_reports_preserve_real_relation_column_labels
pnpm test:rs:package -p yss-sci-runtime --test causal_operations
pnpm test:rs:package -p yss-node-kernel --lib did_randomization_node_is_reproducible_and_reports_valid_permutations
pnpm test:rs:package -p yss-application --test numeric_execution causal_category_nodes_execute_catalog_defaults_and_typed_effect_connections
pnpm test:rs:package -p yss-bayes-worker -p yss-bayes-worker-julia -p yss-bayes-runtime -p yss-bayes-model --lib
pnpm lint:rs:package -p yss-sci -p yss-sci-runtime --lib
pnpm lint:rs:package -p yss-node-catalog -p yss-graph-analysis --lib
pnpm lint:rs:package -p yss-sci-runtime -p yss-node-kernel --lib
pnpm lint:rs:package -p yss-bayes-worker -p yss-bayes-worker-julia -p yss-bayes-runtime -p yss-bayes-model --lib
pnpm lint:rs:package -p yss-julia-extension --bin yss-julia-extension
pnpm lint:rs:package -p yss-node-catalog --lib
```

上述 Clippy 都退出零，SCI 依然有 categorical 两项、nonparametric 一项、sample_mean 一项
既有 warning；Catalog/Analysis 与 native 检查没有 warning。最后的 Catalog 检查针对删除
冗余 helper 后的源码版本；类型输出等价，复用本批未变的 Analysis/Application 消费者证据。
十八份本批改动 Rust 文件的定向 rustfmt 均通过，未做全仓格式化。

Julia 原目录留存的 Runtime 旧红为一失败、九过滤，编译 4.74 秒、测试 0.01 秒；
回收引入的取消竞争随后用确定性释放 gate 实测一失败、十过滤，错误为 CancelFailed/
bayes_worker_stale_task，再增加精确结算重验修绿。最终四包编译 15.56 秒、四十三项通过，
Extension Clippy 实际组合编译 1 分 17 秒；这些时间是验证成本，不是公平性能 benchmark。
本批仅新增一个纯 Runtime 测试函数，其他都是既有用例扩写；未执行完整 CI、桌面构建、
插件重打包或真实 Julia 计算链，未提交 Git。

03:17 的文档契约六项通过（包含模块索引），本文与七份 README 共八路径的 oxfmt 检查
通过，全局 `git -c core.safecrlf=false diff --check` 退出零。补入本段后再复查本文格式、
文档契约及最后的全局差异；其他未变验证沿用本批结果。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src-tauri/crates/yss-sci/README.md src-tauri/crates/yss-node-catalog/README.md src-tauri/crates/yss-sci-runtime/README.md src-tauri/crates/yss-node-kernel/README.md plugins/julia/native/crates/yss-bayes-model/README.md plugins/julia/native/crates/yss-bayes-worker/README.md plugins/julia/README.md
git -c core.safecrlf=false diff --check
```

## 2026-10-03：IV 模型边界、Julia 取消结算与 SCI 内部复用

本批继续由三个子代理并行工作，主代理负责消费边界复核、宿主/插件错误文案和唯一报告。
写入按文件 owner 分配；Cargo 依依赖顺序使用同一窗口，上游源码在消费者验证期间冻结。
Rust 检查同时推进 TypeScript 检查和互不冲突的源码阅读，不重跑未变化的上批检查。

**IV 共享模型直接传递。** SCI 已返回 `InstrumentalVariableFit`，原 Runtime 仍先转成
JSON，再由 Kernel 写入标签并经过输出转换。现 Runtime 直接重导出 SCI 入口，Kernel
在共享类型上恢复 response_name、parameter_names 和 instrument_names，然后交给原
common::value。Summary 从存储的运行值解码模型仍是实际边界，保留原路径，没有新模型、
兼容层、状态或数值算法。

原 JSON 转换会丢掉非有限数值的信息，现在既有有限值校验先于编码运行，因此两个
IV Fit 的实现 revision 从 5 递增至 6。此非有限分支尚无真实数值旧红，不能把普通模型
等价测试写成 Inf 修复证据。既有 Kernel 用例验证两种估计器、完整模型标签及 Fit→Summary，
Application 既有因果用例增加 2SLS/LIML 的真实关系输入、列名、第一阶段及报告链。
实际运行两项不同用例，不新增测试函数。

**Julia 取消与物化失败。** finish_worker_task 在读取产物前拍过取消快照，但产物读取
期间新接纳的取消可能被后续 failed_task 覆盖。Extension 原样投影 Failed/Cancelled，
宿主接纳该远端终态，不能代为修正分类。现仅在原状态锁内结算物化错误时重验
Cancelling/Cancelled：已取消则使用原 cancelled_task，否则保留原失败来源。
部分产物清理、句柄和 source 移除、统一 worker release 尾部保持原 owner。

新增一个纯 Runtime 回归，用第二份产物的读取 gate 固定顺序，先验证未取消失败，再验证
第一份文件已复制→接纳取消→第二份读取失败。旧实现实际得到 Failed 而非 Cancelled；
修复后两阶段均通过，并等待队列排空后验证复制目录和 worker 源目录均回收。
没有启动 Julia 进程或用终态发布与随后清理之间的调度间隙充当回归。

**删除失效的数据加载接口。** 全仓构造点核对表明 BayesDatasetLoadError、
DatasetSourceUnsupported、DatasetLoadFailed 已无生产者。快照读取归 Extension，
Runtime 只接收已验证 StatisticalInput，故删除这些错误类型及 Display/source 分支、
Extension 的无效映射，以及宿主和插件两种语言中的两项旧错误码。
实际快照读取失败仍走 plugin_snapshot_invalid；没有改变该路径的语义或建立兼容翻译。
Extension 的真实组合 Clippy 与两个 TypeScript 工程检查均通过，没有新增翻译或 UI 测试。

**SCI 的内部重复计算。** KDE 公开入口已经过滤非有限输入，唯一生产调用的私有
silverman_bandwidth 却再次过滤并分配 Vec。现直接借用已有切片，保持计算顺序和原回退值，
将既有非有限输入断言移到公开入口，比较有限/混合输入的完整密度输出。
distribution 原私有 check 与 ScientificExecutionControl::check 的取消优先、期限比较和
错误完全一致，现十一处直接复用原执行控制 owner。两项都不改变 API、算法或输出契约，
不增加测试函数、revision 或 README 约定，也不据代码减少宣称已测得加速。

**SCI 全文覆盖核销。** 当前 src 有 215 份 Rust 文件，其中八份独立 tests.rs 均由父模块
在 cfg(test) 下引入；生产实现共 207 份。历史明确路径三十八份，本批新增五十六份，
累计 94/207，尚余 113 份。Kernel 77/77、Catalog 69/69、Runtime 40/40、Julia native 32/32
保持上批去重范围，消费者片段、重复阅读及测试文件不再增加生产覆盖。

下列路径统一相对于 src-tauri/crates/yss-sci/src/。历史三十八份按目录核销；其中
diagnostics/serial_correlation.rs 由上批实际全文的代理补证，不重复算本批新增。

| 历史目录                  | 已有全文证据的文件                                                                                |
| ------------------------- | ------------------------------------------------------------------------------------------------- |
| hypothesis                | `sample_mean.rs`、`categorical.rs`、`nonparametric.rs`、`variance.rs`、`linear_test.rs`           |
| 根                        | `error.rs`                                                                                        |
| regression                | `design.rs`                                                                                       |
| causal/iv                 | `fit.rs`                                                                                          |
| regression/linear         | `fit.rs`、`mod.rs`、`prais.rs`、`wls.rs`、`gls.rs`                                                |
| regression/linear/ols     | `mod.rs`、`fit.rs`、`inference.rs`                                                                |
| regression/discrete       | `mod.rs`、`fit.rs`、`logit.rs`、`probit.rs`、`postestimation.rs`                                  |
| regression/postestimation | `evaluation.rs`                                                                                   |
| time_series               | `models.rs`、`var.rs`、`vec.rs`、`vec_vecrank_cv.rs`                                              |
| time_series/var           | `types.rs`、`estimate.rs`、`varsoc.rs`、`postestimation.rs`                                       |
| time_series/vec           | `types.rs`、`stage.rs`、`estimate.rs`、`vecrank.rs`、`stats.rs`、`linalg.rs`、`postestimation.rs` |
| diagnostics               | `serial_correlation.rs`                                                                           |

本批新增的五十六份如下：

| 范围                     | 新增全文文件                                                                                                                                                                                                                                                                                                                                                     |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 基础与共享计算（15）     | `lib.rs`、`hypothesis/mod.rs`、`regression/mod.rs`、`diagnostics/mod.rs`、`time_series/mod.rs`、`hypothesis/t_test.rs`、`hypothesis/wald_test.rs`、`hypothesis/linear_hypothesis.rs`、`density.rs`、`distribution.rs`、`descriptive.rs`、`descriptive/quantiles.rs`、`regression/collinearity.rs`、`regression/covariance.rs`、`time_series/distributions.rs`    |
| IV 模块/估计（3）        | `causal/iv/mod.rs`、`causal/iv/ivliml.rs`、`causal/iv/iv2sls/fit.rs`                                                                                                                                                                                                                                                                                             |
| 因果与 IV 后续计算（13） | `causal/common.rs`、`causal/designs.rs`、`causal/did.rs`、`causal/econometrics.rs`、`causal/mod.rs`、`causal/treatment.rs`、`causal/iv/estimate.rs`、`causal/iv/iv2sls/critical_values.rs`、`causal/iv/iv2sls/design.rs`、`causal/iv/iv2sls/first_stage.rs`、`causal/iv/iv2sls/mod.rs`、`causal/iv/iv2sls/postestimation.rs`、`causal/iv/iv2sls/types.rs`        |
| 残差与诊断（13）         | `diagnostics/breusch_pagan.rs`、`diagnostics/comparison.rs`、`diagnostics/design.rs`、`diagnostics/im_test.rs`、`diagnostics/influence.rs`、`diagnostics/leverage.rs`、`diagnostics/normality.rs`、`diagnostics/reclassification.rs`、`diagnostics/reset.rs`、`diagnostics/residual.rs`、`diagnostics/vif.rs`、`diagnostics/weighted.rs`、`diagnostics/white.rs` |
| ANOVA、关联、推断（12）  | `anova/design.rs`、`anova/mod.rs`、`anova/multivariate.rs`、`anova/repeated.rs`、`association/agreement.rs`、`association/correlation.rs`、`association/mod.rs`、`association/ridit.rs`、`inference/cluster.rs`、`inference/comparisons.rs`、`inference/intervals.rs`、`inference/mod.rs`                                                                        |

尚未全文核销的 113 份按一级领域为：decision 14、doe 6、longitudinal 4、meta 6、multivariate 7、panel 15、path 8、power 5、psychometrics 4、quality 5、regression 10、spatial 4、survey 5、survival 6、time_series 9、visualization 5。
这是阅读清单，不是这些文件不符合规则的判定；源码全文也不替代数值、跨 owner 行为或原生验收。

ANOVA 继续共用设计、秩和输入校验；关联分析保留配对与独立样本的区别；推断复用
OLS、系数表和分位数。Kernel 使用同一 design_columns 准入、实际 Kappa 类别平方预算，
Ridit 使用独立列读取。残差载体调用各自 BP/White/IM/RESET/VIF/Leverage 实现，模型比较
重验响应、权重及列空间嵌套，影响诊断借用拟合与 precision weights。没有为文件长度另建
状态 owner 或空抽象；本次阅读不是全面统计公式、临界值表或取消延迟的独立验算。

**继续保留的具体入口。** 以下只有当前源码/调用链证据，尚无本批数值旧红，未计作修复：

- IV 第一阶段的多内生变量分支按列填 Vec 后按 row-major 构造矩阵；三个及以上内生变量
  时剩余两列以上，公开 Fit 和 Kernel 动态输入无两列上限。下一步用非退化三列样本和
  既有 golden 确认实际排列及 Shea 结果，不能以只覆盖一至两列的现有测试消除此项。
- Runtime IV Summary 已将 typed 第一阶段结果编码为 JSON，后续又读标签、系数并
  as_f64().unwrap_or(0.0) 构造展示。可在现有报告 owner 使用 typed 字段，不应新建模型；
  本批 Fit 的 typed 修复不涵盖新计算的第一阶段、overidentification/endogeneity 输出。
- 自动 Newey–West 带宽把最后一列视为常量并置零权重，实际 regression/design 的截距
  放第一列，无截距时也无固定常量位置。OLS→HAC 自动带宽调用已追踪，仍需数值差异验证。
- 公开 HypothesisTestInput 的有限负协方差可经过现有有限值准入；raw t-test 开方后
  只判 se<=0，尚需确认 NaN 的实际错误边界。raw t/Wald 只被校验 wrapper 消费，可评估
  收窄可见性；它们不是第二份算法。
- 无截距模型可从 Kernel→Runtime residual 进入假设首列为常量的 White/IM 路径，
  仍需明确 no-intercept fixture 与预期统计量。White/IM 的同矩阵重复 SVD、部分只读
  to_owned，以及 MANOVA 完整模型分支的 error.clone 也待局部借用/分解复用核对。
  DID 随机置换的可变设计副本有隔离用途，保留。

**本批验证。** 不同 Rust 用例共二十一项：Bayes Runtime 十二，IV 的 Kernel/Application
两项，密度/分布及可视化消费者七项（SCI 五、Kernel 一、Application 一）。Runtime 定向重跑
不重复计数，所有用例均无 ignored；只有取消与产物读取失败这一新测试函数，其他为既有用例。
两个 TypeScript 工程及四份 locale 的定向 lint/格式检查通过，没有新增 UI 测试。

取消误分类旧红为零通过、一失败、十一过滤（编译 0.94 秒、执行 0.02 秒），修复后定向一项通过；
最终 Runtime 十二项通过，编译 0.16 秒、执行 0.03 秒。IV 消费者编译分别 4.88 秒、28.42 秒，
执行 0.09 秒、3.09 秒；分布 Application 编译 17.34 秒、执行 3.64 秒。这些是验证成本，
没有充当公平性能 benchmark。所有调整使用默认 owner 和边界，没有提出新的规则例外。

```powershell
pnpm test:rs:package -p yss-bayes-runtime --lib artifact_read_failure_preserves_an_accepted_cancellation
pnpm test:rs:package -p yss-bayes-runtime --lib
pnpm test:rs:package -p yss-node-kernel --lib iv_nodes_accept_multiple_instruments_and_preserve_identification_results
pnpm test:rs:package -p yss-application --test numeric_execution causal_category_nodes_execute_catalog_defaults_and_typed_effect_connections
pnpm test:rs:package -p yss-sci --lib density::tests::
pnpm test:rs:package -p yss-sci --lib distribution::tests::sampling_preserves_parameter_conventions_integer_edges_and_control
pnpm test:rs:package -p yss-sci --lib visualization::tests::gaussian_density_has_finite_grid_and_approximately_unit_mass
pnpm test:rs:package -p yss-node-kernel --lib every_visualization_kernel_executes_its_declared_input_layout_and_plot_carrier
pnpm test:rs:package -p yss-application --test numeric_execution probability_distribution_catalog_executes_all_defaults_and_rejects_invalid_or_unbounded_requests
pnpm lint:rs:package -p yss-bayes-runtime --lib
pnpm lint:rs:package -p yss-julia-extension --bin yss-julia-extension
pnpm lint:rs:package -p yss-sci-runtime -p yss-node-kernel --lib
pnpm lint:rs:package -p yss-sci -p yss-node-kernel --lib
pnpm check:ts
pnpm check:plugin:julia
pnpm exec oxlint src/app/i18n/locales/en-US.ts src/app/i18n/locales/zh-CN.ts plugins/julia/web/src/app/i18n/locales/en-US.ts plugins/julia/web/src/app/i18n/locales/zh-CN.ts
pnpm format:check:ts src/app/i18n/locales/en-US.ts src/app/i18n/locales/zh-CN.ts plugins/julia/web/src/app/i18n/locales/en-US.ts plugins/julia/web/src/app/i18n/locales/zh-CN.ts
```

四条 Clippy 都退出零；Runtime/Extension 无 warning，SCI 保留原 categorical 两项、
nonparametric 一项、sample_mean 一项共四条 warning。本批九份 Rust 文件已由各 owner
显式 rustfmt --edition 2024 --config skip_children=true --check 验证；局部 diff 检查通过。
未运行完整 CI、桌面构建、Julia 进程端到端、插件打包或 UI 人工验收，未提交 Git。
整体 goal 继续 active，后续沿上述剩余清单和有调用依据的候选推进。

03:39 的文档契约六项通过（含模块索引）；本文和三份 README 的定向 oxfmt 检查通过。
补入本段后复查文档与格式，交付前最后执行全局 diff 检查：

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src-tauri/crates/yss-sci-runtime/README.md src-tauri/crates/yss-node-kernel/README.md plugins/julia/README.md
git -c core.safecrlf=false diff --check
```

## 2026-10-03：SCI 全文覆盖、数值边界与 Julia 取消投影

本批按用户授权由三名子代理与 Root 分工。文件按 owner 分配，SCI/Cargo 修改和检查使用
依次交接的窗口；前端检查与只读审查可并行。Root 统一复核改动、维护实现 revision 和本文。
没有将重复阅读、相同用例重跑或历史混合工作树中的既有改动计作本批新增。

**IV 第一阶段与报告事实源。** 三个及更多内生变量的 residualization 原来按列写平面数组，
再用 row-major 构造矩阵，使观测与列错位。仅调整循环顺序以保持行布局。既有 golden 保留
原一、二变量及 nonrobust/HC1 断言，追加正交 Fourier 样本、三个内生变量和换列顺序；
独立期望为各变量信号方差占比。旧实现首个新增 Shea 值实际为
0.8019281203686782，期望 0.8；该断言失败后的样本不算已执行旧红。修复后两种列顺序通过。
Kernel 用真实动态输入分别验证 2SLS/LIML 的模型到 Summary 消费链。

Runtime Summary 现在用已有 typed 第一阶段字段生成行、标签、方程和弱工具变量展示，
最后才编码报告；删除 JSON 编码后再读取系数并默认回退为零/空串的路径。
没有增加第二个模型或有限值校验框架；本次不宣称新计算诊断的完整有限值边界已验证。
两个 IV Summary revision 从 5 变为 6，Fit 保持上批的 6。

**ADF 使用共享校准。** 私有 MacKinnon 副本与 time_series/mackinnon.rs 的 N=1
None/Trend 校准重复，且多项式系数顺序反了。删除副本，直接调用现有共享函数。
既有 panel_category_reference.json 由仓库中的 statsmodels 0.14.6 生成器提供独立参考，
本批不重新生成参考。扩写已有 ADF 用例，保留 Drift 并核对 None/Trend 六个样本。
旧实现首个无常数样本的统计量通过，而 p 值 0.5461550654927851 不等于
0.5833868428188507；其余五个没有在该失败运行中执行。修复后六个全部通过。
Kernel 实际报告 pValue 与参考一致；Panel Fisher 的已有分布参考仍通过。
Drift 的 Student-t 约定、临界值及辅助回归不变，仅 adf.test revision 从 5 变为 6。

**Panel 的保留列映射。** 双向 RE MLE 迭代曾展开系数到原列位置，最终与 pooled
likelihood 却传紧凑系数；私有似然函数仍按原列号取值。用带噪声平衡面板，在两个自变量
之间插入重复列，保留原实体 MLE Stata golden 后，旧实现实际在最终 likelihood 越界：
len 3 / index 3。pooled 路径当时尚未执行，不能另记一项旧红。

现在迭代、最终和 pooled likelihood 均使用紧凑系数及 kept 原列映射，迭代残差也复用
同一位置约定，删除原展开缓存。系数、likelihood、LR 和 chibar² 与不含重复列的设计一致。
两个 Kernel 消费用例及 Application 实际能力/旧产物准入用例通过；panel.fit 与
panel.compare revision 从 6 变为 7。Summary/Predict 不重新估计，未改其 revision。
两份 owned Rust 文件内的既有格式漂移一并规范化，没有扩大到整仓格式化。

**SCI 内部复用。** 非排序 XY 路径直接使用既有 sample_indices，不再先分配全部行索引；
排序路径仍对完整顺序稳定排序，再取相同样本。扩写原采样用例，核对降序输入、排序/非排序
坐标、元数据和重复 x 的稳定顺序。调节分析的两处同义 min/max 计算直接调用现有
preparation::range。原 probing.rs 已是工作树未跟踪文件，本批只修改这两处及 import，
不把整个文件算作新增实现。

quantile 的 floor 计算将同一 y 的 mean 从逐行闭包提到一次计算；robust 仅在进入
低尺度分支后计算一次最大绝对响应值。保持运算和分支顺序、取消检查及模型结果，
直接复用既有独立数值 fixture 和 Application 消费者，未增加测试。上述内部复用不改变
API 或输出契约，不推进 revision，不据源码减少声称实测加速，也不申请规则例外。

**Julia 网页取消投影。** 原 task_received 仅保护终态，取消与轮询并行时，迟到的同任务
running/queued 会覆盖 cancelling。现归约器保留已知取消意图，同时接纳新的进度字段和终态。
原取消 catch 还共用 request_failed，能在结果读取/完成后把状态改为 submission_unknown。
现使用私有 cancel_failed 动作，仅在 active 时结算取消失败；generation/taskId 准入不变，
普通提交、轮询和结果读取失败保持原路径，实际结果读取失败仍可结算。

这两个竞争分别取得真实旧红：同一既有纯状态用例第一次因 running/queued 回退两断言失败，
第二次按当时真实 cancel catch 派发 request_failed，因 reading_result/completed 回退
两断言失败。均为四通过、一失败；不是新增 action 导致的编译失败。修复后原三份状态、
Service 与 wire parser 测试共十二项通过，第二次重跑不重复计数。没有新增测试函数或 UI
单元测试，也没有改变原生任务 owner、操作标识或显式重试机制。

**SCI 阅读范围闭合。** 用本批新清点与实际路径集合去重核销：src 下 215 份 Rust 中，
八份独立 cfg(test) tests.rs 不计生产范围，生产 207/207 已全文阅读，未读为零。
历史 94 加本批 113（Root 30、Graph 49、Plugin 19、Database 15）；既有 77/77 Kernel、
69/69 Catalog、40/40 Runtime、32/32 Julia native 范围不重复累计。生产全文是阅读证据，
不等同每个数值公式、边界组合或真实运行已验证。

以下本批 113 路径相对于 src-tauri/crates/yss-sci/src/：

| 目录                        | 本批全文文件                                                                                                                                                                                        |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `decision`                  | `compromise.rs`、`conjoint.rs`、`data.rs`、`experts.rs`、`fuzzy.rs`、`hierarchy.rs`、`influence.rs`、`mod.rs`、`preferences.rs`、`pricing.rs`、`ranking.rs`、`reach.rs`、`systems.rs`、`weights.rs` |
| `doe`                       | `design.rs`、`dose.rs`、`mod.rs`、`range.rs`、`surface.rs`、`uniform.rs`                                                                                                                            |
| `longitudinal`              | `gee.rs`、`glmm.rs`、`mixed.rs`、`mod.rs`                                                                                                                                                           |
| `meta`                      | `data.rs`、`diagnostics.rs`、`effects.rs`、`mod.rs`、`model.rs`、`plots.rs`                                                                                                                         |
| `multivariate`              | `adequacy.rs`、`canonical.rs`、`common.rs`、`discriminant.rs`、`factor.rs`、`mod.rs`、`ordination.rs`                                                                                               |
| `panel`                     | `data.rs`、`dynamic.rs`、`fd.rs`、`fe.rs`、`fit.rs`、`lsdv.rs`、`mod.rs`、`nonstationary.rs`、`re.rs`                                                                                               |
| `panel/re`                  | `be.rs`、`fgls.rs`、`mle.rs`、`shared.rs`、`time.rs`、`twoway.rs`                                                                                                                                   |
| `path`                      | `bootstrap.rs`、`effects.rs`、`mediation.rs`、`mod.rs`、`moderation.rs`、`preparation.rs`、`recursive.rs`                                                                                           |
| `path/moderation`           | `probing.rs`                                                                                                                                                                                        |
| `power`                     | `design.rs`、`distributions.rs`、`evaluate.rs`、`mod.rs`、`solve.rs`                                                                                                                                |
| `psychometrics`             | `content.rs`、`items.rs`、`mod.rs`、`reliability.rs`                                                                                                                                                |
| `quality`                   | `capability.rs`、`control.rs`、`gage.rs`、`mod.rs`、`process.rs`                                                                                                                                    |
| `regression/models`         | `common.rs`、`glm.rs`、`likelihood.rs`、`mod.rs`、`nonlinear.rs`、`regularized.rs`、`robust.rs`、`workflows.rs`                                                                                     |
| `regression/postestimation` | `mod.rs`、`predictions.rs`                                                                                                                                                                          |
| `spatial`                   | `mod.rs`、`moran.rs`、`regression.rs`、`weights.rs`                                                                                                                                                 |
| `survey`                    | `design.rs`、`mean.rs`、`mod.rs`、`regression.rs`、`weights.rs`                                                                                                                                     |
| `survival`                  | `common.rs`、`cox.rs`、`evaluation.rs`、`mod.rs`、`nonparametric.rs`、`parametric.rs`                                                                                                               |
| `time_series`               | `acf_pacf.rs`、`mackinnon.rs`、`unit_root.rs`                                                                                                                                                       |
| `time_series/forecast`      | `arima.rs`、`diagnostics.rs`、`elementary.rs`、`mod.rs`、`smoothing.rs`、`volatility.rs`                                                                                                            |
| `visualization`             | `categorical.rs`、`distribution.rs`、`matrix.rs`、`mod.rs`、`points.rs`                                                                                                                             |

质量分析共享 process、尺度与输入 owner；测量分析共享 analyze/moments；多变量共享
Prepared、谱分解和 adequacy；纵向模型共享设计与推断但保留不同误差模型；空间模型共享
权重、lag、设计和求解；生存模型共享数据准入、风险集和评估入口。决策、设计、路径、
效能和调查的模型准备与结果投影保持各自职责，调用现有线性代数与执行控制。
没有因文件长而再建 manager、通用校验器或平行可写结果；排序、置换和可变模型的必要副本
不因出现 clone 就判作违规。以上结论仅覆盖实际源码职责与调用关系。

**Julia Web 阅读范围。** 当前 src 94 份 TS/TSX 中，13 份既有测试和一份 vite-env 声明
不计生产范围，80/80 个生产文件（含两份 locale payload）全文读完。另读 Vite/TypeScript
配置、manifest 及当前 Plugin/Protocol/Runtime 契约；宿主与 Rust 消费片段未记为新的
生产全文覆盖。locale 文件读取不等同逐条翻译质量验收。路径相对于 plugins/julia/web/src/：

| 目录                                         | 本批全文文件                                                                                                                                                                                                |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `根`                                         | `main.tsx`、`sdk.ts`                                                                                                                                                                                        |
| `app/i18n/locales`                           | `en-US.ts`、`zh-CN.ts`                                                                                                                                                                                      |
| `components/ui`                              | `alert.tsx`、`button.tsx`、`card.tsx`、`dialog.tsx`、`input.tsx`、`label.tsx`、`progress.tsx`、`scroll-area.tsx`、`select.tsx`、`table.tsx`、`tabs.tsx`、`tooltip.tsx`                                      |
| `features/application`                       | `errorReference.ts`                                                                                                                                                                                         |
| `features/application/bayes`                 | `bayesActions.ts`、`bayesError.ts`、`formulaParsing.ts`、`index.ts`、`useBayesArtifacts.ts`、`useBayesDatasets.ts`、`useBayesInferenceTask.ts`、`useBayesModelDraft.ts`、`useBayesValidation.ts`            |
| `features/domain/bayes`                      | `diagnostics.ts`、`draft.ts`、`expressionAst.ts`、`expressionSymbols.ts`、`index.ts`、`likelihoodDefaults.ts`、`parameterInference.ts`、`priorDefaults.ts`、`samplerDefaults.ts`、`validationFormatting.ts` |
| `lib`                                        | `utils.ts`                                                                                                                                                                                                  |
| `modules/bayes`                              | `public.ts`                                                                                                                                                                                                 |
| `modules/bayes/internal/ui`                  | `BayesView.tsx`、`bayesIssuePresentation.ts`                                                                                                                                                                |
| `modules/bayes/internal/ui/components`       | `BayesProgressStatus.tsx`、`BayesResultPanels.tsx`、`useBayesPlotData.ts`                                                                                                                                   |
| `modules/bayes/internal/ui/components/model` | `BayesFields.tsx`、`FormulaStep.tsx`、`LatexPresentation.tsx`、`SamplerStep.tsx`、`SymbolConfigDialog.tsx`、`SymbolRoleStep.tsx`、`symbolConfigValues.ts`、`types.ts`                                       |
| `services/bayes`                             | `bayesInferenceService.ts`、`bayesModelService.ts`、`index.ts`                                                                                                                                              |
| `services/ipc`                               | `index.ts`                                                                                                                                                                                                  |
| `services/platform`                          | `opener.ts`、`pathDialog.ts`、`platformTypes.ts`                                                                                                                                                            |
| `shared/charts/cartesian`                    | `MultiLineChart.tsx`                                                                                                                                                                                        |
| `shared/charts/core`                         | `domain.ts`、`layers.ts`、`margins.ts`、`theme.tsx`、`types.ts`、`useChartContainerSize.ts`                                                                                                                 |
| `shared/charts/statistical`                  | `PredictiveIntervalChart.tsx`                                                                                                                                                                               |
| `shared/theme`                               | `chartTheme.ts`、`colorThemePresets.ts`、`themeTokens.ts`                                                                                                                                                   |
| `shared/types`                               | `deepReadonly.ts`                                                                                                                                                                                           |
| `shared/types/bayes`                         | `expression.ts`、`index.ts`、`inferenceConfig.ts`、`likelihood.ts`、`modelSpec.ts`、`prior.ts`、`result.ts`、`validation.ts`、`wireParser.ts`                                                               |
| `shared/utils`                               | `globalEvent.ts`                                                                                                                                                                                            |

网页是独立插件页面，通信入口、Service/wire 解析、领域草稿与表达式、应用任务生命周期、
呈现组件和图表分别已有 owner。表单/任务的组件局部 state/ref 与图表局部尺寸不需要再镜像
到共享 store，低频主题 Context 不算高频完整模型广播；提交时捕获草稿副本有请求身份隔离
用途。没有以统一 Zustand 为由增加第二个持久业务模型。

**本批验证。** 十七个不同 Rust 用例全部通过：IV 三项、采样/调节五项、ADF 三项、
Panel 四项、稳健/分位数两项。所有新增断言都扩写已有用例，没有新增测试函数；
同一 Kernel tests.rs 被两个范围修改只记一份文件。Julia Web 三份非 UI 测试十二项通过，
插件 TypeScript、两份 TS 的定向 lint 与格式检查通过。Rust 本批修改文件共十三份。

```powershell
pnpm test:rs:package -p yss-sci --test regression_golden iv2sls_recovers_known_coefficients_with_single_and_multiple_endogenous_regressors
pnpm test:rs:package -p yss-node-kernel --lib iv_nodes_accept_multiple_instruments_and_preserve_identification_results
pnpm test:rs:package -p yss-application --test numeric_execution causal_category_nodes_execute_catalog_defaults_and_typed_effect_connections
pnpm test:rs:package -p yss-sci --lib visualization::tests::point_sampling_preserves_endpoints_and_quadrant_counts_use_the_population
pnpm test:rs:package -p yss-sci --test path_moderation
pnpm test:rs:package -p yss-node-kernel --lib every_visualization_kernel_executes_its_declared_input_layout_and_plot_carrier
pnpm test:rs:package -p yss-application --test numeric_execution path::moderation_nodes_keep_role_alignment_conditional_effects_and_every_prediction
pnpm test:rs:package -p yss-sci --test time_series_operations test_adf_preserves_drift_and_matches_mackinnon_reference
pnpm test:rs:package -p yss-sci --test panel_category panel_fisher_tests_match_statsmodels_reference_distributions
pnpm test:rs:package -p yss-node-kernel --lib selected_lag_var_and_adf_collections_keep_real_sample_and_failure_information
pnpm test:rs:package -p yss-sci --test panel_re_mle_diagnostic panel_re_mle_lin
pnpm test:rs:package -p yss-node-kernel --lib panel_nodes_select_each_implemented_estimator_and_reject_unsupported_combinations
pnpm test:rs:package -p yss-node-kernel --lib panel_estimation_predictions_and_comparison_failures_are_explicit
pnpm test:rs:package -p yss-application --lib numeric_extension_uses_actual_capabilities_and_rejects_old_artifacts
pnpm test:rs:package -p yss-sci --lib robust_estimators_and_quantile_match_independent_fits
pnpm test:rs:package -p yss-application --test numeric_execution regression_category_nodes_execute_catalog_defaults_and_conditional_parameters
pnpm lint:rs:package -p yss-sci --lib
pnpm lint:rs:package -p yss-sci-runtime -p yss-node-kernel --lib
pnpm lint:rs:package -p yss-sci -p yss-node-kernel --lib
pnpm lint:rs:package -p yss-sci -p yss-sci-runtime -p yss-node-kernel --lib
pnpm test:plugin:julia plugins/julia/web/src/features/application/bayes/useBayesInferenceTask.test.ts plugins/julia/web/src/services/bayes/bayesInferenceService.test.ts plugins/julia/web/src/shared/types/bayes/wireParser.test.ts
pnpm check:plugin:julia
pnpm exec oxlint plugins/julia/web/src/features/application/bayes/useBayesInferenceTask.ts plugins/julia/web/src/features/application/bayes/useBayesInferenceTask.test.ts
```

Clippy 各次退出零，仍为 SCI categorical 两条、nonparametric 与 sample_mean 各一条
共四条既有 warning。各 owner 的 Rust 定向 rustfmt 和路径 diff 检查通过；Root 已复核
修复与消费者断言。IV SCI 旧红/新绿编译分别 4.46/2.24 秒；Panel SCI 旧红/新绿执行
2.97/2.90 秒，Application 消费编译 25.62 秒；稳健/分位数 Application 编译/执行
17.12/6.54 秒。这些是检查成本，不是性能 benchmark。

本批未运行完整 CI、桌面构建、Julia 实际采样或进程端到端、插件打包及 UI 人工验收，
未提交 Git。既有 Graph JSON 边界的 benchmark 保留在本文原证据范围，未增加新例外。

**Julia 计算脚本范围。** plugins/julia/runtime/julia 下七个生产 .jl 共 1147 行已全文读完：
worker.jl、worker_protocol.jl、scientific_runtime.jl、ops/bayes_fit.jl，以及
ops/bayes/{expression,runtime,turing_generic_normal}.jl。worker.jl 已有上一批明确证据，
本批新增其余六个文件、982 行；两份 tests/*.jl 不计生产覆盖。Arrow 输入输出、表达式、
Turing 模型与调度入口按当前文件 owner 核对，未将 native Rust 的 32 份覆盖替代脚本阅读。

其中两项保持待验证：手工 prior preview 为 LogNormal 选择零时，合法 log(beta)
表达式可能在真正推断前被拒绝；这是前五行 predictor 准入，并非 sampler 初值。
ACTIVE_TASKS 只有登记、没有完成后的移除，复用 worker 进程时继续持有已完成 Task；
若在完成时移除，需要同时处理 EOF 持锁等待，避免引入锁等待闭环。
另有 Exponential 的 UI scale 标签与 Julia 取参数倒数的构造约定待核实，不据标签直接改
统计语义。这里只记录源码调用依据，未运行 Julia 来证明实际回归，也未测内存影响、
修改脚本或依赖。协作取消与 Rust 终止 worker 的分工不能证明实时取消延迟。

**后续入口与结论边界。** 本批已关闭上批 IV 矩阵/JSON 重读候选、本批 ADF 副本、
Panel 保留列索引和两项网页取消竞争；历史段落按当时证据保留，最新状态以本段为准。
剩余入口有源码/调用依据，但没有在本批取得各自旧红，不记为修复或架构例外：

- 自动 Newey–West 带宽的截距列假设、无截距 White/IM 的设计约定，以及有限负协方差
  经 raw t-test 后的错误边界，仍需从公开消费者取得独立行为证据。
- IV Summary 新计算的第一阶段、过度识别和内生性诊断有限值边界尚未完整验证；
  typed 展示重构不能替代这一准入证明。
- Panel MLE 无截距模型的 null likelihood、非平衡 TWFE 的变换，以及公开 ADF 极端
  lag 的算术准入仍待分别核实；普通 panel fit 已保护时间差，不能把 raw FD 的极端
  时间候选泛化为现有 GUI 路径已存在越界。
- Meta Fixed/零异质性可复用已计算的零方差拟合，Stouffer 权重可评估借用；White/IM
  重复分解、部分多变量/空间只读副本也待沿已有 owner 核对。未改无关 Meta 代码，
  未为这些候选声明测得收益。
- Julia FormulaStep 的分布参数以 split(',')[0] 拆解，可能截断合法 min/max 表达式；
  Domain 已有 formatRawExpressionLatex(rawPredictor)，应优先验证并复用这一 owner，
  不另建字符串解析器或 UI 单元测试。

整体 goal 仍为 active。SCI、Julia Web 全文清单闭合不取消跨 owner 行为、原生生命周期、
真实计算和界面验收的剩余工作，也不证明项目所有模块已满足全部规则。

04:17 的文档契约六项通过，包含模块索引检查；本文及四份 README 的定向 oxfmt 检查
通过。补入验证回执后，仅重验变化的本文与文档契约；交付前最后执行全局差异检查。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src-tauri/crates/yss-sci/README.md src-tauri/crates/yss-sci-runtime/README.md src-tauri/crates/yss-node-kernel/README.md plugins/julia/README.md
git -c core.safecrlf=false diff --check
```

## 2026-10-03：公式事实源、Julia 任务回收与对比检验边界

上一批形成了实际修复和新证据，本批继续三名子代理分工；Root 维护 Rust 公共入口、
Kernel revision 和统一记录。Cargo 在 Meta 与对比检验消费者之间依次交接，Julia、
TypeScript 和只读复核并行。未新增全仓扫描器、通用验证框架或测试调度层。

**公式使用已有 AST。** FormulaStep 原先从显示用 formulaText 按等号、分布正则和逗号
反拆 predictor，另以 bound AST formatter 回退。实际纯函数对照表明，嵌套
min(max(x, 1), 2) 被旧逗号拆分截断；新路径直接调用已有
formatRawExpressionLatex(draft.rawPredictor)，完整保留参数。formulaText/rawPredictor
已由应用解析流程共同安装，不需要再建立一套反向解析。

删除失去唯一消费者的 formatExpression、括号辅助函数和 BINARY_OPERATOR_LABELS，
保留 AST 类型及现有 raw formatter。扩写已有 Domain 纯用例验证嵌套函数，既有
expressionSymbols、formulaParsing 和 BayesPanels 三文件十四项纯函数测试通过。
BayesPanels 本次所选文件不执行渲染；没有新增 UI 单元测试。当前 CUA 没有可用
app/browser，本批没有界面验收，也不把纯逻辑对照称作渲染回归。

**Meta 复用已有计算和借用边界。** model 的初次零异质性拟合已经拥有 Fixed 和 tau=0
所需结果，现直接借用；仅 tau 非零才再次拟合，保留初次 Q 用于异质性统计。
Stouffer 对显式权重借用切片，缺省时才创建单位权重。输入校验、计算顺序、正常/KH
推断、预测区间和输出契约不变；独立数值 fixture 与实际 Application 消费者三项通过，
未增加测试、revision 或性能例外。

这两份 Meta 文件属于此前已有的未跟踪文件；git diff 为空不代表没有用户内容。
本批仅局部修改两处，未暂存。rustfmt 通过；额外 no-index diff 检查没有空白诊断，
其“存在全文差异”的退出 1 不伪称退出零。没有据减少一次拟合或复制宣称测得加速。

**对比检验只通过原校验入口。** 公开中立 HypothesisTestInput 接受有限负协方差后，
原 t 计算先开方，再检查 se<=0；NaN 绕过此比较，最终在 statrs beta.rs 内因
XOutOfRange panic。扩写已有 binary_postestimation 的系数约束用例，真实旧运行
零通过、一失败、二过滤，编译 1.24 秒；失败不是新增字段或编译错误。

现先检查对比方差非正/NaN，再开方；沿原 Scientific/ComputationFailed 错误返回。
同一用例保留原有效 t/z/chi² 断言，并验证有限样本与渐近入口均返回错误，新运行
一通过、二过滤，编译 3.25 秒。矩阵级 t/Wald 模块和函数收窄为内部入口；
全仓调用与消费者编译确认仅原 linear_test 直接消费，不保留可绕过准入的公开别名。

实际调用链核对到七个 Kernel ID，包括 diagnostic.wald 经 model.summarize 的间接
消费者。该节点单约束仍选择 t 检验，不能仅据节点名排除；Kernel 最终使用已有
InvalidParameter 映射，并非新增 ScientificFailure。实现 revision 同步如下：

| Kernel ID 后缀，前缀均为 yssbi.statistics.   | 原 revision | 当前 revision |
| -------------------------------------------- | ----------- | ------------- |
| linear.summary                               | 9           | 10            |
| logit.summary、probit.summary、prais.summary | 5           | 6             |
| iv.2sls.summary、iv.liml.summary             | 6           | 7             |
| diagnostic.wald                              | 3           | 4             |

扩写两个已有 Kernel 用例，实际开启三处开关并验证 z/t 类型；两种 IV 的 display-only
轮保持原样。已有 diagnostic.wald 用例真实调用 x1=0；Application 大报告用例在完整拟合
上执行线性 hypothesis 并与系数统计量对照。四项消费者通过，包含最终 revision。
独立 sample_mean 的 test.t.*、Fit/Predict 和 DID 不走此实现，未无据升版或扩大测试。

**Julia 先验参数只由实际 distribution 解释。** 原 bayes_prior_default 手工解释九类
先验，把 LogNormal(0,1) 的第一个参数 0 作为 preview，导致合法 log(beta) predictor
在前五行准入阶段被拒绝。现在先构造实际传给 Turing 的 priors，再使用 median.(priors)
做准入，删除整个重复解释函数；preview 不设置 sampler 初值。

新增一个独立 Julia testset，通过真实生成 predictor/likelihood 文件和现有模型入口运行，
在 initializing_nuts 进度点受控取消以证明已经通过准入，不启动 MCMC。旧实现实际得到
predictor returned a non-finite value at row 1；修复后到达预定取消点。首次绿测还暴露
fixture 缺少真实 payload 所带 sampler，补齐 fixture 后重跑完整文件；不把这次测试构造
失败当作另一个生产旧红。

**Julia Task 生命周期归同一列表。** 原 ACTIVE_TASKS 只登记、不移除，复用进程持续持有
已完成 Task。现在先创建未调度 Task，在原锁内登记后启动；包住整个 process_run 的
finally 按身份只移除自身，也覆盖参数准备前的异常。EOF 在停止准入后锁内取等待快照、
锁外 wait，避免回收 finally 与 EOF 持锁等待形成闭环，不增加第二个任务账本。

新增另一个独立 testset，以 Channel 固定完成、挂起、后继完成和取消顺序；旧实现六条
断言通过、四条快照断言失败。最初 EOF fixture 的多行 JSON 已改为协议要求的单行 NDJSON
并重新取得干净旧红，不以脚手架错误证明缺陷。修复后活动后继保留、完成记录移除、
取消分类正确，真实 worker 子进程在 EOF 后正常退出。EOF 检查不是确定性复现旧锁环，
也不是实时取消延迟或吞吐 benchmark。

**实际 Julia 环境与验证。** 本机原无 Julia；从
[官方版本清单](https://julialang-s3.julialang.org/bin/versions.json)
取得与 Manifest 一致的便携 Julia 1.12.6，在忽略目录 target/julia-audit 中使用独立 depot。
下载资产 SHA256 为 a63d991976e6893f508c512e3dc7bca1836c1a1f6ad1f3e4aedec159b6733e89，
实际校验匹配。没有更改全局 PATH、安装全局程序或修改 Project/Manifest；前后哈希及
最终 git diff 均确认两份依赖文件未变。

两份 Julia 源码测试全部退出零：bayes_fit_tests 八个 testset、三十二条断言，
worker_protocol_tests 三个 testset、十五条断言，共十一组、四十七条断言；新增两组
分别对应先验准入和任务回收。最终日志在 target/julia-audit/bayes-green-final.log 与
worker-green-final.log，干净生命周期旧红在 worker-old-red-corrected.log。
各 testset 时间不是完整进程 wall time，不用冷启动或预编译成本宣称算法性能。

**桌面和工具入口复核。** 本批还全文读取以下八个组合、构建与验证入口；不将名字搜索或重复
阅读累计到此前的 SCI/Kernel/插件全文数：

- src-tauri/src/main.rs、src-tauri/src/lib.rs、src-tauri/build.rs。
- plugins/scripts/package-plugin.mjs、plugins/julia/scripts/package.mjs、
  plugins/julia/scripts/test-native.mjs。
- scripts/generate-crate-dependencies.mjs、docs/reference/generate-module-map.mjs。

桌面保持 Tauri 组合入口，业务初始化、命令与结果租约归 Application；通用签名、资产
封装与同版本内容身份归 package-plugin，Julia 脚本只组合 Vite/Cargo 及该 owner。
两个生成器从实际 Cargo/目录生成派生导航，不另持业务状态。现有依赖快照检查通过，
共七十一个 workspace crate、二百四十八条声明；这不是依赖方向逐项合规证明。
两个现有打包规则测试均通过且无 skipped，使用测试资产，未替代真实插件包验收。

**本批检查与范围。** 八个不同 Rust 用例、十四项插件纯 TypeScript 测试、两项 Node
打包规则测试及上述四十七条 Julia 断言通过。Rust/TS 没有新增测试函数；Julia 新增
两个 testset 各针对独立实际回归。九份 Rust、四份 TS、五份 Julia 源/测试被修改，
另更新四份模块 README 和本文。Clippy 仍为 SCI 四条既有 warning；
Julia 冷启动仅见 SciMLBase 既有弃用提示，没有升级依赖来消除提示。

```powershell
pnpm test:rs:package -p yss-sci --test meta_category meta_pooling_regression_and_sensitivity_match_statsmodels_and_scipy
pnpm test:rs:package -p yss-sci --test meta_category meta_asymmetry_and_p_combination_match_reference_distributions
pnpm test:rs:package -p yss-application --test numeric_execution meta_category_executes_every_node_and_reuses_study_tables
pnpm test:rs:package -p yss-sci --test binary_postestimation coefficient_restrictions_use_z_and_chisquare_for_likelihood_models
pnpm test:rs:package -p yss-node-kernel --lib binary_and_prais_nodes_honor_options_and_predict_without_refitting
pnpm test:rs:package -p yss-node-kernel --lib iv_nodes_accept_multiple_instruments_and_preserve_identification_results
pnpm test:rs:package -p yss-node-kernel --lib weighted_diagnostics_and_cluster_covariance_use_fitted_observations
pnpm test:rs:package -p yss-application --lib large_report_reads_bounded_views_and_runs_tests_on_the_complete_fit
pnpm lint:rs:package -p yss-sci --lib
pnpm lint:rs:package -p yss-sci -p yss-sci-runtime -p yss-node-kernel --lib
pnpm test:plugin:julia plugins/julia/web/src/features/domain/bayes/expressionSymbols.test.ts plugins/julia/web/src/features/application/bayes/formulaParsing.test.ts plugins/julia/web/src/modules/bayes/internal/ui/components/BayesPanels.test.ts
pnpm check:plugin:julia
pnpm test:plugin:package
pnpm docs:crate-dependencies:check

$env:JULIA_DEPOT_PATH='G:\006RustProject\YssBI\target\julia-audit\depot'
$env:JULIA_PKG_PRECOMPILE_AUTO='0'
$julia='G:\006RustProject\YssBI\target\julia-audit\julia-1.12.6\bin\julia.exe'
& $julia --startup-file=no --project=plugins/julia/runtime/julia plugins/julia/runtime/julia/tests/bayes_fit_tests.jl
& $julia --startup-file=no --project=plugins/julia/runtime/julia plugins/julia/runtime/julia/tests/worker_protocol_tests.jl
```

前述 Julia 环境变量仅属于开发检查进程，不能假定会穿过插件进程的环境筛选。真实安装包、
Rust 插件进程与完整 MCMC 计算链尚待验证；本批未运行完整 CI、桌面构建或 UI 人工验收，
没有提交 Git，也没有新增规则例外。整体 goal 保持 active。

本批已关闭上批的 Formula 私拆、Meta 重算/权重复制、负对比方差、先验 preview 与已完成
Task 保留候选。Newey–West 截距、White/IM 无截距设计、IV 新诊断有限值、Panel null
likelihood/非平衡变换和公开 ADF lag 算术等入口仍保持原待验证状态。新补证的两个 Julia
入口也待单独确认：Exponential 的 UI scale 与实际 rate 约定，以及迟到 cancel 在已完成
Task 清理之后可能登记取消标记；本批未扩改这些独立行为。

**下一轮原生验收入口已核实。** native_extension target 的源现位于
plugins/julia/tests/native_extension.rs，已全文读取其五个 ignored 用例和内联 helper。
其中四项 view/lease 用例只启动插件；下面一项才运行环境准备和实际 NUTS
（1 chain、64 warmup、64 samples），并检查结果读取/保留、幂等、取消、上下文失效和卸载。
第二个长任务提交后立即取消，不能据此声称已测量采样中的取消延迟。

现有三个包均为旧 executable，未包含本批 Task 回收和 median.(priors)，仍有旧 preview
helper；须实际重建，不能使用 --skip-build 或只选择最新旧包。开发 depot 也不会直接
继承到原生链：PluginProcess 筛选环境，worker 使用自己的 depot 加标准用户 depot；
本机标准用户 depot 尚不存在。下一轮需明确准备可复用缓存或接受冷下载/编译，不修改
生产环境筛选来绕过这项前置。准备/计算等待上限为 620/200 秒，属于用例边界而非耗时数据。

```powershell
pnpm plugin:julia:package --dev
# 使用上述命令实际生成的新包绝对路径
$env:YSSBI_PLUGIN_TEST_PACKAGE = '<新包绝对路径>'
$env:PATH = "G:\006RustProject\YssBI\target\julia-audit\julia-1.12.6\bin;$env:PATH"
pnpm test:rs:package -p yss-plugin-runtime --test native_extension independently_installs_executes_cancels_and_uninstalls_a_native_extension -- --ignored --exact --nocapture
```

此命令预期一项运行、四项过滤；本批只完成前置调查，没有将它记作已执行或通过。

04:50 的文档契约六项通过，包含模块索引检查；本文和四份当前 README 的定向
oxfmt 检查均通过。源码和依赖未变，复用上述代码验证；原生打包与计算验收转入下一批。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src-tauri/crates/yss-sci/README.md src-tauri/crates/yss-node-kernel/README.md plugins/julia/README.md plugins/julia/runtime/julia/README.md
```

## 2026-10-03：原生 Julia 计算验收、ADF 准入与 HAC 截距

本批继续三个子代理并行：Plugin 实际打包和原生计算，Database 核实并修复 ADF 准入，
Graph 核对自动 HAC 的设计列身份；Root 维护文档和跨模块复核。Cargo 依次交接，
未通过并发重编译或新增调度层来加速。下面只记录已实际完成的验收。

**新包运行真实 Julia 计算。** `pnpm plugin:julia:package --dev` 成功，
总计 16.65 秒，其中 Rust 构建 10.29 秒，产物为
target/plugin-packages/yssbi.julia-0.2.0-dev.1790974263014.fcfad93dfc42-x86_64-pc-windows-msvc.yssplugin。
包内 executable 逐字节包含当前 worker.jl、expression.jl 和 turing_generic_normal.jl；
旧 bayes_prior_default 缺席。此次未使用 skip-build 或复用旧包。

原生链使用便携 Julia 1.12.6。标准用户 depot 原不存在，本任务临时创建
C:\Users\Administrator\.julia junction 指向已准备的 target/julia-audit/depot，
因此属于复用缓存的验证，不能称为全新机器的冷准备测试。结束时核对链接路径、类型、
唯一目标和创建时间，仅非递归删除链接本身；缓存保留、临时 fixture 根已删除，
相关 Julia/插件/测试进程为零。没有更改生产环境筛选、测试超时或全局 PATH。

第一次调用少一层本环境所需的 pnpm 参数分隔，Cargo 拒绝 --ignored，零测试运行，
不计作 native 用例失败。以下修正命令真实运行一项、过滤四项，最终一通过、零失败、
零 ignored；测试 41.10 秒，含命令包装 41.62 秒，Cargo 编译 0.25 秒：

```powershell
pnpm plugin:julia:package --dev
$env:YSSBI_PLUGIN_TEST_PACKAGE = 'G:\006RustProject\YssBI\target\plugin-packages\yssbi.julia-0.2.0-dev.1790974263014.fcfad93dfc42-x86_64-pc-windows-msvc.yssplugin'
$env:PATH = "G:\006RustProject\YssBI\target\julia-audit\julia-1.12.6\bin;$env:PATH"
pnpm test:rs:package -p yss-plugin-runtime --test native_extension independently_installs_executes_cancels_and_uninstalls_a_native_extension -- -- --ignored --exact --nocapture
```

实际覆盖安装、依赖准备及准备取消、NUTS（1 chain、64 warmup、64 samples）、
结果读取/保留、幂等提交、后续任务取消、上下文失效和卸载。后续任务立即取消，
不声称测得采样过程中的取消延迟；该 fixture 也不替代上一批 LogNormal 准入回归。
四个独立 view/lease ignored 用例本批没有运行，未把它们计入验收数。

原始日志在 target/julia-audit/native-package-build.log、native-compute-final.log
及 native-compute-argument-failure.log。Root 已读取这些日志，确认实际 target、参数和汇总。
Vite 仍提示包块超过 500 kB；没有为了消除提示扩改打包。以上耗时是单次验证回执，
不是架构方案的公平性能 benchmark。

**ADF 在唯一计算入口先验证 lag。** 公开 Runtime 原样把 usize lag 传到 SCI 的
adf_test；原实现先做 1 + lags，再检查有效样本，usize::MAX 因此在默认 test/debug
构建溢出。现把原有同一错误分支前移：确认至少四个观测后比较 lags >= n_raw - 1，
再分配差分及计算偏移。普通越界诊断、有效输入计算和原错误映射均保留，没有新增 helper、
重复校验层或任意样本上限。

扩写已有 Runtime 用例，保留未知 regression 的 InvalidInput 断言，加入普通越界
lag=11 和 usize::MAX。旧运行零通过、一失败、九过滤，编译 5.17 秒，
在 unit_root.rs:130:17 得到 attempt to add with overflow；前两个合法错误断言已先通过。
修复后同一用例一通过、九过滤，编译 2.63 秒；两种 lag 都得到既有
ComputationFailed { Adf }，未知 regression 分类不变。没有新增测试函数，
也没有把测试构造错误当作旧红；未运行 release 旧实现。

SCI 数值参考一通过、二过滤；Kernel ADF 集合消费者一通过、四十九过滤，
均无 ignored，共三个不同 Rust 用例。Kernel 本身先检查 lags>=n 或 lags>1000，
Panel 也先检查 lag 和样本自由度，极端输入不能从这些当前路径绕过；
普通 n-1 越界仍得到原失败。Plugin 子代理与 Root 独立复核该调用链，
因此没有改变 Kernel revision，也没有宣称 GUI 已存在该溢出路径。

```powershell
pnpm test:rs:package -p yss-sci-runtime --lib augmented_dickey_fuller_rejects_unknown_regression
pnpm test:rs:package -p yss-sci --test time_series_operations test_adf_preserves_drift_and_matches_mackinnon_reference
pnpm test:rs:package -p yss-node-kernel --lib selected_lag_var_and_adf_collections_keep_real_sample_and_failure_information
pnpm lint:rs:package -p yss-sci -p yss-sci-runtime --lib
```

两份 Rust 文件的显式 rustfmt 和局部 diff 检查通过；Clippy 退出零，仍为 SCI
四条既有 warning。SCI README 同步准入契约。本次 ADF 变更仅 guard 前移和已有用例扩写；
同文件之前的 MacKinnon 归并属于历史批次，不重复归因或计数。

**自动 HAC 读取已有截距身份。** 原 Newey–West(1994) 带宽 helper 在多列时无条件
排除最后一列；实际 OLS/WLS 和 IV 设计在首列插入截距，无截距时则不应排除任何列。
WLS 乘以 sqrt(weights) 后截距列不再等值，不能通过扫描列值补猜身份。
协方差公开入口现接收 intercept_col: Option<usize> 并拒绝越界索引，
经原 HAC 分支传给自动带宽；五个生产调用分别使用 OLS、WLS、2SLS、LIML 的
config.constant 或 FirstStage 已有 has_constant，内建设计传 Some(0) 或 None。
原始矩阵调用可明确传尾列位置；单列仍使用原全一权重。

该身份规则与 [sandwich 的 bwNeweyWest 源码](https://raw.githubusercontent.com/cran/sandwich/master/R/vcovHAC.R)
按截距名称排除权重、无截距保留全部列及单列特例一致。这里只核准截距权重规则；
该实现与本项目的其他带宽约定不同，未拿其完整结果充作数值 golden。
没有增加截距检测器、模型副本、配置层或兼容入口，未改其他 NW94 计算公式。

扩写原 HAC 用例时，第一次平滑数据的两个排列都选到带宽 18，未区分错误分支；
它不是旧红。最终复用仓内 diagnostics_reference.json 的前两个预测列和响应，
仅交换两预测列，旧实现实际选到 17/24，协方差 (0,0) 的对应断言失败：
零通过、一失败、一百零四过滤，编译 1.73 秒、测试 0.10 秒。生产代码当时尚未修改，
没有新增 fixture 或用复制算法公式生成 expected。

Root、Plugin 与 Database 分别复核了五个调用点和测试设计。单靠预测列置换不能抓住
含截距时误传 None，因为全部列求和也具有置换不变性；非等权 WLS 因此另需确认
真实截距角色转发。现有 Kernel 协方差适配始终构造 Some(bandwidth)，默认值为 1；
Newey { lag: None } 是另一条仅依赖样本量的公式，Panel 拒绝 HAC。
所以 Kernel 本次没有可达自动分支的行为变化，不升级 revision；
它的既有 IV 用例仅用于受影响接口和既有行为检查，不充当自动 HAC 回归。

修复后的 HAC 用例保留原五十行显式带宽检查，并用已有六百四十行参考数据验证
有/无截距的预测列置换、显式带宽控制、原始矩阵尾列截距及单列规则。
非等权 WLS 既有用例实际选择自动带宽；除列置换外，把同一拟合的加权设计和残差
交给原协方差入口、显式指定 Some(0) 核对转发，且确认 None 的协方差存在超过
1e-8 的差异。此对照证明消费者传参，不称另一套独立 NW94 公式 golden。

既有 IV 用例增选 HAC { bandwidth: None }，实际执行 Fit 与 first_stage(false)；
单内生变量的 summary 稳健 F 有限且为正，p 值在范围内，拟合标准误有限为正。
多内生变量的原系数/诊断断言保留，没有用不受自动 HAC 影响的 equation 标准误或
Shea 指标冒充该分支证据。测试编写中一次把 Option<f64> 当作 f64 导致编译失败，
按既有单内生 Some 契约修正后重新运行；它不属于生产数值旧红。

最终八个不同用例通过：SCI covariance 三项、WLS 一项、协方差参数边界一项、
OLS golden 一项、IV 自动 HAC/FirstStage 一项及 Kernel IV 兼容一项。
五个生产调用点及所选消费者均已编译；SCI、Runtime、Kernel
的聚焦 Clippy 退出零，仍只有 SCI 四条既有 warning。Rust 没有新增测试函数或 fixture；
HAC 改动为六份生产文件及三份外部测试文件，内联 covariance 测试计入其生产文件。

```powershell
pnpm test:rs:package -p yss-sci --lib regression::covariance::tests
pnpm test:rs:package -p yss-sci --test weighted_statistics weighted_statistics_use_the_transformed_intercept
pnpm test:rs:package -p yss-sci --test overall_inference named_covariance_rejects_unknown_or_incomplete_configuration
pnpm test:rs:package -p yss-sci --test regression_golden test_ols_golden
pnpm test:rs:package -p yss-sci --test regression_golden iv2sls_recovers_known_coefficients_with_single_and_multiple_endogenous_regressors
pnpm test:rs:package -p yss-node-kernel --lib iv_nodes_accept_multiple_instruments_and_preserve_identification_results
pnpm lint:rs:package -p yss-sci -p yss-sci-runtime -p yss-node-kernel --lib
```

本批验收共十二个不同 Rust 用例：原生计算一项、ADF 三项、HAC 及消费者八项；
与前一批同名的 Kernel IV 运行不在跨批汇总中重复计数。本批修改十一份 Rust 源/测试、
SCI README 和本文；前一批 Formula、Meta、假设检验及 Julia worker 的变更仍保留。
前一批未受本次影响的 TypeScript、Julia 源码和打包规则结果复用，不反复执行完整检查。

本批关闭了公开 ADF lag 算术、自动 HAC 截距身份及一个新安装包真实计算链验收入口。
White/IM 无截距设计与重复分解、IV 新诊断的完整有限值边界、Panel null likelihood/
非平衡变换、其他只读矩阵副本、Julia Exponential scale/rate 和迟到取消标记仍需逐项确认。
Julia 标记候选仅有代码时序依据：Rust 在锁外发送取消，Julia 完成后已清掉标记，
迟到通知可能重新登记；本批没有为它取得运行旧红或修改实现。
真实桌面/UI 验收仍未完成，未运行完整 CI，未新增规则例外或提交 Git。
整体 goal 保持 active，当前阅读清单和局部验收不等于全项目合规。

05:12 的文档契约六项通过，包含模块索引；本文及四份当前 README 的定向格式检查通过。
ADF 两份和 HAC 九份 Rust 的显式格式及局部差异检查均通过，Julia Project/Manifest
仍无差异。补入本回执后，仅重验变化的本文及文档契约，交付前最后执行全局差异检查。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src-tauri/crates/yss-sci/README.md src-tauri/crates/yss-node-kernel/README.md plugins/julia/README.md plugins/julia/runtime/julia/README.md
git -c core.safecrlf=false diff --check
```

## 2026-10-03：并行闭合依赖方向、Julia 取消与 White/IM 准入

本轮继续三个子代理：一路修复 SCI 诊断的模型准入与重复计算，一路修复 Rust/Julia 取消生命周期，
一路只读合并跨层依赖证据并交叉审查。主代理审查最终源码、修正文档覆盖口径并整合验证。
Cargo 按 Julia 库检查 → SCI/Kernel → 新插件包原生验收交接，源码冻结后才交窗；
只读审查、Julia 源码测试和文档工作并行。没有新增扫描器、许可表或全局线程/构建任务限制。

### 声明与实际入口的依赖方向

按当前模块索引、真实 manifests 和源码入口复核 71 个 Rust package 的 248 条非 dev 仓内声明，
另独立检查所有 manifest 的 dev、target 和 build 段；不是把依赖快照生成或编译成功当作方向证明。
分组为桌面/Application/Tracing 3 包、Plugin/Julia 13 包、Harness/IPC/UI Contract 8 包、
Graph/Function projection 11 包、Node 4 包、Database/Data 11 包、Project/身份 12 包、
SCI/MathExpr 5 包、其他基础 owner 4 包，合计 71 包。

已核对的易混淆边包括：`yss-graph-execution/Cargo.toml` 的 SCI Runtime 仅为 dev dependency，
消费者是 `examples/ols_bench.rs`；Kernel 是 SCI Runtime 的唯一生产消费者；
`yss-filesystem/Cargo.toml` 没有仓内依赖。Plugin Runtime 中 Julia 的 `native_extension`
是测试 target 路径，不是 Julia 实现的生产依赖。桌面 `src-tauri/src/lib.rs` 只组合平台插件、
Application 初始化及其唯一业务 invoke registry，销毁窗口时通知原结果 owner 回收。

前端 17 个模块的 `public.ts` 全部读取，并核对静态 import/re-export、显式动态 import、
相对越层路径与直接 Tauri 使用。App 的 `rootPanelRegistry.tsx`、`editorRendererRegistry.ts`
和 `App.tsx` 经 public 组合呈现或延迟加载窗口。Core/Domain 未发现反向业务引用
Application/Services/App/UI，Services 未发现反向引用 Features/Modules，跨模块消费者未发现
绕开 public 读取其他模块 internal。Application 对 Workbench public 的引用调用已有布局 owner；
Workbench 内部没有反向导入 `features/application` 业务流程。Julia Web 使用本地 SDK 的 MessagePort。

发布入口同时核对根 capability、桌面配置、Plugin Runtime 的 `bridge.rs`、
`PluginViewFrame.tsx` 和 `usePluginView.ts`：宿主 `csp: null` 不是插件缺失 CSP 的证据，
Runtime 会在插件页面前注入独立 CSP；iframe 使用 `sandbox="allow-scripts"`，MessagePort
绑定当前页面并在后续加载时撤销。实际浏览器隔离和重载仍须人工验收。
补读实际 before-build 的 `yss-application/examples/build_samples.rs` 与 Samples README，
确认源定义、派生 catalog 和运行时导入复用现有 IO/hash owner，没有新增第二份可写事实。

本轮未发现新的具体越层边，声明与上述实际入口的方向核对在此闭合；这不是所有运行时组合的行为证明。
主代理抽查 Cargo dev/生产区分、Filesystem 依赖、桌面注册入口、App/Workbench public、
CSP 和页面绑定，与子代理结论一致。覆盖表已删除“模块内部/TypeScript 全图未完成”及
“其余 Core/Application 未读”等过时措辞。重复阅读不再增加文件覆盖数。

### Rust/Julia 共用原任务 owner 处理取消

真实 worker 探针先确认：任务完成后再收到 cancel，旧 `CANCELLED_TASK_IDS` 会重新保留该 ID，
后继任务完成也不删除它。Rust 的 active 发布原先早于 stdin 写锁，旧的既有测试扩展后实际写出
`[cancel, run]`，得到 0 通过、1 失败、7 filtered；这不是仅靠源码推断的时序。
Julia 既有生命周期 testset 随后实际得到完成后取消标记仍在的两处失败：全文件 17 通过、2 失败。

Rust 复用原 stdin Mutex，在取得写入权后才短暂发布 active ID，随后写出 run；
已观察到该 active ID 的 cancel 必须等同一个 writer，因而排在 run 之后。
私有 `send_worker_message<W: Write>` 是普通发送和任务准入共用的唯一编码/写入路径，
没有增加锁或第二注册表；active 锁不跨 IO，写入失败仍由原调用点清 active。
这一规则适用于未带可选取消 token 的 Manager 调用；准入前没有活动任务，cancel 仍返回 false，
输入准备期间的取消继续使用已有可选 token。

Julia 保留原 `ACTIVE_TASKS` 列表和锁，每项同时拥有 ID、Task 和 cancelled 字段，
删除独立取消集合及其锁。cancel 只标记当前登记，未知或已完成 ID 不留下状态；
任务仍先登记再 schedule，finally 按 Task 引用删除自身，旧完成不能删除同 ID 的后继登记。
原 Rust request gate 继续串行准入计算，没有增加重复 ID 账本。EOF 仍在列表锁内取快照、锁外等待。
任务 ID 复用 `require_string` 在原控制入口准入，空 ID 经原 `invalid_request` 响应，
不再成为 Task 外未响应的错误。raw cancel-before-run 不为将来任务排队。

扩展原 Rust `cancellation_releases_active_task_lock_before_worker_io`，通过共用 writer 和既有
cancel hook 固定实际写出次序，并验证 IO 时可取得 active 锁；最终 Worker 库 8 项、Bayes adapter
库 4 项全部通过。该 writer 回归使用内存输出，不能冒充真实进程取消成功。
Julia 原生命周期 testset 实际验证迟到取消不残留、登记后首次执行前取消仍交付 cancelled、
活动任务取消及 EOF 清理；空 ID 也由真实子进程响应。全文件 3 个 testset、20 个断言通过。
Extension bin 的定向 Clippy 无 warning，Rust 精确格式及限定差异检查通过。
没有新增测试函数或 testset；前一批已存在的 Task 回收用例不算本轮新增。

```powershell
pnpm test:rs:package -p yss-julia-worker --lib cancellation_releases_active_task_lock_before_worker_io
pnpm test:rs:package -p yss-julia-worker --lib
pnpm test:rs:package -p yss-bayes-worker-julia --lib
pnpm lint:rs:package -p yss-julia-extension --bin yss-julia-extension
```

Julia 使用既有隔离 depot 和 `target/julia-audit/julia-1.12.6/bin/julia.exe` 执行
`--startup-file=no --project=plugins/julia/runtime/julia plugins/julia/runtime/julia/tests/worker_protocol_tests.jl`。
原始输出位于 `target/julia-audit/late-cancel-{probe,rust-old-red,julia-old-red,worker-final,adapter-final,extension-clippy,julia-final}.log`。
本轮只修改 worker Rust/README、Julia worker、既有协议测试及其 README 五个路径；
其他先前 Julia 先验预览和表达式变更原样保留，不计入本轮新增。

### White/IM 使用已拟合模型事实，复用既有分解

Catalog 中英文帮助已要求 White/IM 的原模型含截距和至少一个预测变量。
SCI 的原始矩阵函数约定第一列为常数，而 `residual::diagnose` 原先对无截距模型仍直接调用它们，
两个真实预测列会被当成“截距加一个预测变量”。本轮在原 diagnose 构造矩阵前检查
`model.constant`，拒绝不满足契约的 White/IM；不扫描数值猜截距，不额外补列，不改变 Fit/Predict。

扩展既有 Kernel `weighted_diagnostics_and_cluster_covariance_use_fitted_observations`，
沿真实 Fit 构造两个预测列的无截距 OLS/WLS，各自执行两个 standalone 诊断与 Summary 两项。
旧实现实际执行完全部八个检查，结果均不符合拒绝契约，得到 0 通过、1 失败、49 filtered。
修复后 standalone 返回既有 `ScientificFailure`，Summary 保留相应 `unavailable_reason`；
既有含截距、WLS、Cluster 及其他断言保留。没有新 fixture 或测试函数。

White 的普通/加权路径和 IM 辅助回归改为同一个原 `Svd::factor` 同时提供奇异值、秩及左右向量，
删除先 `singular_values()` 再分解同一矩阵的重复工作，并借用同作用域的列/矩阵结果。
保留原 full SVD、阈值、自由度和公式，不在这一修复中更换算法。
加权 IM 将已求得的加权 White 结果交给本文件私有公共尾部，只计算偏度、峰度和合计一次；
删除整套未加权 IM 计算后覆盖 White/total 的旧路径。仅异方差分量加权的既有约定不变。

受影响 Kernel revision 为 White 3→4、Information Matrix 3→4、Linear Summary 10→11；
其他注册、Fit/Predict 和工作区历史改动保留。SCI/Kernel README 同步上述当前契约。
最终 Kernel 定向用例及 SCI 的 OLS、WLS、direct-helper 三个 golden 共四个不同用例通过，
没有 ignored 或零匹配。三包 lib Clippy 退出零，仍为 SCI 四条既有 warning；六份 Rust 精确格式通过。
这支持删除重复工作，没有提出未测量的加速倍数，也未要求新的规则例外。

```powershell
pnpm test:rs:package -p yss-node-kernel --lib weighted_diagnostics_and_cluster_covariance_use_fitted_observations
pnpm test:rs:package -p yss-sci --test regression_golden test_ols_golden
pnpm test:rs:package -p yss-sci --test regression_golden test_wls_golden
pnpm test:rs:package -p yss-sci --test regression_golden test_diagnostics_direct_helpers
pnpm lint:rs:package -p yss-sci -p yss-sci-runtime -p yss-node-kernel --lib
```

三项 golden 均实际运行 `tests/regression_golden.rs`，各 1 通过、3 filtered；输出保留于工具
stdout，没有另存磁盘日志，不为补日志重复运行。主代理与 Graph 子代理分别检查最终三个 SCI
改动、两个 revision 选择入口和既有 Kernel 扩展，未发现新增问题。

### 新包原生验收与本批边界

源码冻结后重新运行 `pnpm plugin:julia:package --dev`，总耗时 12.697 秒，其中 Rust 6.66 秒。
产物为 `target/plugin-packages/yssbi.julia-0.2.0-dev.1790976992887.0d4040136c59-x86_64-pc-windows-msvc.yssplugin`，
包 SHA256 为 `df9573eba98c6ee99fa8bf51de7bef0064afae749ecb72893307bdfda2ae7b30`。
可执行文件 SHA256 为 `7a6db61e1cf66465d270ccc3aa630dfe1ecac3c146dbf42b45a727d5df77e814`；
当前 worker.jl、expression.jl 和 turing_generic_normal.jl 的完整字节均确认嵌入，
未沿用前一批旧包的验收结论。Web 构建仍有已有的 chunk 大于 500 kB 提示。

```powershell
pnpm plugin:julia:package --dev
pnpm test:rs:package -p yss-plugin-runtime --test native_extension independently_installs_executes_cancels_and_uninstalls_a_native_extension -- -- --ignored --exact --nocapture
```

上述 exact native 用例实际 1 通过、0 失败、0 ignored、4 filtered，测试耗时 41.08 秒，
命令总耗时 41.553 秒。覆盖真实 NUTS 计算、准备及准备取消、结果保留/读取、第二任务立即取消、
上下文失效和卸载；第二任务不是采样中途取消，因此不声称取得了中途取消延迟的测量。
其他四个 view/lease 用例本批没有重跑。

复用已有 Julia 1.12.6 和隔离 depot。测试前确认标准 `.julia` 不存在，暂建到该 depot 的 junction；
结束时检查路径、LinkType、唯一 target 及创建身份，只删除同一链接，保留缓存。
Project/Manifest 哈希未变化，主代理另确认两文件 `git diff --exit-code` 为零。
清理证明显示 fixture 已消失、标准链接不存在、缓存仍在、相关进程为零；所有进程 session 终态。
主代理已读取 `late-cancel-package-build.log`、`late-cancel-native-final.log`、
`late-cancel-package-proof.json` 和 `late-cancel-native-cleanup.json` 的实际输出，
这些原始证据均位于 `target/julia-audit/`。

本批合计 17 个不同 Rust 用例（Worker 8、adapter 4、White/IM 4、native 1），
另有 Julia 3 个 testset 的 20 个断言；同一 Rust focused case 的先红/后绿与库重跑不重复计数。
OLS/WLS golden 和 native 等跨批重复用例不再算全项目新增覆盖。
本批修改七份 Rust 源/测试、两份 Julia 源/测试、四份模块 README 和本文；
没有新测试函数、fixture、UI 单元测试或规则例外，没有运行完整 CI、提出性能倍数或提交 Git。
原有四个 Graph JSON 例外继续使用各自未变化源码的同语义 benchmark 证据。

当前 UI 工具仍返回 `apps: []`、`browsers: []`，本批未进行真实界面验收。
已修改路径的项目替换/同路径图关闭重开、分屏浮窗取消恢复、结果租约及 iframe 重载仍保留。
前一批列出的 IV 新诊断有限值与 JSON 交界等候选仍需具体确认，不把尚未复现的数值猜测写成新违规；
一般统计精度探索、locale 逐句校对和未实现产品能力不自动成为本架构目标的新范围。
goal 保持 active；本轮存在实际修复和验收进展，不能因界面不可用将其误报为整体无进展或已完成。

05:42 的文档契约 6 项通过，包含既有模块索引检查；本文和上述四份 README 的定向格式检查通过。
补入本回执后只复验变化的本文及文档契约，交付前最后执行全局差异检查。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src-tauri/crates/yss-sci/README.md src-tauri/crates/yss-node-kernel/README.md plugins/julia/native/crates/yss-julia-worker/README.md plugins/julia/runtime/julia/README.md
git -c core.safecrlf=false diff --check
```

## 2026-10-03：并行修复 IV 不可用诊断、Panel 重复计算和 Exponential 标签

三个子代理分别承担 IV 原计算及消费边界、Julia 标签与 Panel 去重、规则证据及最终交叉审查。
主代理审查差量、维护规范 README 与本文，源码冻结后统一检查。Cargo 按 IV → Panel →
三包 Clippy 顺序交接；只读审查、前端检查和文档并行，不同时改写相同 Rust 源或争用构建窗口。

### IV 在原计算模块表达不可用结果

既有 Kernel IV 用例先沿有效 Fit → Summary 复现首阶段调整 R² 被编码成 Null：
0 通过、1 失败、49 filtered。JSON 本身不能区分 `Some(NaN)` 和明确的 `None`，因此继续在
既有 SCI golden 中检查原类型，而不是在 JSON 适配层增加通用有限值修补器。

SCI 原实现的实际失败包括：16 行响应 `1..16`、无外生变量、内生变量与工具均为全一列、
`constant=false` 时，调整 R² 为 `Some(NaN)`，Hausman 为 `Some(stat=0,p=NaN,df=0)`；
两行响应 `[1,2]`、内生与工具为 `[1,1]` 时，Wu 分母自由度为 0、p 值为 NaN。
后一个输入的 Hausman 实际是有限 `Some(stat=0,p=1,df=1)`，不把它描述为两项均不可用。
两次 typed 旧红均为 0 通过、1 失败、3 filtered。最初缺少 Kernel 必需参数的试跑属于用例构造错误，
不计为生产旧红。

`first_stage.rs` 在现有居中总平方和为零时返回已有的 `None`；其他非零分支公式保留。
`postestimation.rs` 只在原分布可由实际自由度构造时返回 Hausman 或 Endogenous 结果，
零秩 Hausman、Wu 分母自由度为零的组合检验保持不可用。保留其他可用诊断与有效 Fit，
没有新增结果类型、重拟合、通用校验框架或无截距 R² 公式改写。
Runtime 的两项诊断同时不可用分支，现在用现有协方差事实区分
`insufficient_residual_variation_or_degrees_of_freedom` 与 `requires_nonrobust_covariance`。

最终 Kernel 用例同时覆盖有意的调整 R² Null、16 行输入的 Hausman 整项 Null、
独立 Hausman 节点的 `ScientificFailure`，并保留两行全一设计的有限 Hausman。
另用两行内生/工具 `[1,0]` 实际运行 Fit → Summary，验证两项 None 及非稳健协方差下的正确原因；
这是最终消费覆盖，没有为该输入单独取得旧红。正常 2SLS/LIML、HAC 和多内生 Shea 原断言仍在。

可观察的 endogeneity 结果与独立诊断分别更新 `yssbi.statistics.iv.2sls.summary` revision 7→8、
`yssbi.statistics.diagnostic.hausman` 4→5。LIML Summary 仍为 7：其共享首阶段内部从
`Some(NaN)` 变成 `None`，既有 JSON 均为 Null，未因此新增可观察结果变更。Fit/Predict 不变。
SCI、Runtime 与 Kernel README 已同步这一诊断边界。

```powershell
pnpm test:rs:package -p yss-sci --test regression_golden iv2sls_recovers_known_coefficients_with_single_and_multiple_endogenous_regressors
pnpm test:rs:package -p yss-node-kernel --lib iv_nodes_accept_multiple_instruments_and_preserve_identification_results
```

最终两个不同既有用例均通过，SCI 为 1 通过/3 filtered，Kernel 为 1 通过/49 filtered，均无 ignored。
编译分别 2.87、4.87 秒，执行分别 0.18、0.38 秒。六份 Rust 精确格式及限定差异检查通过；
原始结果保留在工具输出，没有另存磁盘日志。这闭合已实测的三个未定义统计边界，
不声称证明所有 IV 非有限值、任意机器溢出或统计精度情形。

### FE 与 RE 共用单向组内中心化

原 FE 和 RE 各有一份相同组均值与逐观测减均值实现。本轮将计算归入已有私有 `panel::data`，
FE 的原包装继续返回相同长度错误，RE 在既有形状准入后通过别名调用共享函数。
删除 FE 纯别名及 RE 复制的算法，输出直接构造 `Col`，不再中间收集 Vec。
组内加总次序、跳过 NaN、全缺失组的零均值及输入观测顺序保持原语义。

修改仅涉及 `panel/data.rs`、`panel/fe.rs`、`panel/re/shared.rs` 三文件，净 26 行增加、47 行删除，
没有新增模块、trait、框架、测试、fixture 或 revision。双向中心化的 NaN/总均值语义有差异，
未将它们强行合并，也未改写非平衡 TWFE 或 RE null likelihood 公式。SCI README 已说明共享 owner。

```powershell
pnpm test:rs:package -p yss-sci --test panel_operations
pnpm test:rs:package -p yss-sci --test panel_re_mle_diagnostic
pnpm test:rs:package -p yss-node-kernel --lib panel_nodes_select_each_implemented_estimator_and_reject_unsupported_combinations
```

实际分别 6、1、1 个不同用例通过，最后一项 49 filtered，均无 ignored。
RE golden 使用已有 476 行真实 fixture，保留 LL `334.649391657151` 与 LR `964.5033822605337`；
Kernel 经 Fit → Summary 覆盖已实现的估计器/效应选择及不支持组合拒绝。
编译分别 4.41、1.11、5.84 秒，RE golden 执行 2.82 秒，Kernel 执行 0.26 秒。
三份 Rust 精确格式与限定差异检查通过。复用已有消费测试，没有为纯去重增加同构测试或声称加速倍数。

### Exponential 参数名称跟随已有 wire 契约

实际读取 UI 值转换、Rust prior 定义与转换、Julia `ops/bayes/runtime.jl` 及锁定的
Distributions 0.25.127 构造源码，确认 wire 一直是 `args: [rate]`，Julia 已将其变成
`Exponential(1 / rate)` 的 scale。UI 原来显示 Scale/尺度，例如输入 2 实际均值为 0.5。

本轮只将 `SymbolConfigDialog.tsx` 的 Exponential 参数标签改用现有 locale 结构中的新增
`rate`，中英文分别为“率”和“Rate”，并在 Julia runtime README 写明转换。
数值、wire、Rust 和 Julia 执行链均未变。`pnpm check:plugin:julia`、四条修改路径的定向格式
及差异检查通过；未新增 UI 单元测试。真实中英文显示仍待人工验收。
前一批原生包仍证明当时未变化的计算/worker 链，该包不含本批新 UI 标签；本批未重新打包。

### 当前规则证据与保留例外

前端共享 owner、ID 订阅、一次事务发布、引用和输入隔离、原 parser 的边界准入、
唯一 Graph 同步/补丁入口与手势暂态已按真实调用链核对，相关状态和应用回归见各批。
Rust 十项要求按职责、依赖、接口、唯一实现、数据生命周期和旧路径检查；
本批关闭实际发现的 Panel 重复与 IV typed 边界。前一批声明/实际入口的依赖方向结论继续有效。
不再把所有未枚举的算法组合或一般精度探索列为无穷架构待办，受影响界面验收继续保留。

四个 Graph JSON 例外的原测量仍分别位于
[constants.dataValue 原始记录](../benchmark/probes/graph-constants-data-value-2026-10-02.txt)和
[另外三个投影 JSON 边界原始记录](../benchmark/probes/graph-projection-json-2026-10-02.txt)。
两者均为同输入、同外壳、同一事务、同校验与冻结的 ABBA 四个新进程对照，每任务 30 次，
并检查内容、删除、保留字键、旧值隔离及引用语义。B 使用 Immer 遍历并保留危险字典键回退，
不是纯 Immer 替换。前者 500 个常量，8 个测量任务，A 为 4.33–4.61 ms、B 为 29.65–30.29 ms；
后者 500 节点/1500 个独立 JSON 根，12 个任务，fresh A 为 25.84–34.23 ms、B 为 257.85–267.07 ms，
delta 误差区间重叠，不宣称 delta 加速。

本轮复核 constants 原记录的生产/helper/对照/validator 等 7/9 个哈希仍匹配；另外两个是后来
追加三种 projection 场景的 fixture/bench 文件，其当前哈希与后一个记录一致。不能写成旧九个文件
全部未变；原 constants 场景/命令仍保留，旧数值只属于记录的进程和输入。后一个记录 13/13 哈希匹配。
生产策略未变，无需重复 benchmark。固定合成规模、共享机器、OS/GC 未隔离等局限继续保留；
准备耗时包含派生、校验与冻结，不等于 helper 自耗时、单个调用归因或 UI render/layout/paint。

### 合并检查与剩余人工验收

本批合计 10 个不同 Rust 用例（IV 2、Panel 8），不重复计算旧红/后绿、既有断言或前一批 17 项。
主代理在最终 Rust 冻结后执行以下三包 lib Clippy，退出 0，耗时 2.79 秒；
仍有 SCI `hypothesis` 的四条既有警告：categorical 的 is_multiple_of/type_complexity、
nonparametric 的 explicit_counter_loop、sample_mean 的 too_many_arguments。
根 `pnpm lint:ts` 退出 0，仍有 projectEventStream 的 spread 提示及两份既有测试的 this-alias 提示。
没有为了消除提示改动这些路径。最终只读交叉审查未发现本批新增问题，结论仅覆盖上述差量及直接调用。

```powershell
pnpm lint:rs:package -p yss-sci -p yss-sci-runtime -p yss-node-kernel --lib
pnpm check:plugin:julia
pnpm lint:ts
```

本批修改九份 Rust 源/测试、三份 TS/TSX、四份模块 README 与本文，共 17 个路径；
工作区其他历史改动保留。没有新测试函数、UI 单元测试、规则例外、完整 CI 或 Git 提交。

本轮重新取得的 UI 状态仍为 `apps: []`、`browsers: []`。已向用户发出验收条件问题，尚未收到答复，
没有把等待时间当作确认。根 `.rules` 明确要求“禁止给 ui 写单元测试，涉及验收请使用人工验收”。
真实桌面仍需覆盖表中的受影响交互，主要组合链包括：画布单/多节点拖拽、缩放与 Escape/隐藏/保存中断，
分屏浮窗及位置单次提交并单独记录 render/layout/paint；确认期间切换项目、同路径图关闭重开与结果租约；
插件页面重载/卸载后的 MessagePort 撤销及 Exponential 中英文标签。其余已修改呈现入口按覆盖表验收，
不以这三条组合链抹去各模块的原验收范围。官方桌面入口仍为 `pnpm dev`。

前批四个原生 view/lease 用例和最新原生 NUTS 包证据已存在；不能把插件生命周期全部记为未测，
也不能用这些进程结果、静态页面、源码阅读或编译代替人工交互。实际采样中途取消的延迟尚未测量。
源码和定向行为本批有实际进展，goal 保持 active，未宣称全项目验收完成。

06:13 的文档契约 6 项通过，包含既有模块索引检查；本文与 SCI、SCI Runtime、Kernel、
Julia runtime 四份 README 的定向格式检查通过。补入本回执后仅复验变化的本文与文档契约，
交付前最后执行全局差异检查。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src-tauri/crates/yss-sci/README.md src-tauri/crates/yss-sci-runtime/README.md src-tauri/crates/yss-node-kernel/README.md plugins/julia/runtime/julia/README.md
git -c core.safecrlf=false diff --check
```

## 2026-10-03：剩余证据核对与真实采样取消验收

前一轮属于实际进展：IV、Panel 和 Rate 标签已改变当前源码并完成所选检查。
本轮三个子代理继续分工：前端人工验收范围核对、Rust 十项剩余证据审计、真实采样取消取证。
复用未变化源码的既有结果，只补具名缺口；没有以再读文件、全量编译或状态复述增加完成数。

### Rust 十项当前证据入口

当前相关 manifests、源码与 README 已重新核对；完整生产阅读和 71 包依赖方向证据复用前批。
没有发现另一个具名、已确证但尚未修复的 Rust 源码架构问题。以下对应关系不是新的自动架构扫描器，
也不把这一轮的局部复读计为第二次全生产审计。

| 要求                 | 当前权威实现与证据入口                                                                                                                      |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. 结构清晰          | Graph Analysis 的解析、投影、求解、快照与 Execution 的准备、运行、结果、最终提交按职责分开；根入口及各批消费者验证已核对                    |
| 2. 职责单一          | Kernel 适配节点输入，SCI Runtime 组织中立调用，SCI 计算，Linalg 独占 faer；当前 manifests 与模块 README 一致                                |
| 3. 层次分明          | Kernel 无 Graph/Application/Tauri 依赖，Runtime 无 Arrow/Linalg 依赖，Filesystem 无仓内依赖，IPC Contract 无 Tauri 依赖；全量方向证据见前批 |
| 4. 最小接口          | Analysis 私有模块以明确 re-export 暴露能力；GraphSemanticSnapshot 提供借用的就绪视图，不复制为可写语义模型                                  |
| 5. 唯一事实源        | FE/RE 共用 panel::data 的组中心化；TabularColumnName 拥有列名规则；Contract execution 拥有 ScientificInputViolation                         |
| 6. 控制抽象          | Panel、IV 在原函数和已有类型内修复，未新增框架、平行账本、通用 serializer 或生产 benchmark 策略开关                                         |
| 7. 函数设计          | White/IM 复用同次 SVD 与加权公共尾部；各阶段保留具体计算职责，不按行数或 lint 提示机械拆分                                                  |
| 8. 数据流和所有权    | Harness EventWriter 持久化后顺序发布；Host 取消、Waker 替换/释放、Channel 发送/析构保留已修正的锁外边界                                     |
| 9. 目录表达职责      | SCI/Runtime/Contract 按领域组织；Kernel statistics/common 仅承担输入、模型、表格、有限值及错误适配，README 明确边界                         |
| 10. 修改与旧路径清理 | SCI/Runtime 共用输入违反类型；重复计算在原 owner 删除，行为变化沿已有 Kernel revision 注册，最新 IV Summary 8、Hausman 5                    |

相关当前契约见 [Graph Analysis](../../crates/yss-graph-analysis/README.md)、
[Graph Execution](../../crates/yss-graph-execution/README.md)、
[Kernel](../../crates/yss-node-kernel/README.md)、[SCI](../../crates/yss-sci/README.md)、
[SCI Runtime](../../crates/yss-sci-runtime/README.md)和
[Harness](../../crates/yss-harness-core/README.md)。
EventWriter 的串行 append→publish 是顺序契约，不重新标成死锁待办。
历史候选中的 Meta、对比检验、HAC、ADF、White/IM、IV 按后续批次核销；
Panel null likelihood、非平衡 TWFE、任意溢出及一般精度探索尚无已证架构违规，不据此追加无界数值矩阵。

### 受影响界面的七组人工验收

该轮 UI 工具返回 `apps: []`、`browsers: []`，当时尚未收到人工反馈。
后续用户于 2026-10-03 先确认项目管理基本流程全部成功，再逐项确认以下七组功能通过。
画布功能已通过；用户随后明确要求本轮跳过 Performance 录制及量化分析，该项记为未测。
以下保留已验收的范围与入口，七组功能无需重测；具体用户回执与证据限制见本文末尾。
根 `.rules` 要求人工验收，`src/.rules` 要求数据处理与浏览器 render/layout/paint 分别测量，
因此四处 JSON benchmark、源码或测试绿灯不能替代以下实际呈现证据。

| 验收组                 | 受影响的实际流程                                                                                                                                        | 当前入口                                                                                                                                                                     |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 画布与高频绘制         | Graph 平移/缩放、单多节点拖动与连接反馈；Escape、隐藏/保存中断、松键一次提交；Mind 树布局/测量/选择；分屏浮窗交互（绘制量化按用户要求跳过）             | [Graph Editor](../../react/src/modules/graph-editor/README.md)、[Document Editor](../../react/src/modules/document-editor/README.md)、[Workbench](../../react/src/modules/workbench/README.md) |
| 工作台、窗口与设置     | 拖动/分屏/停靠/折叠/恢复和面板状态保留；dirty 关闭取消、确认期间换项目、同路径图重开；独立窗口恢复、主题/语言与确认焦点                                 | [Workbench](../../react/src/modules/workbench/README.md)、[Features](../../react/src/features/README.md)                                                                                 |
| 资源编辑与目标身份     | 创建/重命名/删除弹窗关闭重开及换项目；Graph/Mind/Doc/Chart 保存/放弃；Details 切换、删除回退及草稿保留；目录搜索/拖入、菜单快捷键、Problems/Output 定位 | [资源操作](../../react/src/features/application/resource/README.md)、[Features](../../react/src/features/README.md)                                                                      |
| 导入、表格、图表与结果 | 导入选择/取消；分页、拖选、复制、全选及失焦；切换后的迟到响应；结果报告切换/补选、快速关闭及独立窗口租约；PDF/外链打开                                  | [Results](../../react/src/modules/results/README.md)、[结果应用层](../../react/src/features/application/results/README.md)及覆盖表对应呈现入口                                           |
| Logs 持续呈现          | 持续追加、领域切换/分屏、筛选、Details、自动滚动、主/独立窗口及恢复/终止状态反馈                                                                        | [Logs](../../react/src/modules/logs/README.md)                                                                                                                                     |
| Assistant 流式会话     | 真实流式回复及工具/链接呈现，期间切换会话、关闭重开或失败恢复；HMR 后旧订阅不再续写                                                                     | 既有 assistantHarnessRuntime、assistantHarnessSession 与 AssistantThread 入口                                                                                                |
| 插件页面与任务交互     | UI 安装/维护/卸载后的列表和面板；iframe 重载/关闭重开的 MessagePort 撤销；运行中取消与后继状态；公式展示及 Exponential 中英文 Rate 标签                 | [Julia 插件](../../crates/yss-plugin-runtime/README.md)、PluginViewFrame 与 PluginsPanel 入口                                                                                            |

前几组直接覆盖面板暂态、窄订阅、一次发布、身份准入和高频绘制，其他组验证这些规则在已修改消费者上的呈现。
Logs 已有积压/存储失败回归，不要求手工制造全部磁盘故障；Assistant 验证实际会话归属，
不扩成全部模型供应商、记忆策略或回答质量验收。外部服务限定为本次涉及的原生选择器、剪贴板、
PDF/外链和 Assistant 配置入口；locale 全文校对与未实现插件能力不自动追加。
覆盖表后没有具名入口的“其余模块/读取边界继续检查”旧段落已更正；插件行区分已验证原生计算链与页面交互。

### 以真实采样进度准入取消

现有 native 用例的第二个任务原来在 start 后立即取消，不能证明已进入 NUTS 采样。
本轮仅扩展 `plugins/julia/tests/native_extension.rs` 的原用例：通过既有 `tasks.get` 等待
`state=running`、`progress.stage=sampling` 且 `warmup < completed < total`，然后调用原取消入口。
进度来自 `bayes_fit.jl` 的真实采样 callback，不以固定等待时间猜测阶段，不增加生产 hook。
等待有原 180 秒任务预算，提前终态或未进入阶段即失败；保留原 30 秒取消终态预算和其后断言。
长任务请求从 1,000,000 减为 100,000 draws，以限制实际采样的内存需求；未让任务提前成功替代取消。

当前代码实际观察到 sampling **2500 / 100064**，warmup **64**，随后
`tasks.cancel` 至观察到 `cancelled` 为 **259.4583 ms**。这一数值包含宿主取消 token 导致 worker
进程终止的路径和 250 ms 状态轮询，属于本机该次 fixture 的端到端观察，不是纯 Julia 协作取消、
NUTS 内部抢占延迟或所有环境性能保证。原准备取消、短采样成功、结果读取、同操作重试、
上下文失效及卸载断言仍全部执行。

```powershell
pnpm test:rs:package -p yss-plugin-runtime --test native_extension independently_installs_executes_cancels_and_uninstalls_a_native_extension -- -- --ignored --exact --nocapture
pnpm lint:rs:package -p yss-plugin-runtime --test native_extension
```

唯一 exact 用例实际 1 通过、0 失败、0 ignored、4 filtered；编译 1.91 秒，测试 40.69 秒，
命令总耗时 43.2497 秒。所选 test target Clippy 退出 0、无 warning、耗时 0.52 秒；
该 Rust 文件精确格式和限定差异检查通过。这是扩写一个既有用例后的验证，不增加新的测试函数计数；
早前同文件的三个 view 回归不算本轮新增，也未重跑另外四个用例。

复用前批包 `yssbi.julia-0.2.0-dev.1790976992887.0d4040136c59-x86_64-pc-windows-msvc.yssplugin`。
执行前重新核对包 SHA256 `df9573eba98c6ee99fa8bf51de7bef0064afae749ecb72893307bdfda2ae7b30`、
可执行文件 `7a6db61e1cf66465d270ccc3aa630dfe1ecac3c146dbf42b45a727d5df77e814`；
后者与当前已构建二进制相同，assets.rs 引用的 Project/Manifest 和七份 Julia 源码完整当前字节均嵌入。
生产 Rust/Julia 未改，故没有重打包。旧包不含前批 Rate 标签，本次也没有 UI 验收。

使用既有 Julia 1.12.6 与隔离 depot。执行前标准 `.julia` 不存在，临时 junction 的路径、类型、
目标和创建身份有记录，结束后只删除该同一链接；缓存保留，环境变量恢复，Project/Manifest 哈希未变。
本次唯一测试目录 `yssbi-native-extension-13615e9e-9201-40ef-b276-ac3f53d3b272` 已消失，
相关进程为零，所有 session 终态；没有重试历史上被拒的 view 目录删除。
主代理已读取原始输出、包证明、环境前后记录及清理证明：
`target/julia-audit/sampling-cancel-{native-final.log,native-clippy.log,package-proof.json,environment-before.json,junction-proof.json,native-environment.json,fixture.json,native-cleanup.json}`。
Julia 插件 README 同步了这一验收方法与解释限制。

### 本轮完成审计

本轮源码差量仅为既有 Rust native 测试净增 39 行，生产实现未改；另更新 Julia 插件 README 与本文。
没有新增规则例外、benchmark、全项目测试矩阵、UI 单元测试或完整 CI，没有提交 Git。
四处 JSON 例外继续复用上一轮已校对适用范围和源码的测量记录。

截至本轮，已定位的非 UI 缺口可独立执行的最后一项——真实采样中途取消——已有实际输出。
源码与行为证据仍不能证明七组界面验收完成。最近三个连续 goal 轮次（05:42、06:13 与本轮）
都实际观察到 UI 工具没有 apps/browsers，用户的验收条件答复仍缺失；前两轮及本轮先完成了能够独立推进的修复和验证。
现在继续核销这些必要项依赖可操作的应用界面或用户按 `pnpm dev` 提供人工验收反馈，
不是通过重复相同检查、扩大算法探索或把待验收项改成已完成来解除。整体完成条件尚未成立。

06:25 的文档契约 6 项及本文、Julia 插件 README 的定向格式检查通过；
主代理另确认 Project/Manifest 的 `git diff --exit-code` 为零。补入此回执后仅复验变化的本文与
文档契约，交付前最后执行全局差异检查。

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md plugins/julia/README.md
git -c core.safecrlf=false diff --check
```

## 2026-10-03：项目管理 unknown_error 回归

用户人工反馈清理、导入、新建和导出项目均失败，并补充界面错误为 `unknown_error`。
这是真实失败反馈，推翻了先前对这些入口的完成预期；前批源码复核和测试通过不能代替界面验收。
三个子代理分别核对前端调用链、Rust wire 契约和运行日志，随后进行独立源码复核。
日志没有本次失败记录，未用旧日志推断原因；主因由真实 Rust 序列化样本和前端失败回归确定。

Rust `ProjectRecord` 始终返回八个字段，前端 `parseProjectRecord` 却仅接受七个，漏掉
`rootIdentityState`。含该字段的正常回复因精确字段校验而抛出 `TypeError`，应用层将其映射为
`unknown_error`。清理命令的 `{removed}` 本身合法，随后列表刷新被拒；导入的注册记录、
新建和另存为的生命周期回执均经过同一解析器。生命周期事件也共用它。
当前源码的项目复制导出入口为“项目另存为”及 `save_project_as`；没有独立 `exportProject` 命令，
本次不据此宣称数据库或结果导出已验收。

修复直接补齐原类型和 parser：要求八字段及 `valid`/`invalid` 状态，拒绝缺失状态、未知状态及未知字段。
`rootIdentity` 保留 Rust 定义的字符串语义，包括空值；记录解析不授予删除权限，Rust 原有
`deletion_identity()` 状态与非空身份校验保持不变。没有兼容默认值、忽略未知字段或吞掉解析错误。
原后端操作可能在前端拒绝回执前已提交，本轮没有重新执行真实项目写入、清理或删除。

此前前端测试另行手写七字段记录，与错误 parser 一致，所以没有检出回归。
现在 Rust 既有序列化测试和前端 Service 既有契约用例共用
`src/tests/fixtures/project-event-wire/project-record.json`：Rust 真实序列化必须等于该样本，
前端必须接纳同一份样本。既有 Service case 在旧 parser 上实际失败（1 failed、19 skipped，
9 个 soft 断言失败），修复后同 case 通过；两份 Application 测试仅补齐已有 fixture 的状态。
没有新增 UI 测试或测试函数。

独立复核另发现删除文件请求的字段名错误：前端发送 `expectedActiveProjectInstanceId`，
Rust 参数 `expected_active_instance_id` 对应 `expectedActiveInstanceId`。
这是单独的请求映射缺陷，不是上述四项失败的共同原因。既有 Service case 增加完整请求断言后，
先真实复现唯一字段名差异（1 failed、19 skipped），再在原 Service 内显式映射正确字段。
验证通过模拟 IPC 完成，没有调用真实回收站或删除用户数据。

受影响的四份前端测试共 **60 个不同用例通过**：Service 20、事件解析 2、回执结算 21、生命周期操作 17。
删除参数修复后重跑 Service 20 项，另外三文件的 40 项复用未变化代码的本轮结果，不重复计数。
`pnpm check:ts` 通过；Rust registry contract 三个既有测试及该包 lib/tests Clippy 通过，后者无 warning。
最终 TS lint 退出 0，保留前批三个 warning（ProjectEventStream 的 spread 及两处既有测试的 this alias）。
没有运行完整 CI、重建桌面程序或宣称人工验收通过；本轮 Rust 仅改既有测试的期望来源，生产修复均在前端。

```powershell
pnpm test:ts src/services/project/projectService.test.ts src/services/project/projectEventParser.test.ts src/features/application/projectLifecycleReceipt.test.ts src/features/application/project/projectLifecycleOperations.test.tsx
pnpm test:ts src/services/project/projectService.test.ts
pnpm check:ts
pnpm lint:ts
pnpm test:rs:package -p yss-project-registry-contract --lib
pnpm lint:rs:package -p yss-project-registry-contract --lib --tests
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md src-tauri/crates/yss-application/src/project/README.md src/tests/fixtures/project-event-wire/project-record.json
git -c core.safecrlf=false diff --check
```

2026-10-03 用户在收到七项项目管理清单后明确反馈“好的，都成功了”。据此记录用户人工复验通过：
新建；保存与重开；另存为并保留原项目；从列表移除后重新导入；清理失效登记且保留有效项目与文件；
临时项目移到回收站；在操作确认前取消。该记录来源为用户的真实桌面反馈。

项目管理基本流程已核销，无需重复要求用户测试；其余七组的后续回执如下。

## 2026-10-03：七组功能验收通过与本轮收尾

用户针对上一轮七组清单逐项回复，第 1 至第 7 组均“通过”。
据此记录画布与 Mind、资源编辑、工作台窗口与设置、数据与结果、Logs、Assistant、插件页面七组
功能人工验收全部通过。项目管理的七项基本流程继续沿用上一条用户回执。证据来源为用户实际操作反馈，
不将其描述为代理远程操作或新增自动测试。

复用前批两位子代理的只读收尾复核：没有新增具名功能验收项或尚未修复的代码缺口。
四处 Graph JSON 例外的同语义 ABBA 对照、行为验证与适用限制沿用前批证据；IV、Panel、White/IM、
HAC、ADF、项目管理 wire、真实采样中途取消等历史待办均由后续批次核销。
本轮仅更新本文，无生产修改、额外功能矩阵、重复 benchmark 或完整 CI。

本轮尚未取得 Performance 原始录制；工作区及常见导出目录的限定检索没有找到可用文件。
用户在收到唯一剩余项“画布 Performance 录制及分析”的说明后，明确回复“先跳过这部分吧”。
据此将该项从本轮必须完成的验收范围移出，状态为**未测，按用户要求跳过**。
这一范围调整仅适用于本轮收尾；规则文本和四处 JSON 例外的既有 benchmark 证据保持原范围。
实际脚本、layout、paint 和帧时间仍无本轮量化结论，后续如继续性能分析，应从原始录制取得证据。

已完成的架构审计、修复、受影响消费者检查、规则例外测量和用户功能验收统一保留在本文。
本轮以用户确认范围完成 goal 收尾，七组功能无需重测；仅更新文档并执行以下交付检查：

```powershell
pnpm test:ts src/tests/documentationContract.test.ts
pnpm format:check:ts docs/roadmap/ARCHITECTURE_RULES_REVIEW.md
git -c core.safecrlf=false diff --check
```

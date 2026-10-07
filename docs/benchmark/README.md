# 基准与测量数据

> Status: Current
> Scope: 数据引擎、Graph 编辑同步与实时解析的测量资料和复现入口
> Canonical owners: 基准源码定义测量方法；原始结果限定记录时的环境和范围
> Update when: 测量方法、命令入口或数据文件改变时

保留的结果是特定环境中的观察，不代表当前桌面端到端性能。再次测量时记录提交、构建配置、数据规模和机器环境；原始输出不随文档清理重新生成。

| 测量           | 源码与方法                                                                                                                           | 结果                                                                                                         |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------ |
| 数据引擎       | [查询、编辑、压实与 OLS 测量说明](DATA_ENGINE_BENCHMARK.md)                                                                          | [原始 JSON](DATA_ENGINE_BENCHMARK_RESULTS.json)                                                              |
| Graph 编辑同步 | [同步基准](../../react/src/tests/benchmarks/graphEditorSync.bench.ts)：完整快照与增量的 JSON 解码、投影安装和 Store 采纳，IPC 使用内存模拟 | [原始 JSON](probes/graph-editor-sync-results.json)                                                           |
| Graph 实时解析 | [Rust 基准](../../crates/yss-graph-runtime/src/resolution_tests.rs)：标量和 DataFrame 合成图的语义快照复用与投影生成       | [原始 JSON](probes/graph-resolution-results.json)                                                            |
| 项目快照准备   | [资源候选基准](../../react/src/tests/benchmarks/projectSnapshot.bench.ts)：共享当前状态与额外深拷贝的成本                                  | [测量及架构复核](../roadmap/ARCHITECTURE_RULES_REVIEW.md)、[原始 JSON](probes/project-snapshot-results.json) |

## Graph 编辑同步

从仓库根目录运行：

```powershell
pnpm bench:graph --outputJson docs/benchmark/probes/graph-editor-sync-results.json
```

原记录使用 100、1,000、5,000 节点合成图。结果不包含真实 Tauri IPC、Rust Resolve、DOM 绘制或业务图的完整 Schema 复杂度。

已有图再次接收完整快照的样本独立覆盖同值、单节点移动、节点及端口重排，每个节点包含两个端口。
`sync` 包括模拟 JSON 接收、冻结和校验；`adoption` 在计时外准备已验证的新输入；
`sync and adoption` 测两者合计。每个样本恢复同一已安装基线，计时外验证未变引用及接纳行为。
这些样本与原先清空图后加载的结果不可直接作为版本前后对照，也没有替代实现供规则例外比较。

```powershell
pnpm bench:graph --testNamePattern='^5000 nodes: existing graph'
```

2026-10-02 的 [原始输出与环境](probes/graph-existing-snapshot-2026-10-02.txt) 保留上述 9 组、
每组 30 次测量，结果及解释见 [架构复核记录](../roadmap/ARCHITECTURE_RULES_REVIEW.md)。

同日的 [重排身份对齐测量](probes/graph-existing-snapshot-aligned-2026-10-02.txt) 只选择 5000 节点
的三组 adoption，保留端口预扫描尝试、旧比较策略定位对照、局部计时探针及逐节点共享方案的输出。
样本、计时体与 hooks 保持相同；不同筛选组合、进程热身与 GC 会影响结果，不能直接用前一轮
九组进程的均值推断某段代码的成本。对照细节、源码恢复确认及局限以原始记录为准。

同日的 [Graph 常量读取镜像测量](probes/graph-constants-mirror-2026-10-02.txt) 记录移除前端完整
document 镜像后，同筛选的三组 adoption（每组 30 样本）。会话只保留常量读取事实，节点成员复用
已有实体索引；完整文档仍在 IPC 边界校验。计时体和 fixtures 不变，计时外 fresh-document 断言
改为比较初始接收对象。样本常量为空，前后数据是同机独立顺序进程的观察，不能外推大常量、
同步整体或桌面性能，也不是保留通用递归共享的默认方案对照或规则例外依据。

同日的 [非空常量边界参考测量](probes/graph-constants-immer-2026-10-02.txt) 在同一进程中比较
现有共享实现与 [一次 Immer 参考](../../react/src/tests/benchmarks/graphConstantSharingReference.ts)。
样本含 500 条常量，每条包含两个 16 项 List、嵌套元数据、说明和标签；分别测同值、仅改名称、
一个嵌套值改变。六组各 30 样本，计时包含共享和统一发布冻结步骤，JSON 物化、输入冻结、
完整文档校验及身份断言在计时外。文件总耗时 16,577 ms，完整命令、环境、源码 hash 和输出见原始记录。
该次参考对合法自有 `__proto__` 键会抛错，已从生产移除，该记录仅覆盖不含该类键的普通 JSON 样本。
其性能数据不构成完整同语义替代证明，不批准 constants 或其余 Graph 通用共享路径的规则例外。

```powershell
pnpm bench:graph --testNamePattern='^500 constants:'
```

同日的 [typed 常量外壳测量](probes/graph-constants-typed-immer-2026-10-02.txt) 保留上述 fixture、
准备与计时体，增加生产 `shareGraphConstants` 作为第三种策略：一次 Immer 处理常量表和固定字段，
任意 JSON 分支在 draft 外共享。九组各 30 样本，文件耗时 24,100 ms；typed 方案相对同进程 manual
的三个均值额外为 0.3054、0.3525、0.3553 ms。生产行为验证另覆盖合法自有 `__proto__`、删除及增量身份。
这个对照用于选择 typed 外壳实现；两者仍使用相同的任意 JSON 共享 helper，不能据此批准该递归边界的
规则例外。旧的六组记录和不完整参考的能力限制继续保留；顺序测量及进程环境限制见新原始记录。

同日的 [合法动态键常量对照](probes/graph-constants-dynamic-keys-2026-10-02.txt) 使用当前 typed 生产入口，
与修订后的一次 Immer 参考比较。500 条 Object 常量均包含深层自有 `__proto__` 和 `constructor`；
参考仅在含自有 `__proto__` 的字典子树复用现有共享 helper，避免将该键写入 Immer draft。
四场景各比较两策略，每组 30 样本：同值、仅名称、嵌套值及字典/数组变化、已有结构共享的增量。
生产均值依次为 4.5066、4.2612、4.3196、0.0713 ms，参考为 31.3138、31.4255、33.1509、0.1083 ms。
文件耗时 21,795 ms，命令整体 25.33 秒；完整 RME、环境、输出和前后九个相同源码 hash 见原始记录。
冻结、校验及内容/引用断言在计时外，计时包含共享和相同的发布冻结步骤。新增纯测试另覆盖保留键
整体替换与删除、可选字段删除、表格列及 delta 根身份。这是合法动态键样例的完整常量边界对照，
不是所有 JSON 的穷举或纯 Immer 替代；固定顺序、未隔离的其他进程和 GC 仍限制外推。
本次不采用所测整体参考替代生产入口，但差值不能全部归因于 dataValue，也不能批准参数、端口或
整个通用 helper 的例外。若进一步核销 dataValue，仍需保持相同 typed 外壳、只替换该遍历的直接对照。

```powershell
pnpm bench:graph --testNamePattern='^500 constants with dynamic keys:'
```

同日的 [dataValue 最小边界对照](probes/graph-constants-data-value-2026-10-02.txt) 保持相同 typed 常量外壳、
ValueType、表格处理和一次 `produce`，仅受控临时替换 dataValue 遍历。候选在普通 JSON 分支使用现有
Immer 事务，在自有 `__proto__` 字典子树复用安全共享。A/B/B/A 四个新进程各测同值和嵌套变化，
每组 30 样本；两轮生产均值分别为 4.3526/4.3318 和 4.4885/4.6064 ms，候选为
29.8407/30.2174 和 29.6478/30.2939 ms。完整 RME、四轮输出、临时 patch、候选 hash 和前后九个
源码 hash 见原始记录，总耗时 27.36 秒。生产源码按原字节恢复，未保留测试依赖或第二条策略。
候选的既有 4 文件 61 项行为检查及类型检查通过；名称、删除和共享增量只做行为验证，本次未计时。
该直接对照支持 constants.dataValue 保留现有 draft 外共享，不采用所测候选；费用包括其 draft/finalize
及相同发布冻结步骤，未单独测量特殊字典 fallback。有限样本、非隔离机器及顺序进程限制仍适用，
不能推断所有 Immer 方案、参数或端口边界，也不表示桌面性能。

```powershell
pnpm bench:graph --testNamePattern='^500 constants with dynamic keys: (equal|nested change) typed production$'
```

上述过滤在 A、B 轮显示相同的 `typed production` 名称，策略由原始记录中的轮次和源码 hash 区分。

同日的 [投影三个 JSON 边界对照](probes/graph-projection-json-2026-10-02.txt) 独立覆盖参数 `value`、
输入 `literalOverride` 和 `protocolDefault`。500 个合成节点各有三个独立普通 JSON 值，每值包含两段
16 项对象数组和深层自有 `__proto__`、`constructor`；没有使用序列化常量的变体形状。
候选只在原投影的一次 `produce` 内替换这三处共享算法，特殊字典继续走已有安全回退。
计时通过现有 `prepareGraphSessions`，包含其共享、实体准备及所需校验/冻结；JSON 物化、输入冻结与
校验、内容及引用断言在计时外。同值完整快照、首节点三个值变化、共享增量各在 A/B/B/A 中测量，
共 12 个任务、每任务 30 样本，总耗时 91.16 秒。两轮生产完整快照均值范围 25.8446–34.2264 ms，
候选为 257.85–267.07 ms；共享增量为 0.6882–0.7420 ms，未据其离散差异判断提速。
记录保留全部 12 组均值/RME、四轮 stdout、候选 patch 和前后 13 个源码 hash。候选 9 文件 89 项行为
检查及类型检查通过；生产已按原字节恢复，无测试依赖残留。该对照支持这三个字段作为同一投影 JSON
边界保留 draft 外共享；不区分每个调用的成本，也不把整体准备差值视作递归函数自耗时。
两轮生产存在明显漂移，机器未隔离非团队进程，样本也不代表实际图分布；没有外推桌面、其他模块或所有
可能的 Immer 实现。旧常量及空叶投影记录保持原样。

```powershell
pnpm bench:graph --testNamePattern='^500 nodes with JSON values:'
```

上述命令分别用于四轮进程；A/B 算法由原始记录中的轮次及源码 hash 区分。

同日的 [typed 投影拓扑测量](probes/graph-typed-topology-2026-10-02.txt) 记录一次 Immer 处理图根、
节点、端口和参数外壳后的当前安装费用。只测 5000 节点的 delta adoption 和三个完整快照 adoption；
原 `time=500`、最少 30 样本保持不变，delta 实际 181 样本，其余各 30 样本，文件耗时 40,970 ms。
delta 为每节点一个端口，完整快照为两个端口，不能横比为等量工作的加速比。move 完整快照均值
100.23 ms 的 RME 为 ±25.65%，包含 464.52 ms 长尾；该观察完整保留，不能视作稳定费用的精确估计。
此批没有旧策略控制，不从历史异 hash、异筛选进程推断加速或回退，也不豁免仍保留的 opaque 和复杂
typed 读取分支递归。完整命令、四组数据、环境、行为检查及前后七个源码 hash 见原始记录。
该记录对应 C1 的源码；后续 C2 将连接、诊断、Schema 等固定字段接入同一 Immer 事务，未重复运行
上述空连接/诊断/Schema 样本。C1 数字不代表 C2 当前源码的实测费用。

```powershell
pnpm bench:graph --testNamePattern='^5000 nodes: (delta adoption|existing graph .* snapshot adoption)$'
```

## Graph 实时解析

从仓库根目录显式运行忽略的性能用例：

```powershell
pnpm test:rs:package -p yss-graph-runtime --release --lib benchmark_repeated_resolution_and_projection '--' --ignored --nocapture
```

原测量的 scalar 图包含 100、1,000、5,000 个逻辑节点；DataFrame 图每 10 个节点组成数据源与 Limit 链，Schema 有 12 列。两种模式都预热节点缓存：对照模式丢弃完整快照，复用模式保留完整快照；类型和 Schema 缓存都保留。

该比较只说明完整语义快照复用的局部收益，不包含 Project 提交、结果有效性、Tauri 传输或 React 绘制，也不是所有优化相对旧版本的总体收益。

当前图与报告契约见 [Graph 与 Execution](../../crates/yss-application/src/graph/README.md)，完成记录与待验收事项见 [v0.3](../roadmap/v0_3.md)。

[返回文档索引](../README.md)

# Graph execution

> Status: Current
> Scope: 计划、调度、内核调用、运行状态与 ResultStore
> Canonical owners: 本 crate 源码拥有执行及结果存储；Application 协调资源授权与最终提交
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## Execute

`execute_graph` 接收编辑版本、`semanticInputHash` 与 demand，并从 Project 读取该版本的 document。Application 先校验文档并向 Project 准备资源授权，再捕获、解析及重验依赖，确认语义身份和可运行性；随后捕获结果发布依据、调用 Execution 准备计划及资源绑定。运行不隐式保存，也不回退磁盘旧文档。草稿或依赖变化返回 `graph_draft_changed`，阻断诊断返回 `graph_not_ready`，内部解析和计划构建故障保留诊断编号。

Demand selection 和 DAG scheduler 保留。`yss-node-kernel::KernelRegistry` 按 KernelId 向已注册实现传递 `KernelInvocation`；source node type 与 kernel identity 分开保留。参数使用具名完整集合，包含已解析默认值，普通 String 不按路径前缀猜成 Resource。计划中的 input slots 继续携带地址、实例组、预期类型和 coercion；顺序来自 snapshot 的 concrete port/connection order，package admission 校验 slot 与 specialization 一致。

Execution 的 `kernel_invocation` 在已授权的 PreparedRunResources 中解析资源参数，向 kernel 传运行值、固定端口/重复组的局部键、有序输出类型与字段、取消/deadline、预算及中立关系工厂。Application 装配时核对输入布局；调用时注册表复核布局和输出外层载体。Literal 与资源运行值可以借用，列表和记录使用不可变共享缓冲。Execution 将局部输出映射回 PlanOutputRef，并保留 lineage、category 与结果来源。

同一运行的 demand selection 和 producer 索引从准入传给调度器，不重复构建。最终结果在持有 ResultStore 写锁前已成为 `Arc<StoredResult>`，发布只增加引用。GraphAnalysis 的语义快照也按引用共享，展示投影修改时才取得独立内容。协议指纹显式排除展示字段，保留配置对象校验、条件和资源解释；不会递归删除用户数据中的同名字段。

内核的 `KernelError` 不携带图地址、运行阶段或项目状态；Execution 的 `OperationExecutionError` 负责节点定位，并保持已有 RunFailure 错误码和取消/超时终态。RuntimeValue、KernelId、KernelParameterKey 和 KernelFingerprint 由 Node Kernel 拥有，ResultStore 与结果租约仍属于 Execution。

每个 Output contract 保留类型、Schema/lineage、类别和 source identity；scheduler 按 output address 校验返回值，Results 使用该 output 的类别。Operation 不再拥有一个供所有 output 共享的类别。

协议默认参数在 semantic snapshot 中保留 typed literal。计划参数只区分已准备的 `Literal(Arc<RuntimeValue>)` 与待授权 `Resource`；标量、列表和记录在准备时构造一次，调用时借用。`DecimalLiteral` 在求值边界检查并转换为 `Float64`；已求值的数列算术直接传递 `TabularScalar`，不经过数字文本往返。过滤请求共用 Data Contract 的 `FilterLiteral` 保留精确十进制输入。

执行阶段使用 `ExecutePreparedError` 表达失败，`RunFailure` 携带稳定的 `RunFailureCode`、`RunPhase` 及 source identity，RunErrored 传递实际阶段、原因（如 divisionByZero、invalidNumericInput、nonFiniteResult）和节点；不传递原始输入值或后端错误文案。

`RunRegistry` 通过 `RunState` 记录运行状态和终态。成功执行通过 `ExecutionFinalizationHandoff` 将候选结果交给 Application 完成 finalization。

函数签名/正文依赖、调用环、Entry/Return 一致性已在 Resolve 中检查，初期拒绝递归。Root snapshot 按资源身份保存去重后的可达函数语义；GraphFunctionAbi 按 signature 顺序保留参数 ID、Entry output、Return input 和精确类型。实际函数子计划 lowering/execution 尚未接入，缺少实现时编辑解析明确阻断。Execution 不携带始终为空的函数包、另一套 FunctionPlanAbi 或未使用的 recursion_limit；通用执行包只持有实际计划、参数与来源依据。

加、减、乘、除按已解析 specialization 的元素类型和形状执行。标量 Int64 使用检查溢出的整数运算，

## ResultStore and cache validity

显式 `Outputs` demand 的 `reuse_inputs` 控制是否复用当前有效输入。Report 补选设为 true；普通 Execute 和原有输出运行保持 false。请求的输出生产者始终执行，受它们影响的下游也不能复用；其他依赖仅在同一生产者全部输出有效时被跳过。调度以共享结果填充输入槽，RunStarted 只失效实际重算的输出。
ResultStore 在准入时校验复用结果的 ID 与当前缓存一致，并记录实际消费的源结果。发布时再次检查这些输入仍有效且 ID 未变；另一次运行替换输入、图编辑或资源失效后，旧补算不能发布。复用没有单独的结果存储，也不改变常规运行的随机节点行为。

`ResultStore` 是 session-scoped result authority，分别维护当前 output address 索引和不可变结果记录。

图输入更新与对应结果有效性摘要在同一写锁内完成。摘要携带执行会话内单调递增的 `revision`，运行准入、结果发布和依赖重验都推进这一顺序；它独立于图编辑 revision，用于拒绝迟到的旧结果投影。读取摘要不复制结果 payload，既有图缓存有效性与租约规则继续由 ResultStore 执行。
结果以 `{ executionSessionId, resultId }` 标识，保留 type/presentation、payload 与生成时的 provenance。
`StoredResult` 保存 `RuntimeValue`、输出类别和生成时的 `PlanOutputContract`，让已保留结果的类型与 Schema 不依赖当前图或重新推断数据。
`StoredResultSnapshot` 表示共享结果的一致性读取视图；这一机制称为结果缓存与持有租约，不提供历次运行归档。
当前输出和显式报告租约是结果的持有者；输出不再指向结果且最后一个租约释放后，移除结果索引。
计算中的查询通过临时 `Arc` 保证内存安全，最后一个共享引用释放后回收实际数据；不依赖周期性 GC 或前端计数。

每个现存输出最多持有一份结果缓存。缓存持有与有效性分开：语义编辑保留旧值，但只让依据仍匹配的输出参与当前查询。
Graph 提供包含参数、类型、输入绑定与 coercion 的节点指纹；Application 映射资源版本，Execution 记录实际消费的上游结果身份。
依赖检查沿数据关系传播，不因整图 hash 改变而统一释放结果。撤销重新 Resolve 后仅恢复仍存在且依据匹配的缓存；
已删除输出、重算准入或显式清理释放的结果不会被撤销重新创建。

## 相关模块

科学计算适配属于 Node Kernel。报告读取不经过 Execution 的临时计算入口；本 crate 的 SCI runtime/contract 依赖仅用于 `ols_bench` 示例，不进入生产依赖。

[内核与数值/关系操作](../yss-node-kernel/README.md) · [Results 查询与租约](../../../src/features/application/results/README.md) · [Application 编排](../yss-application/src/graph/README.md)

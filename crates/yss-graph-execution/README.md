# Graph execution

> Status: Current
> Scope: 计划、调度、内核调用、运行状态与 ResultStore
> Canonical owners: 本 crate 源码拥有执行及结果存储；Application 协调资源授权与最终提交
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## Execute

`state.rs` 持有会话的计划、结果和运行 registry。内部 `state/admission` 管理工作租约、
关闭与排空；`state/control` 管理单次运行的取消和 deadline；`state/dispatch` 编排
准备资源、执行及生成结果候选；`state/scheduler` 和 `selection` 负责数据 DAG 和 demand。
`error` 拥有执行错误及 RunFailure 映射，原 `state` 公开类型路径继续可用。

`state/scheduler/groups` 顺序读取中立分组会话，把每组 DataFrame 绑定到同一捕获函数定义的私有帧。
它复用 `functions` 的 ABI 校验、嵌套调度和动态 Schema 阶段；组内值不进入全局 ResultStore。
空来源调用一次空表作为输出结构探测，最终组合仍为零行。分组、函数及输出写入共享运行取消、deadline
及无固定内存上限的执行控制。稳定分组结果只冻结其源表并保留键，不提前调用组函数；后续 CurrentInputs 可复用该值。
`RunFailure.groups` 按外层到内层保留调用位置、函数路径和一基组编号，None 表示空输入探测；
主 `source` 保留实际函数内部错误位置。取消和期限失败保持原终态处理，不被组上下文包装改变语义。

运行记录与取消对象统一由 `RunRegistry` 持有，取消请求与终态转换使用同一把锁。
取消对象覆盖 Admitted、Running、Finalizing，在 Succeeded、Failed 或 Cancelled 时释放；
运行通知或执行过程 unwind 时，`state/run_lifecycle` 的守卫将未交付的运行转为 Failed。
守卫只持有必须完成状态转换的责任，不复制运行状态。

`state/active_run` 持有一次运行的工作租约、取消控制和生命周期守卫。单阶段执行与动态 Schema
连续执行复用同一调度入口；各阶段共用 RunId、开始时间和期限。已发布阶段只记录结果身份，
下一阶段从 ResultStore 读取并重验这些值。阶段之间的取消、输入替换或编辑不能继续使用旧依据。

关闭准入和登记运行通过同一准入锁协调，锁顺序为 admission → run registry；
锁内只做登记、取消标记或状态转换，不准备资源、不调用 kernel，也不交付事件。
`cancel_and_drain` 关闭准入并请求取消已登记运行，再等待工作租约释放。
`close_admission` 仅关闭准入。排空在每次唤醒后重验工作数，所有工作释放后返回 Drained。

`execute_graph` 接收编辑版本、`semanticInputHash` 与 demand，并从 Project 读取该版本的 document。
Application 校验文档、捕获解析与语义身份后，Execution 的 `GraphExecutionScope` 按目标输出选择必要上游；
全图请求直接选择所有语义节点，单节点请求按节点身份遍历依赖，只有显式输出请求构建输出地址索引。
范围就绪性由 Analysis 检查，资源授权只消费这些节点及可达函数的语义资源引用，普通路径字符串不作为资源。
`prepare_graph_package` 只构造选中范围的操作、参数及输出，缓存与计划身份包含该范围。
Application 重验依赖、捕获结果发布依据并准备资源绑定。运行不隐式保存，也不回退磁盘旧文档。
草稿或依赖变化返回 `graph_draft_changed`，范围内阻断诊断返回 `graph_not_ready`，内部解析和计划构建故障保留诊断编号。

`PlanBasis::resource_observations` 是执行包中资源存在性与版本的唯一依据：Present 必须携带版本，
Absent 保留可选的删除版本。Application 从同次 Project 授权构造它；资源准备按该观察核对绑定版本、
项目会话与必需/可选要求，不再并行保存一份资源版本映射。Project 的最终授权重验保持独立。

`GraphExecutionScope::schema_frontier` 从范围内被消费的 Deferred / Observed 输出选择最早可执行的
结构边界。Application 发布这些稳定结果并重新解析后继续准备计划。“运行至此”可复用有效边界，
全图重跑先重新求值动态结构；同一次运行已完成的输出即使是后续需求根也不重复计算。
Observed 只描述上一份结果，新求值的输出契约不强制沿用旧字段；消费者在边界完成后按实际结构准备。
CurrentInputs 不触发这种补算。结构边界之前已经成功发布的结果在后续阶段失败时仍可查看，
未成功的下游不发布结果，整个运行只交付一个终态。

Demand selection 和 DAG scheduler 保留。`yss-node-kernel::KernelRegistry` 按 KernelId 向已注册实现传递 `KernelInvocation`；source node type 与 kernel identity 分开保留。参数使用具名完整集合，包含已解析默认值，普通 String 不按路径前缀猜成 Resource。计划中的 input slots 继续携带地址、实例组、预期类型和 coercion；顺序来自 snapshot 的 concrete port/connection order，package admission 校验 slot 与 specialization 一致。

Reroute 是计划中的透明转交操作，保留节点、端口、demand 和结果身份，不请求计算内核。
准入要求一个 Value 输入与同类型输出，禁止参数、coercion 和独立观察意图。
`state/scheduler/reroute` 沿现有 producer 索引把求值边界传到实际生产者，DAG 在那里物化一次，
转接链共享同一值/关系句柄，不逐节点重新扫描或复制表格。未要求求值的中间关系继续保持惰性；
有效缓存、CurrentInputs、编辑失效和撤销恢复沿用 ResultStore，函数私有帧复用同一 DAG 行为。

Execution 的 `kernel_invocation` 在已授权的 PreparedRunResources 中解析资源参数，向 kernel 传运行值、固定端口/重复组的局部键、有序输出类型与字段、取消/deadline 及中立关系工厂。Application 装配时核对输入布局；调用时注册表复核布局和输出外层载体。Literal 与资源运行值可以借用，列表和记录使用不可变共享缓冲。Execution 将局部输出映射回 PlanOutputRef，并保留 lineage、category 与结果来源。
输入 coercion 保留于计划校验，标量广播由 Kernel 执行，调度器交付原值。

`PlanValidationControl` 按已确认的保留决定继续归属本模块，当前尚未接入计划准入；计划校验不检查它的 deadline。

`RunExecutionControl` 只携带取消和 deadline。调度器通过 `KernelControl::new` 为所有节点创建
无固定内存上限的执行控制，输入读取、工作区、结果编码、函数私有帧、分组调用和快照共享该策略。
尺寸溢出与实际分配失败继续由原错误入口交付，取消和 deadline 独立生效。

同一运行的 demand selection 和 producer 索引从准入传给调度器，不重复构建。最终结果在持有 ResultStore 写锁前已成为 `Arc<StoredResult>`，发布只增加引用。GraphAnalysis 的语义快照也按引用共享，展示投影修改时才取得独立内容。协议指纹显式排除展示字段，保留配置对象校验、条件和资源解释；不会递归删除用户数据中的同名字段。

内核的 `KernelError` 不携带图地址、运行阶段或项目状态；Execution 的 `OperationExecutionError` 负责节点定位，并保持已有 RunFailure 错误码和取消/超时终态。RuntimeValue、KernelId、KernelParameterKey 和 KernelFingerprint 由 Node Kernel 拥有，ResultStore 与结果租约仍属于 Execution。

每个 Output contract 保留类型、Schema/lineage、类别和 source identity；scheduler 按 output address 校验返回值，Results 使用该 output 的类别。Operation 不再拥有一个供所有 output 共享的类别。

结果边界将同行域中同一数列表达式的多个输出引用一起冻结；它们共享同一稳定列，避免重复求值。
同名但不同表达式仍独立冻结，列名不能作为表达式身份。

View 的观察意图引用已连接输出的结果，保留各个请求节点身份；多个 View 可观察同一输出。
其禁止内联值和默认值的输入策略由 Catalog 声明，Analysis 在计划准备前阻断非法输入，Execution 不为 View 另造结果值。

协议默认参数在 semantic snapshot 中保留 typed literal。计划参数只区分已准备的 `Literal(Arc<RuntimeValue>)` 与待授权 `Resource`；标量、列表和记录在准备时构造一次，调用时借用。`DecimalLiteral` 在求值边界检查并转换为 `Float64`；已求值的数列算术直接传递 `TabularScalar`，不经过数字文本往返。过滤请求共用 Data Contract 的 `FilterLiteral` 保留精确十进制输入。

常量参数直接消费节点语义事实，按常量 ID 对应的计划参数句柄只物化一次；多个引用节点绑定同一 payload，
不重复展开相同常量的表格单元。该复用属于当前计划参数集合，不另建运行时常量缓存。

执行阶段使用 `ExecutePreparedError` 表达失败，`RunFailure` 携带稳定的 `RunFailureCode`、`RunPhase` 及 source identity，RunErrored 传递实际阶段、原因（如 divisionByZero、invalidNumericInput、nonFiniteResult）和节点；不传递原始输入值或后端错误文案。

`RunRegistry` 通过 `RunState` 记录运行状态和终态。按完成顺序保留最近 1024 个终态，超过上限时淘汰最早完成者；Admitted、Running 和 Finalizing 不参与淘汰。保留期内取消请求仍区分 AlreadyCancelled / AlreadyTerminal，淘汰后返回 NotFound。该策略只作用于运行元数据，结果仍由 ResultStore 的租约规则管理。成功执行通过 `ExecutionFinalizationHandoff` 将候选结果交给 Application 完成 finalization。

同一运行记录使用单调时钟累计 Admitted、Running、Finalizing 的墙钟耗时，状态与计时在同一锁内读取。
活动阶段的快照包含当前经过时间，所有终态固定计时；淘汰后明确不可用。Running 包括运行内的调度、
资源准备和结果物化，不表示纯内核计算时间，也不包含准入前等待或 Harness 工具往返时间。

函数签名/正文依赖、调用环、Entry/Return 一致性由 Analysis 检查，递归仍被拒绝。
`PlanNodeSpecialization` 区分内核与结构操作，Call 按 Analysis 提供的参数 ID/端口绑定调用，
不依赖端口标题或排序猜测形参。Entry/Return 只能在已绑定的私有调用帧内执行，公开计划准入拒绝独立结构入口。

`function_library` 将同一 Analysis 的函数事实、冻结 Registry 和捕获的资源事实附到执行包，校验 Registry 及依赖依据；
没有函数的计划不携带函数库。正文不是运行值，资源读取仍只使用本次授权的 PreparedRunResources。
`state/scheduler/functions` 绑定实际值及列结构，`functions/frame` 调用 Analysis 特化并复用原计划构建和
`scheduler/dag` 求值。每次调用私有地保留参数、中间值及动态 Schema 阶段；不创建执行会话或向全局 ResultStore
发布函数内部地址。嵌套调用共用取消、deadline 和同一内核执行控制，返回值再按调用节点的结果边界求值与发布。
调用中的动态列同样先求值再解析下游，实际缺列保留函数路径及内部引用节点，返回 PlanValidation 阶段失败。
函数定义和资源变化继续通过原捕获重验及结果依赖使旧调用失效。

加、减、乘、除按已解析 specialization 的元素类型和形状执行。标量 Int64 使用检查溢出的整数运算。

## ResultStore and cache validity

显式需求输出、View 观察值及已保留的结果边界在运行成功前求值。关系和数列通过 RelationFactory 的
流式快照写入引擎临时存储；同一节点、同一行域的多列共同保存，实际结构随不可变值保留。
批量内部关系表达式继续组合优化。ResultStore 保留内部依赖身份，但未求值的表达式不出现在当前结果查询、
缓存完成摘要或公开运行回执中，也不参与“运行本节点”的输入复用。已保留的边界在后续批量运行中继续求值，
避免以内部表达式覆盖上次成功值；快照失败或取消走现有失败与旧值保留流程。

`Node` demand 的 `CurrentInputs` 只执行目标节点，必要上游结果缺失或过期时返回带源节点/端口的 `InputResultUnavailable`，不启动隐式补算。
`Dependencies` 执行目标并补算必要依赖，复用当前有效输入；`Default` 全图按依赖重新执行。
View 没有输出，节点 demand 只观察它连接的值；读取已有结果也会在最终发布时核对观察者输入依据与结果身份。
显式 `Outputs` demand 的 `reuse_inputs` 控制是否复用当前有效输入。Report 补选设为 true；普通 Execute 和原有输出运行保持 false。请求的输出生产者始终执行，受它们影响的下游也不能复用；其他依赖仅在同一生产者全部输出有效时被跳过。调度以共享结果填充输入槽，RunStarted 只失效实际重算的输出。
ResultStore 在准入时校验复用结果的 ID 与当前缓存一致，并记录实际消费的源结果。发布时再次检查这些输入仍有效且 ID 未变；另一次运行替换输入、图编辑或资源失效后，旧补算不能发布。复用没有单独的结果存储，也不改变常规运行的随机节点行为。

`ResultStore` 是 session-scoped result authority，分别维护当前 output address 索引和不可变结果记录。
`result_store.rs` 拥有唯一的 registry 与锁；内部 `cache` 管理输入依据及依赖有效性，
`publication` 管理运行准入和结果批次发布，`retention` 管理租约、窗口交接与回收，
`projection` 只读取当前结果及有效性摘要。这些模块共用同一状态，不建立独立缓存或同步流程。
发布先校验完整批次，再在同一写锁内安装结果和实际消费记录。

重算准入使目标缓存及其依赖失效，但继续持有上一次成功值；当前运行的待发布输入依据与成功值的生成依据分开。
失败、取消或被替代的运行不发布新值，旧值可通过已持有的完整结果身份读取，不能作为当前有效输入。
新结果成功发布后才替换缓存持有；旧窗口租约继续有效，最后一个持有者释放时回收。
有效性摘要的 `Stale` 和 `Valid` 均带各自保留的 `result_id`，`Missing` 不带身份。
显式查看旧值可以据此取得普通结果租约；当前输出查询及执行复用仍只接纳 `Valid`。

Schema 反馈读取同一 ResultStore 的已求值结果，没有额外可写列目录。候选生成依据和当前图输入通过与缓存
相同的依赖检查比较；同输入重跑期间可保留上次成功 Schema，但数据仍是过时结果，不能作为当前输入复用。
运行准入在锁内重验解析采用的结果 ID，拒绝已替换的观测。纯 Schema 反馈只撤销输入变化的在途输出，
文档与资源定义变化仍撤销旧运行的发布权限。结果有效性和观测匹配共用一次依赖传播实现。
编辑与资源变化撤销待发布运行的权限，不把未完成的重算恢复为有效旧值。

图输入更新与对应结果有效性摘要在同一写锁内完成。摘要携带执行会话内单调递增的 `revision`，运行准入、结果发布和依赖重验都推进这一顺序；它独立于图编辑 revision，用于拒绝迟到的旧结果投影。读取摘要不复制结果 payload，既有图缓存有效性与租约规则继续由 ResultStore 执行。
`ExecutionRuntimeState::result_revision` 在同一 registry 读锁内读取该顺序，不构造逐输出摘要。Application 在运行事件产生时捕获它；相同会话中达到该版本的摘要已包含事件前的结果变更。恢复交付保留事件原有版本，不用重放时的当前值替换。
结果以 `{ executionSessionId, resultId }` 标识，保留 type/presentation、payload 与生成时的 provenance。
`StoredResult` 保存 `RuntimeValue`、输出类别和生成时的 `PlanOutputContract`，让已保留结果的类型与 Schema 不依赖当前图或重新推断数据。
`StoredResultSnapshot` 表示共享结果的一致性读取视图；这一机制称为结果缓存与持有租约，不提供历次运行归档。
`query_result_with_validity` 在同一 registry 读锁内交付保留值及其当前身份状态，区分当前有效、当前过时和仅被租约保留的旧结果。
`query_graph_result_entries` 默认只列当前 Pin（含过时值），指定 RunId 时读取仍被持有的该次运行结果；不新增运行归档，也不复制 payload。
当前输出和显式报告租约是结果的持有者；输出不再指向结果且最后一个租约释放后，移除结果索引。
写入事务在锁内收集已移除的结果，释放 registry 锁后再释放这些值及其关系句柄；unwind 也遵守该释放顺序。
计算中的查询通过临时 `Arc` 保证内存安全，最后一个共享引用释放后回收实际数据；不依赖周期性 GC 或前端计数。
会话不提供重置整个 ResultStore 的入口；图失效、租约释放和会话结束负责各自的清理，
同一会话内的摘要 revision 与已关闭 owner 记录不会被清空重置。

每个现存输出最多持有一份结果缓存。缓存持有与有效性分开：语义编辑保留旧值，但只让依据仍匹配的输出参与当前查询。
Graph 提供包含参数、类型、输入绑定与 coercion 的节点指纹；Application 映射资源版本，Execution 记录实际消费的上游结果身份。
依赖检查沿数据关系传播，不因整图 hash 改变而统一释放结果。撤销重新 Resolve 后仅恢复仍存在且依据匹配的缓存；
已删除输出或显式清理释放的结果不会被撤销重新创建；重算保留的上次成功值仍遵循上述过时与持有规则。

## 相关模块

科学计算适配属于 Node Kernel。报告读取不经过 Execution 的临时计算入口；本 crate 的 SCI runtime/contract 依赖仅用于 `ols_bench` 示例，不进入生产依赖。
Database Engine 仅作为数列快照回归的测试依赖，用真实关系表达式与数据验证重复引用；不进入生产依赖。

[内核与数值/关系操作](../yss-node-kernel/README.md) · [Results 查询与租约](../../react/src/features/application/results/README.md) · [Application 编排](../yss-application/src/graph/README.md)

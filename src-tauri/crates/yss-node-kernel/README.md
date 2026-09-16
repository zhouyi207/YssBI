# yss-node-kernel

> Status: Current
> Scope: 不依赖 Graph 的内核调用契约、运行值、冻结注册表和内置节点执行适配
> Canonical owners: 本 crate 源码拥有内核能力；图计划、资源授权、结果生命周期由 [Graph 与 Execution](../../../docs/architecture/GRAPH_AND_EXECUTION.md) 定义
> Update when: 内核调用契约、注册/指纹规则、控制边界或内置适配改变时

本 crate 持有可执行函数。Node Protocol/Registry/Catalog 持有节点声明和目录；Application 的 `NodeComponents` 核对声明与内核契约，冻结一份注册表，供同一会话的能力检查和实际执行共同使用。

## 调用边界

`KernelInvocation` 接收已求值输入、输入组、有序输出类型/字段、已解析参数及 `KernelControl`。参数可以借用现有 literal 和已授权的资源运行值，避免仅为构造调用就复制完整数据。内核返回按调用局部输出顺序排列的 `Vec<RuntimeValue>`。

输出契约通过 `ValueType` 引用七种 Semantic，数值端口统一为 Numeric。四则运算根据实际标量或
Arrow 字段的 Physical 选择整数/浮点表示，除法使用浮点表示，有损提升会被拒绝。
比较节点共享精确 Numeric 比较，整数比较不经过浮点转换，混合表示比较不使用 epsilon。
Equal/NotEqual 对列表和记录递归应用该规则，其他运行值维持原有值或资源身份相等语义。
Graph 的广播说明不提前改写标量值。数值、转换、比较和关系筛选适配的行为变化会推进实现 revision。

Execution 的 [kernel_invocation.rs](../yss-graph-execution/src/kernel_invocation.rs) 负责从计划生成这些信息。它在准备好的资源绑定中解析资源参数，保留图端口地址、Schema 血缘和结果类别，并将返回值映射到对应输出。内核不接收 `GraphDocument`、`PlanOutputRef`、项目状态或资源授权服务。

`KernelError` 只表达计算错误、取消和超时。Execution 的 `OperationExecutionError` 负责图来源与阶段，并继续投影已有 `RunFailure` 错误码。ResultStore、结果引用、租约、保存和运行生命周期仍属于其原所有者。

## 模块

| 模块                                     | 职责                                                   |
| ---------------------------------------- | ------------------------------------------------------ |
| [identity](src/identity.rs)              | KernelId、参数键及能力指纹                             |
| [invocation](src/invocation.rs)          | 中立调用、输出元数据、取消/deadline 和计算预算         |
| [value](src/value.rs)                    | 标量、列表、记录、关系/数列句柄与原生 OLS 运行值       |
| [registry](src/registry.rs)              | 注册一致性、冻结指纹、能力查询、执行契约与输出数量检查 |
| [builtins](src/builtins/mod.rs)          | 内置注册装配及逻辑、常量、转换等适配                   |
| [numeric](src/builtins/numeric.rs)       | 已有标量/数列四则运算                                  |
| [relational](src/builtins/relational.rs) | 已有 DataFrame/数列关系操作和按输出 Schema 拆列        |
| [statistics](src/builtins/statistics.rs) | 已有 OLS Fit/Summary 到 SCI Runtime 的适配             |

科学计算继续调用 `yss-sci-runtime`，数值算法属于 SCI，关系执行使用 `yss-relational-contract` 的句柄。这里不直接依赖 Graph、Project、Application、Tauri、DataFusion 或 Linalg。

## 注册与扩展

`KernelRegistryBuilder::register` 接收 KernelId、非零实现 revision、KernelContract 和执行函数。KernelContract 声明实际参数键集合及输出数量范围；它不复制 Catalog 的分类、本地化文本或完整配置模型。

`with_builtins` 组合已有内置实现；Application 可以在冻结前加入扩展。冻结后所有调用使用同一能力指纹。指纹继续采用 `yssbi.kernel-registry.v1` 编码，覆盖排序后的 ID、revision、参数键和输出数量。尚未接入的分布采样等节点仍返回缺少执行能力的诊断。

新增节点时在对应方法族模块实现适配，再加入内置装配或由应用 provider 注册；实际算法放回 SCI 或相应数据所有者。执行函数只使用已经解析的类型和资源，不重新求解图类型，也不自行读取项目资源。

实现行为变化需要递增 revision。参数或输出形状变化需要同步节点声明和消费者，并复核解析与计划缓存的能力身份。透明重路由不产生执行操作；内置表中保留的同名身份不改变其 Graph 语义。

## 验证

已有内置指纹作为缓存契约验证；独立调用测试检查拆列输出顺序。Application 的扩展注册、配置、数值执行和结果查询测试继续覆盖跨边界行为。运行命令与范围选择见[本地工作流](../../../docs/development/LOCAL_WORKFLOW.md)。

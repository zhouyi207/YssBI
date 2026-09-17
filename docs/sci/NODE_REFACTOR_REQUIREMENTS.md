# 节点组件与图操作 API 重构要求

> Status: Planned
> Scope: PORTS/KERNEL/PARAMETERS/DATA/OPERATORS 的独立组件与节点装配、基础与复合类型、目录展示分离、端口级操作 API、Command 提交和 undo/redo
> Canonical owners: 本文拥有本轮确认的通用节点重构要求；当前实现由源码及 ../architecture/GRAPH_AND_EXECUTION.md 拥有；OLS 模型行为由 OLS_SPEC.md 拥有
> Update when: 组件契约与装配、类型及转换规则、展示信息归属、操作接口、提交边界、历史或验收约定改变时

本文作为后续代码重构的要求和验收依据。文中的组件类型、装配及链式编辑接口均是目标设计，不表示已经实现；`yss-node-kernel` 的分离已经完成，当前实现说明见其 [README](../../src-tauri/crates/yss-node-kernel/README.md)。

本规范细化[组件化重构计划](../draft/component-plan.md)中的节点及图编辑边界，并作为 [OLS 首批规格](OLS_SPEC.md) 的通用基础。实现时保留已有输入输出和编辑行为，按本文明确调整接口、信息归属及事务组织；不同时新增统计方法或替换数据引擎。

## 1. 已确认的目标

| 编号 | 要求                                                                                                                                  |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------- |
| R1   | 节点级标题、文档、别名、图标和目录位置在 Catalog 注册时统一提供                                                                       |
| R2   | PORTS 和 PARAMETERS 的完整契约留在各自组件中；端口默认英文，无 title/description/i18n，参数文案仍随参数定义维护                       |
| R3   | PORTS、KERNEL、PARAMETERS、DATA、OPERATORS 是可独立构造、校验、复用和组合的组件；节点通过装配组件定义                                 |
| R4   | 直接提供 `graph.port(a).connect(b)` 等对象操作接口，不暴露 `.operators()` 层                                                          |
| R5   | 修改方法只准备候选变更；`commit()` 原子提交一个可撤销 Command                                                                         |
| R6   | 同一链中的操作保持当前对象身份；端口 a 连续连接 b、c 表示 a→b 和 a→c                                                                  |
| R7   | 图编辑状态由所属 Project/Graph 管理，结果在执行会话内共享 ResultStore；不新增全局可变 OperatorStore                                   |
| R8   | Kernel 读取输入和参数、产生输出数据；端口结构与类型/Schema 解析仍由图编辑及分析所有者负责                                             |
| R9   | 复用现有版本检查、回执、可逆补丁、结果失效和前端单向投影机制                                                                          |
| R10  | 面向用户的基础类型限定为 Numeric、Categorical、Ordinal、Binary、Datetime、Text、Identifier；复合类型只保留 DataSeries<T> 和 DataFrame |
| R11  | Identifier 表达标识变量；无论底层是否为数字，都不得自动进入均值、相关系数、OLS 等统计计算的分析变量输入                               |

组件组合是本轮必须落实的内部模型。节点提供身份并关联组件；端口声明、参数契约、数据访问和行为绑定分别由组件拥有，分析、执行和编辑系统按所需组件工作。仅给现有 NodeProtocol 的字段分类，或增加一个包揽所有配置方法的 NodeDefinition Builder，不能满足 R3。

优先在现有 crate 中实现节点领域的类型化组件和 bundle 装配。组件模型与是否选用 ECS 库分开决定；本轮不需要引入全局 World、任意类型容器或服务定位器。NodeId、NodeTypeId 和 PortAddress 继续表达稳定身份。

## 2. 节点定义与目录展示

### 2.1 节点定义保留的内容

- 稳定的 NodeTypeId，以及 Kernel、结构节点或透明节点的行为绑定。
- 统一端口集合中每项的 key、direction、类型、连接容量、动态模板及局部类型/Schema 约束；输入和输出通过 direction 区分。
- 端口的类型化编辑配置；不设置独立的 title、description 或文案 key。
- 参数的 key、类型、默认值、约束、配置分组、编辑方式，以及参数自己的名称、说明和本地化 key。
- 确定性、缓存和数据消费/产出要求，以及适用图类型、自动管理角色等语义约束。

参数定义与节点实例的实际参数值分开：共享定义描述允许的配置，图文档保存用户的配置。数据字段名、枚举机器值、参数路径等有语义的字符串继续属于对应契约。

### 2.2 Catalog 注册的内容

Catalog 引用已注册节点的 NodeTypeId，集中登记节点级标题、帮助文档、描述、别名、图标、样式、分类及排序/可见性信息。端口和参数不在目录树中重复登记，也不新增 `input_labels`、`output_labels`、`parameter_labels` 镜像。

对外可以在构建分类树时一次登记节点描述；对内按 NodeTypeId 建立描述索引，分类树维护放置关系。画布、详情、搜索和不在创建列表展示的节点都通过该索引查询，避免依赖遍历整棵树获取标题。

静态节点标题来自 Catalog，用户给图中实例设置的 `user_label` 仍属于图文档；资源名称、列名称等动态信息仍由资源/Schema 提供。目录调整不修改节点 ID、端口 key 或参数 key。

端口不做 i18n，默认使用英文。固定端口直接显示稳定的英文 key，例如 `samples`、`value`、`result`；不查询本地化词条，也不随界面语言切换改变标签。

动态端口优先原样显示已有资源字段名或实例标签，缺少标签时回退到 key。资源和用户提供的名称保留原文，不翻译或强制改为英文。key 仍是端口身份，不能为了修改显示名称而改 key；不增加目录中的端口文案映射表。

当前 [Catalog 端口投影](../../src-tauri/crates/yss-node-catalog/src/localization.rs) 和[图端口投影](../../src-tauri/crates/yss-graph-analysis/src/port_projection.rs) 会读取 PortSpec.title 作为显示标签。代码迁移时须一并调整这些消费者和第 2.4 节的指纹编码；移除该字段后，固定端口的默认显示文字将采用上述 key 规则。

### 2.3 先定义组件，再装配节点

Rust 接口必须直接表达组件组合。以下示意先独立构造五个组件，再将它们作为一个类型化 bundle 注册；节点没有 `.output()`、`.configuration()` 或 `.kernel()` 之类跨组件配置方法。组件内部可以使用构造函数或 Builder，其修改范围只限于该组件。

类型名和错误形式是提议，尚不是可调用的生产 API。正态采样继续使用当前 `configuration` 分组；本示例不表示其执行内核已经实现。

```rust
fn example_ports() -> Ports {
    Ports::new([
        Port::new("samples", PortDirection::Out, Type::DataSeries(Box::new(Type::Numeric))),
        Port::new("samples", PortDirection::In, Type::DataSeries(Box::new(Type::Numeric))),
        Port::new("samples", PortDirection::Out, Type::DataSeries(Box::new(Type::Numeric))),
    ])
}

fn example_parameters() -> Parameters {
    Parameters::new([
        ParametersGroup::new("配置组 1", [
            Parameter::number("mean").float().default(0.0),
            Parameter::number("standard_deviation").float().positive().default(1.0),
            Parameter::number("sample_count").int().positive().min(1).default(100),
        ]),
        ParametersGroup::new("配置组 2", [
            Parameter::number("mean").float().default(0.0),
            Parameter::number("standard_deviation").float().positive().default(1.0),
            Parameter::text("sample_count").default("你好"),
        ]),
    ])
}

definitions.register(
    Definition::new("xxx::xxx::xxx::xxx::example")
        .aliases(["example", "example_alias"])
        .documentation(documentation(zh, en))
        .ports(example_ports())
        .parameters(example_parameters())
        .data(example_data())
        .kernel(example_kernel())
        .operations(example_operations())
)?;

definitions.register(
    "xxx::xxx::xxx::xxx::example", ["example aliases 1", "example aliases 3", "example aliases 3"], documentation(zh, en)
    ports, parameters, data, kernel, operations
)?;
```

`register` 的第二个参数是节点领域的组件 bundle。示例用固定类型的元组表达装配边界；实现可以使用具名 bundle，但必须保留组件的独立类型、校验和复用能力。注册后各系统读取所需组件，不能又将它们展开复制成一份独立可修改的 NodeProtocol。

`Data::port_values()` 声明通过已捕获的端口值参与计算，不创建 ResultStore，也不复制端口类型。`Operators::standard()` 选择标准图编辑处理器及其支持的操作，不在每个节点里创建一套编辑器。`Kernel::bind` 只建立实现引用，实际可执行性由冻结的 KernelRegistry 判断；缺少实现时仍允许编辑节点并报告未就绪。

`normal_ports()` 和 `normal_parameters()` 可分别用于装配和组件契约校验；替换为兼容的 Kernel 只更换对应组件，其他组件无需重新定义。组件在定义装配阶段组合和替换，注册冻结后只读；实例编辑仍通过第 4 节的图操作入口。

端口的 `DataSeries<Numeric>` 表达 Numeric 语义要求，不将其固定绑定到 Int64 或 Float64。Physical 与 Semantic 在数据 Detail 中分别选择；节点仍可声明计算所需的约束，例如 sample_count 必须为整数。具体选择规则见第 3.7 节。

示例中的节点及参数文案可以使用本地化 key。节点级文案 key 属于目录描述，参数文案 key 随参数组件维护；对应语言资源继续统一加载。端口 key 是机器标识，不是本地化文案 key。

### 2.4 校验与指纹

- 组件先校验自身契约，节点装配再校验组件间的端口/参数引用、数据访问要求和行为适用性。重复或冲突组件、无效引用及可静态判定的类型冲突必须拒绝；Kernel 实现暂缺与非法组合分别报告，不能混为一种注册失败。
- Catalog 校验节点引用、分类树及展示资源，不让分类和节点级文案成为执行契约的依赖。
- 注册组合必须能发现未知 NodeTypeId、重复描述、无效分类引用及需要展示的文案缺口。节点缺少树位置不等于语义定义无效。
- 语义指纹排除纯展示字段，包括节点文案和参数说明；端口 key 及其他影响绑定、默认值、合法性、类型或 Schema 的内容必须继续参与语义身份。
- 展示身份必须覆盖实际使用的文案，包括仍保存在参数定义中的文案。展示变化刷新目录和编辑器投影，动态端口标签按当前资源/实例事实投影，不能因复用语义缓存而继续显示旧文字。
- 当前 [canonical_semantic_protocol](../../src-tauri/crates/yss-node-registry/src/fingerprint.rs) 已排除顶层 Catalog，但 interface/parameters 中仍含展示字段。迁移时明确调整指纹编码和缓存失效，不将旧编码与新编码混用。

## 3. 组件契约、系统与状态归属

| 组件       | 定义级内容                                         | 实例或运行级内容                                           | 修改边界                                              |
| ---------- | -------------------------------------------------- | ---------------------------------------------------------- | ----------------------------------------------------- |
| PORTS      | 端口声明、动态模板、局部类型/Schema 约束和编辑配置 | 用户创建的端口实例、绑定；解析后的端口通过只读语义投影取得 | 编辑操作改变意图，解析器产生派生事实                  |
| KERNEL     | KernelId 绑定及执行策略                            | 使用已捕获输入的调用上下文                                 | 计算输出值，不修改图结构、用户参数或端口声明          |
| PARAMETERS | 类型、默认值、约束、编辑配置和文案                 | 图中实例的用户参数值                                       | 经可提交的图编辑命令修改                              |
| DATA       | 数据访问方式、资源需求；引用端口的数据契约         | 已授权数据句柄、输入值和结果引用                           | Execution 读取/发布；大数据由已有数据和结果所有者持有 |
| OPERATORS  | 编辑处理器及 resolver_type 类型推导行为的绑定      | 处理器准备候选变更，resolver_type 根据当前组件事实推导类型 | 统一 commit 提交；类型推导不独立写图或建立历史        |

### 3.1 组件组合必须具备的能力

1. **独立定义**：Ports 拥有端口声明和局部约束，Parameters 拥有参数声明和约束，Data 拥有数据访问要求，Operators 拥有编辑操作及 resolver_type 类型推导行为。各组件可以单独构造和校验，不先创建一个 Node 才能调用配置方法。
2. **显式装配**：节点类型以稳定 ID 关联组件 bundle，装配器校验组合。没有参数或外部数据需求时可以使用明确的空组件；普通叶节点、结构节点与透明节点保留各自合法的组合规则。
3. **复用与替换**：兼容的节点定义可以共享不可变的 Ports、Parameters 或 Operators。替换 Kernel 或一个操作处理器时，其他组件与通用调度代码无需跟随改写；不兼容的契约必须在可判定的装配、解析或调用边界拒绝。
4. **按组件消费**：分析、执行和编辑系统接收完成工作所需的组件或借用视图。普通节点差异通过组件声明与绑定表达，不能把实现搬进按 NodeTypeId 分支的巨型 Node 方法。
5. **唯一状态来源**：实例组件的读取和编辑落在现有图文档、语义快照与执行状态的所有权内。bundle 是定义组合，实例视图是对现有状态的访问方式，两者都不新增一套长期可写图。

这些是实现验收条件，不能只以出现五个同名字段或模块作为完成依据。组件不要求一一对应 crate；Kernel 和操作处理器的代码也不复制到每个实例中。

KERNEL 组件持有内核绑定，算法实现仍由 `yss-node-kernel` 注册和调用。OPERATORS 组件持有编辑处理器及 resolver_type 绑定，具体实现分别由 Graph 编辑和分析所有者执行。使用类型明确的绑定表达这些边界，节点协议不能反向依赖 Graph 实现，也不引入按任意字符串寻找可变服务的容器。

### 3.2 定义组件与实例状态分开

共享定义是不可变的 Ports、Parameters、Data、Kernel 和 Operators 声明。创建节点时只记录稳定身份、用户参数、端口实例和绑定等文档意图；默认值如何生效沿用现有协议，不因为共享组件就让两个节点共享可变参数。

解析阶段从共享组件和文档意图取得组件视图，产出所属图的语义快照。执行阶段从捕获的文档/语义身份建立本次调用的数据视图和有效参数，结果交给 ResultStore；两次运行不能通过某个共享 Data 组件相互覆盖。

NodeId、NodeTypeId、所属图、布局和实例标签继续由其现有所有者管理。Kernel 对普通叶节点适用；函数结构和透明重路由按原有角色解释，不能用空内核伪装成普通计算节点。

### 3.3 PORTS 的统一集合与三种信息

PORTS 统一存储端口列表，每项使用同一种端口描述类型，通过 `direction: PortDirection` 标明 Input 或 Output。节点内端口 key 在整个列表中唯一；声明顺序保留，按方向筛选时保持各自的相对顺序。

`inputs()`、`outputs()` 如作为查询方法提供，只返回统一集合的筛选视图或迭代器，不维护独立可写列表。连接校验、Kernel 输入/输出映射及编辑器展示都从该集合和 direction 取得端口信息。

现有 [NodeInterfaceProtocol](../../src-tauri/crates/yss-node-protocol/src/model.rs) 已使用 `ports: Box<[PortSpec]>`，且 PortSpec 已包含 direction。重构沿用这一存储语义，把声明归入 PORTS 组件即可。

端口信息仍按生命周期区分：

1. **声明**：节点类型的固定端口和动态模板。
2. **编辑意图**：用户添加的实例、字面量/常量绑定以及独立的连接记录。
3. **解析结果**：具体端口、类型、Schema、血缘及诊断，由 `GraphSemanticSnapshot` 拥有。

连接和参数变化可能触发第三种信息变化，不一定修改第一种声明。图级 Connection 记录两端地址，节点和端口上的查询索引只能作为派生索引，不各自保存一份可独立修改的连接事实。

### 3.4 DATA 与共享范围

同一执行会话中的图可以共享 ResultStore；项目/会话切换仍使用既有身份隔离和租约规则。DATA 持有值或引用，不成为参数、连接、缓存有效性和结果内容的第二个权威来源。数据集本体继续由 DatasetStore/Database 等现有所有者管理。

NodeRegistry、Catalog、KernelRegistry 是可共享或注入的只读配置，不需要全局单例。图中的节点、端口、参数、版本和历史仍归所属 Project/Graph。短期命令候选只服务本次提交，不成为另一套长期可写图草稿。

### 3.5 系统如何使用组件

| 使用方           | 读取的组件或状态                                                     | 产出及权限                                                    |
| ---------------- | -------------------------------------------------------------------- | ------------------------------------------------------------- |
| 装配/注册        | 独立组件及行为角色                                                   | 校验并冻结组件组合；不执行内核或编辑图                        |
| Resolve          | OPERATORS.resolver_type、PORTS、PARAMETERS、必要的 DATA 事实及图拓扑 | 调用节点类型推导并协调依赖传播，生成端口、类型、Schema 和诊断 |
| 操作处理器       | OPERATORS 绑定、两端 PORTS、需要的参数/数据事实及候选拓扑            | 准备候选文档变更；不自行提交或建立历史                        |
| Execution        | KERNEL 绑定、已解析 PORTS、有效 PARAMETERS 和已捕获 DATA             | 构造图无关调用，校验并发布结果                                |
| commit / history | 整批候选补丁、基准版本、资源身份和已有回执                           | 原子提交或撤销/重做，驱动解析与结果有效性更新                 |

例如 connect 先从端口地址定位两端节点，再读取各自的 PORTS 和 OPERATORS，按需要读取 PARAMETERS/DATA。处理器在候选图中准备连接和动态端口变更，Resolve 调用受影响节点的 OPERATORS.resolver_type 并沿依赖传播，统一 commit 发布结果。`graph.port(a).connect(b)` 是这个流程的操作入口，组件模型通过其内部的数据和行为协作体现。

### 3.6 类型推导与数据转换：Add 示例

这里的 DS 指 DataSeries。PORTS 声明允许的输入集合和局部约束；跨端口的类型推导行为放在 OPERATORS 的 `resolver_type` 中。Graph 的 Resolve 负责调用该行为并协调节点间的依赖传播，解析事实仍由 GraphSemanticSnapshot 拥有。

组件装配示意：

```rust
let operators = Operators {
    resolver_type: Some(resolve_add_type),
    ..Operators::standard()
};
```

`resolver_type` 读取节点当前的端口类型、相关参数和数据元信息，返回推导结果及诊断。它不直接修改其他节点、连接或数据，也不执行 Kernel；其他节点由图级解析流程按依赖关系调用各自的 resolver_type。

连接、断开、端口增删、相关参数或数据 Detail 变化，以及 undo/redo，都自动触发受影响节点的解析。相关类型或约束变化后继续向依赖节点传播，直到受影响的事实稳定。这里更新的是节点的已解析端口类型，不改变 NodeTypeId。候选编辑中的推导结果随同一次 commit 发布，不另产生撤销项。

为复用输入约束，定义以下类型集合，统一使用 `GeneralNumerical` 命名：

```text
Numerical                  = 满足数值运算要求的标量集合
DataSeries<Numerical>       = 元素满足数值运算要求的数列集合
GeneralNumerical           = Numerical | DataSeries<Numerical>

Add.inputs[*]: GeneralNumerical
```

这些集合用于端口约束，不新增基础类型或 Detail 选项。Numeric 仍是数据的 Semantic 选择；Numerical 要求当前 Semantic 为 Numeric，且 Physical 满足该运算支持的数值表示约束，不固定绑定某一种 Physical。Identifier、Categorical、Ordinal、Binary 不因底层使用数字而自动属于 Numerical。

集合决定输入是否可接受，输出解析规则决定最终结果。Add 不能仅将输出也标为 GeneralNumerical 就结束解析；输入确定后，应解析为具体的数值标量或数列，并继续确定执行所需的数值表示。

| 责任               | 归属                    | Add 示例                                                                 |
| ------------------ | ----------------------- | ------------------------------------------------------------------------ |
| 声明输入约束       | PORTS                   | 输入接受 GeneralNumerical 集合                                           |
| 定义类型推导行为   | OPERATORS.resolver_type | 任一输入为 DS 时输出形状为 DS；全部为标量时输出标量；按数值规则确定表示  |
| 准备编辑并触发解析 | OPERATORS / 图编辑流程  | connect、disconnect、参数修改等改变候选文档，按需要请求 Resolve          |
| 协调解析及依赖传播 | Resolve                 | 调用受影响节点的 resolver_type，汇集类型与输入适配要求并传播到依赖节点   |
| 转换实际数据并计算 | Execution / KERNEL      | 按解析后的规格执行数值转换、标量广播和加法，并校验数列的长度或行域兼容性 |

在输入均满足上述集合约束时，Add 的输出规则为：

```text
Numerical             + Numerical             → Numerical
Numerical             + DataSeries<Numerical> → DataSeries<Numerical>
DataSeries<Numerical> + Numerical             → DataSeries<Numerical>
DataSeries<Numerical> + DataSeries<Numerical> → DataSeries<Numerical>
```

第二个例子同时包含两件事：编辑解析将 result 的已解析类型确定为 DS<Numeric>；执行时标量按数列范围参与广播加法。内部仍推导具体数值表示，例如 Scalar<Float64> 与 DS<Int64> 相加需要 DS<Float64> 输出；两个 DS 还须满足长度或行域兼容要求。标量来源端口仍是标量，广播不会改写上游节点的端口声明或数据。类型推导只更新当前语义快照中的派生事实，不把输出类型作为用户配置写入 GraphDocument。

规则必须从当前输入重新求解。例如将 DS 输入替换为标量后，若其余输入也均为合法标量，输出恢复为标量。打开图、上游类型变化、参数修改和 undo/redo 也使用同一规则；不能仅在 connect 处理器中单向执行“将 output 改为 DS”。未绑定、未知或冲突的输入继续保留相应类型状态和诊断，不能只因某个输入为 DS 就认为节点已经可执行。

若反向使用输出约束：要求标量输出意味着所有输入都须为数值标量；要求数列输出只意味着至少一个输入须为数列，不能据此认定每个输入都是数列，或任意修改某个输入的数据类型。未确定的输入继续保留集合约束，不默认选择标量。

当前 [Add 定义](../../src-tauri/crates/yss-node-catalog/src/core_nodes/math.rs) 已通过 [NodeTypingSpec::NumericFold](../../src-tauri/crates/yss-node-protocol/src/typing.rs) 声明 `AnySeriesElseScalar` 和 `Widen`，并覆盖全部动态 operands 实例。重构时由 OPERATORS.resolver_type 绑定对应的推导行为，复用[现有类型求解](../../src-tauri/crates/yss-graph-analysis/src/type_resolution.rs)和[数值内核](../../src-tauri/crates/yss-node-kernel/src/builtins/numeric.rs)，保持推导与运行结果一致，不在 PORTS 中另存一套跨端口推导规则。

本例纳入 A7 的解析联动与 A9 的编辑/执行边界验收：输入替换及 undo/redo 能重新推导输出，编辑阶段不执行广播或加法，实际计算结果符合已解析的输出契约。

### 3.7 七种基础类型与两种复合类型

下面七种基础类型作为数据的 Semantic 选项，在数据 Detail 中选择。Physical 是另一个独立选项，两者没有固定的一一对应关系；合法的组合范围由数据约束和支持的转换规则决定。

| 基础类型    | 含义与示例                            | 需要保留的语义                                                               |
| ----------- | ------------------------------------- | ---------------------------------------------------------------------------- |
| Numeric     | 数值变量，如身高、收入、温度          | 数值范围、整数等约束；内部表示与精度                                         |
| Categorical | 无序分类，如地区、产品类型            | 类别取值及其编码映射；编码大小不代表大小关系                                 |
| Ordinal     | 有序分类，如满意度、教育等级          | 显式等级顺序；不能从标签字典序或编码大小猜测顺序                             |
| Binary      | 二元变量，如是/否、0/1                | 两个允许取值及其映射；需要时明确事件/正类，不能把缺失值算作第三类            |
| Datetime    | 日期/时间，如日期、时间戳             | 日期/时间粒度与精度等表示信息；项目日历及用户可见时间沿用现有无时区语义      |
| Text        | 非结构化文本，如评论、描述            | 原始文本；不能因本批唯一值较少就自动当成分类                                 |
| Identifier  | 标识变量，如样本 ID、客户编号、订单号 | 保留标识语义及原始表示；不自动参与统计数值计算，唯一性由具体数据契约另行约束 |

Physical 与 Semantic 属于数据或字段的元数据，不在 PortType 中建立 Shape、Physical、Semantic 的固定组合层级。数据 Detail 分别提供这两个选项：

| Detail 选项 | 内容                                                              | 选择约束                                       |
| ----------- | ----------------------------------------------------------------- | ---------------------------------------------- |
| Physical    | 实际的数据表示，如 Bool、Int64、Float64、Utf8、Date               | 受当前值域、精度及支持的数据转换限制           |
| Semantic    | Numeric、Categorical、Ordinal、Binary、Datetime、Text、Identifier | 受当前数据及类别、等级、二元映射等语义约束限制 |

例如同为 Int64 的数据，可以选择 Numeric、Categorical、Ordinal 或 Identifier；选择 Binary 时还需满足二元取值约束。选择范围不是任意组合，也不是由 Physical 自动决定唯一的 Semantic。

修改 Semantic 不自动改写 Physical；修改 Physical 需要验证并执行相应的数据转换，不自动把 Semantic 重置为 Numeric。Ordinal 的等级顺序、Binary 的取值映射等配置仍在 Detail 中明确。节点根据数据当前的设置校验输入，不替用户更改选择。

本轮复合类型仅保留以下两种。DataSeries 的 T 为七种基础语义类型之一，DataFrame 使用独立类型标识；不开放 Array 或嵌套容器作为分析数据类型。

| 复合类型      | 含义                                               | 示例                                                             |
| ------------- | -------------------------------------------------- | ---------------------------------------------------------------- |
| DataSeries<T> | 同类元素组成的数据列，关联其行域、顺序及来源事实   | DataSeries<Numeric>、DataSeries<Categorical>、DataSeries<Binary> |
| DataFrame     | 同一行域内的多列数据；字段 Schema 属于数据帧元数据 | income: Numeric、region: Categorical、rating: Ordinal            |

DataFrame 不带 Schema 泛型参数。字段名称、字段类型及相关语义属于资源/运行值的元数据，由 Resolve 捕获并形成图语义快照。以下为类型与元数据的关系示意，不是序列化 wire：

```text
type: DataFrame
schema:
    sample_id: Identifier
    income: Numeric
    region: Categorical
    rating: Ordinal
    employed: Binary
    observed_at: Datetime
    comment: Text
```

DataFrame 的不同字段可以有不同类型；提取 region 后得到 DataSeries<Categorical>，提取 sample_id 后得到 DataSeries<Identifier>，同时保留相应语义元数据和行域信息。字段变化时端口类型仍为 DataFrame，Schema 变化照常触发解析、下游校验和缓存有效性更新；不能因两个输入都叫 DataFrame 就假定字段相容。

有序数据列统一表达为 DataSeries。Rust Vec、运行值列表或 JSON 数组可以作为内部存储、配置及传输结构，不作为额外的分析数据类型暴露。迁移现有 Array 端口、常量和函数签名时，只有元素语义明确且结构符合要求的序列才能迁为 DataSeries，并需建立明确的位置/行域关系；异质或嵌套旧值应报告不支持并要求整理，不静默展平、丢值或丢失 Identifier 等语义。长度相同不能代替行域对齐依据。

类别集合、Ordinal 的等级顺序、Binary 的取值映射以及 nullable 等信息随类型/字段描述传递。资源声明和显式节点配置仍由原有所有者维护，Resolve 在 GraphSemanticSnapshot 中生成相应派生事实，Execution 与结果投影保留所需元数据；筛选后某一类别暂未出现，不自动改变已声明类型或重排类别编码。

转换遵循以下要求：

- Add 默认接受 Numeric 及 DataSeries<Numeric>。Binary、Categorical、Ordinal、Identifier 即使底层使用数字，也不能直接当成 Numeric 相加；需要显式语义转换或明确声明该语义的专用节点。Identifier 的额外限制见第 3.9 节。
- Numeric 内部的整数/浮点提升沿用数值规则与精度约束；统一显示为 Numeric 不能成为静默丢失精度的理由。隐式提升由 Resolve 判定，实际数据处理由执行适配/Kernel 完成。
- Categorical 编码为 Numeric 时保留可追溯的映射；Ordinal 转成分数需明确等级与分值；Binary 转成 0/1 需明确对应关系；Text 转成数值或 Datetime 需明确解析规则和失败行为。
- Numeric 比较等谓词操作产生 Binary 或 DataSeries<Binary>；实际布尔执行表示继续由内核适配处理。分类或序数专用操作通过各自契约声明支持范围，不套用 Numeric 的推导规则。
- 导入可依据源 Schema 与明确元数据确定类型；仅凭出现了 0/1、两个不同值或少量唯一值，不自动把 Numeric/Text 改成 Binary/Categorical。缺失值通过 nullable/有效性信息表示，Unknown、Conflict、泛型及联合约束保留为解析机制，不作为额外的面向用户基础类型。

七类基础类型与两类复合类型描述分析数据。已拟合模型、统计报告等专用产物继续使用已有领域类型及注册契约，保持各自产物的身份和有效性约束。

当前基础语义由 [SemanticType](../../src-tauri/crates/yss-data-contract/src/column_semantic.rs) 唯一定义，
[ValueType](../../src-tauri/crates/yss-data-contract/src/value_type.rs) 与其[前端契约](../../src/shared/types/domain/valueType.ts)
引用基础语义并描述数据结构。DataFrame 分解、选列、常量、函数签名、Graph 解析和编辑器投影已接入七种语义；
[SchemaField](../../src-tauri/crates/yss-node-protocol/src/types.rs) 的标量事实引用同一语义枚举。物理表示继续由值和字段元数据持有。
内部 Array/Object 结构仍保留；本节要求的旧 Array 进一步收敛不以重命名或静默转换代替。

迁移在现有 Data Contract、Node Protocol、Schema/Resolve 和 Execution 所有者内进行，同步处理 DTO/parser、常量与函数签名编辑、数据库字段语义、结果元数据和指纹。旧 Boolean 可按其明确布尔域映射到 Binary；旧整数/浮点仍保留各自表示，已有类别元数据须保留，无法确定的等级或映射不由前端猜测。持久化版本及不兼容输入的迁移/诊断方式须明确，不能出现前端与后端分别维护一套类型判断。

### 3.8 类型收敛后的最小解析职责

七类基础语义由声明或明确的转换确定，不要求系统通过扫描数据猜测变量类别。端口解析按节点需要保留以下职责：

| 情况                 | 处理方式                           | 示例                                                  |
| -------------------- | ---------------------------------- | ----------------------------------------------------- |
| 类型固定             | 直接使用声明，校验输入兼容性和配置 | 正态采样的输出始终为 DataSeries<Numeric>              |
| 输出形状依赖输入     | 按声明规则计算并向下游传播         | Add 的输出随输入确定为 Numeric 或 DataSeries<Numeric> |
| 输出取决于字段或配置 | 从当前 Schema/参数解析输出         | 选择 region 列得到 DataSeries<Categorical>            |

执行时的数值提升和运行适配仍按节点契约处理，不回写数据 Detail 中的 Physical/Semantic 选择。未知输入或缺失 Schema 保持未确定状态和诊断，不能用 Numeric 代替。

因此保留现有 Resolve 入口，负责兼容性校验及类型/Schema 的依赖传播。节点自身的动态输出推导由 OPERATORS.resolver_type 提供，固定类型节点可直接使用声明。各类编辑触发同一解析路径，不因组件化引入新的通用语言类型推理框架。

### 3.9 Identifier 的统计输入约束

Identifier 表示记录或实体的标识，例如 `sample_id`、`customer_id`、`order_id`。它可用字符串或精确整数等表示，但数值外观、列名、唯一值数量和编码大小都不能使它自动成为 Numeric、Ordinal 或普通分类变量。明确的源字段语义或用户声明决定 Identifier 身份；不能宣称仅凭原始数值就已识别所有标识列。

| 使用场景                                   | 要求                                                                                                  |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| 自动选列、推荐变量、展开“全部可分析列”     | 排除 Identifier；按方法明确支持的语义类型选择，不能只检查底层是否为数字                               |
| 均值、相关系数、OLS 等分析变量输入         | 不接受 Identifier；OLS 的 response、predictors 及预测特征都遵循此规则，不自动提升、编码或生成虚拟变量 |
| 显式连入统计数值端口、旧图绑定或程序化调用 | 给出可定位的类型不兼容诊断并阻断执行；不能静默改成 Numeric，也不能丢掉该输入后假装原请求执行成功      |
| 连接键、匹配键、分组键、实体/样本对齐      | 仅在节点明确声明的标识角色中接受 Identifier；作为键使用不使它成为被求均值、相关性或拟合系数的测量变量 |

这项约束必须由 Rust 的类型校验与统计执行入口根据捕获的输入语义落实，GUI 和自动化调用共享规则。前端过滤候选项不能替代后端校验；准备统计数值数组前检查语义，不能在只剩 Float64 数组后再猜测哪些值原本是 ID。输入缺少所需语义事实时保留诊断，不默认按 Numeric 放行；内核仍消费图无关契约。

Identifier 语义参与 Schema/语义身份及计划有效性校验。将字段从 Numeric 改为 Identifier 后必须重新校验受影响的下游，不能继续复用此前允许统计计算的旧计划或把旧计算结果当作当前有效结果。

筛选、重命名、投影、提取 DataSeries、连接以及数列/数据帧转换等保留标识含义的操作必须传播 Identifier；语义不能在 DataSeries/DataFrame、序列化或结果读取之间丢失。普通物理类型转换也不解除标识语义。原始标识表示须保真，例如字符串 ID 的前导零不能因自动数值化而丢失。

只有用户在数据 Detail 中显式选择新的 Semantic，或使用等价的显式转换操作，并通过目标语义和值域校验后，才能解除 Identifier 语义。该变更须保留操作记录，不能由自动选列、Physical 修改、隐式类型提升或统计节点内部补做。

Identifier 约束进一步明确了第 3.8 节中类型校验的必要性：底层同为 Int64 的收入列和客户编号，前者可作为 Numeric 分析变量，后者必须保留标识用途。

## 4. 直接操作 API

### 4.1 对外形式

写操作由对象句柄直接提供，不暴露 `.operators()`：

```rust
graph.port(a).connect(b)?.commit()?;
graph.port(a).disconnect(b)?.commit()?;
graph.port(a).set_literal(value)?.commit()?;

graph.node(n).set_parameters(values)?.commit()?;
graph.node(n).set_configuration("configuration", fields)?.commit()?;
graph.node(n).remove()?.commit()?;

graph.add_node(creation)?.commit()?;
```

`creation` 复用已注册类型或受验证的创建描述，不接受任意未注册定义绕过装配校验。参数修改沿用现有参数/配置更新语义；输入字面量方法只允许在适用的输入端口使用，不是写输出结果的接口。

这些方法准备候选编辑，只有 `commit()` 提交正式状态。查询方法保持只读，不进入编辑历史；执行和保存分别调用对应所有者，不在上述编辑链中混入计算或文件副作用。

Graph/Node/Port 句柄负责定位和转交操作，将调用交给相应 OPERATORS 绑定的处理器。处理器访问当前候选的组件视图；句柄本身不拥有一份 Ports/Parameters/Data，也不重写组件规则。替换操作绑定不能绕过图级方向、容量、版本、资源授权和原子提交约束。

GUI、Agent、粘贴/插入子图及其他程序化调用对节点、端口、连接和参数的编辑，都复用同一命令提交通道，不能直接写组件绕过校验和历史。项目加载、资源发布与运行结果更新继续由各自所有者处理，不伪装成用户编辑命令。

### 4.2 端口地址与对象定位

`a`、`b` 是端口地址或可解析的只读端口句柄。优先复用现有 [PortAddress](../../src-tauri/crates/yss-graph-document/src/model.rs)，它已经包含 NodeId 和 PortRef。无需仅为定位节点新增全局反向索引；以后若使用独立 Port Entity，其 owner 索引也只归所属图。

句柄绑定图/项目身份和编辑上下文，不能通过裸 ID 跨图隐式连线。每次使用都验证对象仍存在；长期持有的句柄不保留另一份可写节点或端口状态。

### 4.3 链式操作的对象保持

```rust
graph.port(a)
    .connect(b)?
    .connect(c)?
    .commit()?;
```

当 a 为输出、b/c 为输入时，上述表示 a→b 和 a→c。`connect` 后操作对象仍是 a，不隐式切换到 b。方法返回可继续编辑的批次句柄；后续操作使用同一候选图、同一基准版本和同一提交身份。

需要修改多个节点/端口时，使用显式图编辑批次，让对象句柄引用同一事务上下文。例如：

```rust
let mut edit = graph.edit();
edit.port(a).connect(b)?;
edit.node(n).set_parameters(values)?;
edit.commit()?;
```

这段也是目标接口示意。批次里的对象方法不能自动提交，也不能暗中启动独立事务。

### 4.4 connect 的内部要求

- 获取两端端口及其所属节点，验证所属图、方向、容量、现有连接和适用规则。
- 从任一端发起时，可依据端口方向规范化为 output→input；input→input 和 output→output 必须拒绝。
- 保留当前有序连接的 OrderKey 语义；需要顺序时由接口显式提供或由已有规则确定，不能丢弃顺序。
- 使用图解析检查类型、Schema、循环依赖及下游影响，不能只分析目标节点。
- 在候选图中处理动态端口 claim、绑定迁移和连接变化，使后续链式操作能读取已经发生的候选变化。
- 复用当前重复连接、连接容量和孤立端口的行为约定，不因链式包装悄悄改变错误语义。

## 5. commit 与 Command

`commit()` 是一次编辑批次的原子提交点，也是一个可撤销 Command 的边界。单次调用只产生一条历史记录，包含本批全部实际变更。

```text
对象操作链
  → 捕获基准版本并准备候选图
  → 校验操作与解析受影响的端口
  → 生成完整可逆 GraphDocumentPatch
  → 提交前重验项目、图版本和实际读取的资源
  → 原子提交当前图、历史和回执
  → 更新结果有效性并发布只读投影
```

| 情况               | 必须满足的行为                                                              |
| ------------------ | --------------------------------------------------------------------------- |
| 尚未 commit        | 正式图、历史、dirty 与当前结果不因候选编辑而改变                            |
| 构建批次时失败     | 本批不产生部分提交；失败批次不得继续提交有效前缀，调用方重新开始或取消      |
| 提交校验失败       | 不应用补丁、不新增历史、不遗留半条连线或孤立端口                            |
| 成功且有变化       | 全批生效，新增一个撤销项，清理原 redo 分支                                  |
| 没有实际变化       | 返回 changed=false，不新增撤销项、不清空 redo；版本和操作回执仍遵循既有协议 |
| 放弃或丢弃候选     | 释放本批临时数据，不隐式提交                                                |
| 提交后响应丢失     | 复用原 operation_id 和请求指纹查询/恢复回执，不能再次追加相同变更和历史     |
| 项目替换或基准过期 | 拒绝旧提交，不静默合并到新图；调用方读取新版本后显式重新构造操作            |

结构非法的操作应拒绝。未连全、待配置或缺少执行内核的图仍可保留为合法编辑状态并显示诊断；编辑提交不要求整张图达到 Execute Ready。

提交回执沿用现有项目/图身份、请求与提交版本、operation_id、changed 以及必要的创建对象关联和投影信息。不新增平行的 CommandStore 或另一份历史游标；操作回执与撤销历史分别复用当前所有者。

## 6. undo / redo

历史保存可逆文档补丁，而不是只保存原始方法调用。连接的逆操作必须准确恢复原绑定、连接 ID、顺序及用户创建端口等状态，不能简单把方法名 connect 替换成 disconnect。

- undo 原子应用同一 Command 的逆补丁；redo 原子应用正补丁，保留已经分配的节点、连接和端口实例身份。
- 多步编辑形成一个撤销单位。撤销、重做自身不作为新的普通 Edit 项再次压入历史。
- 应用失败时不移动历史游标，也不留下部分恢复状态。
- 恢复后重新解析类型、Schema、血缘、动态端口和诊断，再按既有规则更新结果有效性。
- 不把完整数据集、运行输出、旧解析快照或 Kernel 对象写进编辑历史；redo 不重新调用随机采样或模型拟合来“恢复”编辑。
- 外部资源可能已经变化。历史恢复的是文档意图，不回滚外部数据集；重新解析时应显式展示不可用或 orphan 状态。

Kernel 执行、结果读取和运行时结果发布不进入图编辑历史。GUI 普通 commit 提交当前驻留图；文件持久化由显式 Save 负责。现有 Assistant 编辑批次自动保存完整当前图的约定继续保留，由 Application/Project 在同一批次事务中组合持久化与历史，不能先提交内存再补一次非原子保存。

## 7. Kernel 与解析、编辑的边界

Kernel 的数据流为：已捕获输入/数据句柄 + 有效参数 + 已解析输出规格 → 输出值。Execution 校验并发布输出，维护 ResultStore 和运行身份。

Kernel 不直接修改 PORTS 结构、PARAMETERS 用户值、Connection、编辑历史或图文档。PORTS 保存端口声明和局部约束，OPERATORS.resolver_type 提供节点类型推导行为；连接、新增和参数修改触发图编辑及 Resolve，由图级解析流程协调依赖传播与 Schema 解析。

运行时才获得的新资源 Schema 应通过现有资源/结果发布机制交给解析系统，不允许内核自行回写图结构。ECS 或对象操作 API 不替代数据依赖 DAG、执行计划及版本/资源重验。

节点定义、Catalog、本地化及新操作句柄不应让 `yss-node-kernel` 重新依赖 Graph、Project、Application 或 Tauri。可变图编辑逻辑继续位于 Graph/Application 的现有职责范围内。

## 8. 当前实现的复用位置

| 当前所有者                                                                                                                                                         | 可复用内容                                                   | 本次目标调整                                          |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ | ----------------------------------------------------- |
| [Node Protocol](../../src-tauri/crates/yss-node-protocol/src/model.rs) 与 [参数协议](../../src-tauri/crates/yss-node-protocol/src/parameter.rs)                    | 端口、参数、行为和类型约束                                   | 将既有声明归入独立组件契约；分离节点级 Catalog 信息   |
| [Node Registry](../../src-tauri/crates/yss-node-registry/src/model.rs) 与 [指纹](../../src-tauri/crates/yss-node-registry/src/fingerprint.rs)                      | 注册一致性、共享定义和指纹                                   | 装配/冻结组件 bundle，校验组合并区分语义和展示身份    |
| [Node Catalog](../../src-tauri/crates/yss-node-catalog/README.md)                                                                                                  | 分类、本地化、文档、创建描述                                 | 按节点 ID 注册节点级描述及树位置                      |
| [图模型](../../src-tauri/crates/yss-graph-document/src/model.rs)                                                                                                   | NodeId、PortAddress、Connection、实例值                      | 保持身份，提供受控对象句柄                            |
| [EditorGraphMutation](../../src-tauri/crates/yss-graph-editor/src/mutation.rs)                                                                                     | Connect、SetParameters、SetLiteral、CreateNode 等 typed 命令 | 对象方法经 OPERATORS 处理器准备这些命令；收敛编辑入口 |
| [Application 图编辑](../../src-tauri/crates/yss-application/src/graph/editing.rs)                                                                                  | 版本、operation_id、按图编排和回执                           | 组织整个链式批次的提交与重试                          |
| [可逆文档编辑](../../src-tauri/crates/yss-graph-document-edit/src/lib.rs) 与 [Project 历史](../../src-tauri/crates/yss-project/src/project_state/graph_editing.rs) | GraphDocumentPatch、undo/redo 与历史预算                     | 一个 commit 对应一个完整历史项                        |
| [Kernel 调用](../../src-tauri/crates/yss-node-kernel/src/invocation.rs) 与 [Execution 适配](../../src-tauri/crates/yss-graph-execution/src/kernel_invocation.rs)   | 图无关调用及结果映射                                         | 保持编辑与计算分离                                    |

当前 `Connect` 已经使用 output/input PortAddress，本次端口级接口可以建立在现有命令上。组件装配、系统按组件工作与直接操作接口分别验收；不预设更换全部存储方式，也不宣称链式写法本身能改善性能。

## 9. 实施顺序

1. 确定七类基础语义、复合类型及内部表示的契约，补齐类别/等级/二元映射和 Identifier 语义，明确统计输入限制、序列化、现有数据迁移与消费者适配范围。
2. 定义五个组件的类型、契约、实例视图和 bundle 装配规则，将现有端口/参数/行为声明迁入对应组件。用一个已有可执行叶节点贯通定义、注册、解析和执行，再覆盖动态端口及结构/透明节点的组合；证明组件能独立使用和替换，并保持语义类型及数值表示一致。
3. 拆分节点级展示元数据，更新 Catalog 注册、查询和指纹；同步画布、详情、搜索及目录消费者。
4. 将编辑操作和 resolver_type 归入 OPERATORS，接通连接、断开等变化后的自动类型传播；提供 Graph/Node/Port 句柄，保持稳定身份和图上下文。
5. 落实候选编辑与 commit，统一批次、错误、版本、回执和历史边界；接入现有 GUI/Harness 适配。
6. 验证组件复用、实例隔离、动态端口、解析结果、撤销重做及 ResultStore 失效/租约的联动。
7. 在这些通用要求上继续实施 [OLS 首批规格](OLS_SPEC.md) 及其他方法批次。

不因分类或 API 整理修改节点 ID。跨协议、DTO、序列化或指纹的变化须一起处理消费者与明确的失效/迁移方式，按项目 0.x 约定移除过时内部路径，不长期维持两套定义、目录或命令实现。

## 10. 验收要求

| 编号 | 验收点                  | 预期结果                                                                                                                                 |
| ---- | ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| A1   | 节点定义与 Catalog 归属 | 定义不含节点级标题/文档/分类；端口和参数契约留在各自组件；端口无 title/description/i18n，默认显示英文 key，资源/实例标签保留原文         |
| A2   | 目录注册与展示查询      | 通过稳定 ID 取得元数据；画布/详情/搜索可用，目录中没有端口/参数文案镜像                                                                  |
| A3   | 直接连接与句柄身份      | `graph.port(a).connect(b)` 可准备变更；连续连接保持 a；方向、跨图、过期和容量错误可定位                                                  |
| A4   | 候选隔离与失败          | commit 前正式状态不变；后续步骤或提交失败时整批不生效，丢弃候选无副作用                                                                  |
| A5   | 一次提交与一次撤销      | 多步连接/参数修改形成一条历史；undo/redo 恢复完整内容、绑定、顺序和稳定身份                                                              |
| A6   | no-op 与历史分支        | 无变化不新增撤销项或清空 redo；有变化的新提交按规则清理 redo                                                                             |
| A7   | 动态端口与类型传播      | 连接、断开、参数/数据变化及 undo/redo 自动调用受影响节点的 resolver_type 并传播结果；失效成员显式处理，不恢复过时解析快照                |
| A8   | 重试与并发版本          | 响应丢失后恢复同一回执；重复请求不新增连接或历史；旧版本不覆盖新提交                                                                     |
| A9   | 编辑、执行与存储        | Kernel 不编辑图；结果不进入编辑历史；结果租约和会话隔离符合当前契约                                                                      |
| A10  | 文案与缓存              | 改节点文案、参数纯展示字段或分类会更新展示，不误触发科学计算；端口 key 等语义变更仍正确失效                                              |
| A11  | 保存与会话生命周期      | GUI commit 与 Save 分离；Assistant 批次自动保存仍保持原子性；项目替换使旧句柄/批次失效                                                   |
| A12  | 独立组件与组合校验      | 各组件可独立构造和校验；合法 bundle 可注册，冲突组件和无效跨组件引用有明确错误；未实现 Kernel 单独诊断                                   |
| A13  | 组件复用与行为替换      | 两个定义可复用 Ports/Parameters；仅替换兼容 Kernel 或 Operators 绑定即可选择另一处理实现，无需修改通用调度或按节点类型分支               |
| A14  | 实例隔离与系统消费      | 同类型节点共享定义但不共享可变参数/运行数据；系统读取组件契约，组件视图不成为第二套可写图或结果缓存                                      |
| A15  | 端口统一存储            | 输入/输出共用端口列表与描述类型，direction 明确；key 在整个列表中唯一，方向筛选保留相对顺序且不产生独立可写端口表                        |
| A16  | 基础与复合类型契约      | 七类基础类型与两类复合类型；DataSeries<T> 保留元素语义，DataFrame 不带泛型参数且通过元数据维护 Schema；旧 Array 迁移不丢值或语义         |
| A17  | 语义转换与类型推导      | Add 保持 Numeric 的形状推导和内部数值精度规则；分类/序数/二元/标识类型不因数值编码而自动进入算术；显式转换后语义和数据一致               |
| A18  | Identifier 统计隔离     | 数字或字符串 ID 均不被自动选入统计分析；派生操作保留 Identifier；直接/程序化统计数值绑定被后端阻断执行，标识角色与显式语义转换按契约处理 |

后端优先复用现有图编辑、历史、结果生命周期和内核测试，按独立回归风险补最少必要用例。UI 按仓库要求人工验收，禁止新增 UI 单元测试。校验命令、受影响消费者及完成标准遵循[本地工作流](../development/LOCAL_WORKFLOW.md)和[变更流程](../development/CHANGE_PROCESS.md)。

本轮只交付文档要求。代码实施与 A1–A18 验收均为待执行，完成后更新当前架构文档、专项计划和实际验证记录。

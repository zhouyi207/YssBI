# yss-node-kernel

> Status: Current
> Scope: 不依赖 Graph 的内核调用契约、运行值、冻结注册表和内置节点执行适配
> Canonical owners: 本 crate 源码拥有内核能力；图计划、资源授权、结果生命周期由 [Graph 与 Execution](../yss-application/src/graph/README.md) 定义
> Update when: 内核调用契约、注册/指纹规则、控制边界或内置适配改变时

本 crate 持有可执行函数。Node Protocol/Registry/Catalog 持有节点声明和目录；Application 的 `NodeComponents` 核对声明与内核契约，冻结一份注册表，供同一会话的能力检查和实际执行共同使用。

## 调用边界

`builtins/statistics/regression_models` 为回归目录的 31 个新增入口提供无图适配：
按本地输入键共同物化并证明行域对齐，将分类响应和分组编码交给 SCI，返回时恢复
原始标签和声明的有序级别。输入、密集工作区、批量模型结果和结构化序列化均执行
预算检查；SCI 接收同一取消标记和 deadline。每个入口只有结构化 `result`，
不创建报告端口或专用页面。原有 Linear/Logit/Probit/Prais 生命周期继续由原适配器维护。

回归工作区按实际自变量、截距、类别、非线性参数及展开后的设计列估算，数值缓冲按
`f64` 计费。分阶段回归按本次配置保留的模型数计费，分组回归的观测结果按各组总行数
计费；分类概率、样条基列和结构化编码另计。输入常驻内存加上拟合与结果编码两阶段
的较大估算值用于准入，编码估算复用 `common::value` 的容器计费口径。节点的默认
128 MiB 预算可由 `KernelControl.max_input_bytes` 配置，查询引擎的独立内存池限制继续生效；估算仍不是进程 RSS 的精确上限。

`KernelInvocation` 接收已求值输入、固定端口或重复组的局部键、有序输出类型/字段、已解析参数、`KernelControl` 和中立 `RelationFactory`。参数可以借用现有 literal 和已授权的资源运行值。内核返回按调用局部输出顺序排列的 `Vec<RuntimeValue>`；注册表在调用前检查输入布局，在返回后检查输出数量和外层载体，不重新推导 Graph 的元素语义。

运行值的标量载荷统一为 `TabularScalar`，有限浮点表示为 `Float64(FiniteFloat64)`；没有内核自定义 Decimal 算术类型。文档的 `DecimalLiteral` 是精确数字文本，在求值时转换。列表和记录通过不可变 `Arc` 共享。调度槽、下游输入和结果发布不再逐元素复制；统计拟合值和残差只在转换成运行值时分配一次，不先复制原数值向量。紧凑数值缓冲仍属于[后续评估](../../../TODO.md)，当前模型向量与通用运行值列表保持不同表示。

DataFrame 常量和内存列组合统一通过调用方注入的 `RelationFactory` 进入共享查询引擎，后续选列、筛选、拆列和分页均使用 `RelationHandle`。`Record` 承载配置、结构化统计模型和检验结果。字面量关系不声明数据集绑定，不能据此获得外部资源授权；组合仍检查实际数据集的会话、快照及执行环境。

导入裸分类字面量时建立无序值域，已有显式值域和顺序级别继续保持不变；日期时间文本复用 Arrow 适配器的日历转换，去除时区时保留墙钟字段。
数列常量按已解析的元素语义导入关系句柄；Identifier、Datetime 和全空列同样保留声明的语义，不从底层文本或首个非空值重新猜测。

`KernelControl::check_bytes/reserve` 统一执行预算检查和可失败的预留。统计内存列在转换前检查完整形状和总输入大小；GLS 辅助矩阵、密集工作区及两份输出使用合计准入估算，预测和转换循环周期性检查取消。估算不等同于进程 RSS 上限，也不承诺中断正在执行的矩阵分解。

输出契约通过 `ValueType` 引用七种 Semantic，数值端口统一为 Numeric。四则运算根据实际标量或
Arrow 字段的 Physical 选择整数/浮点表示，除法使用浮点表示，有损提升会被拒绝。
幂和任意底数对数复用算术的形状、广播和关系行域检查，统一输出 Float64；实数定义域与非有限结果检查由 `NumericOperation::evaluate_float` 共享给内存和 DataFusion 批执行。
自然对数、以 2/10 为底的对数、平方和平方根通过同一 Numeric 内核执行；操作数数量由 `accepts_arity` 校验，单输入实数计算由 `evaluate_unary_float` 在内存和批执行间共享，不构造重复输入或模拟底数。
六个比较节点统一支持标量、等长内存数列和同一关系行域的惰性数列，以及任一侧的标量广播；空值传播。整数/浮点混合比较复用 Tabular Contract 的精确比较，不经过有损浮点提升或 epsilon。文本按原值比较，排序采用大小写敏感的字典顺序。
六个比较节点对数列逐元素比较，支持精确模式与数值容差模式。惰性比较通过关系契约生成 DataFusion 表达式。Kernel 可以持有 Arrow 数组、字段和批数据，不持有 DataFusion 查询上下文。
AND/OR/NOT 同样支持标量、内存数列及同一行域的惰性数列；内存计算复用 `BooleanOperation` 的三值逻辑，惰性计算通过关系契约生成原生布尔表达式。`false AND null` 为 false，`true OR null` 为 true，`NOT null` 为 null。内存数列检查长度、输出预算及取消状态；旧的纯标量布尔执行入口已移除。
Graph 的广播说明不提前改写标量值。数值、转换、比较和关系筛选适配的行为变化会推进实现 revision。

类型转换支持七种 Semantic，保留标量/数列结构。`builtins::conversion` 仅通过
`yss-database-arrow::convert_semantic_values` 的精确能力调用处理已物化值，输入输出使用中立
`TabularScalar` 和 `ConversionMetadata`。表格物化复用 `materialized_column`，再直接组装 Arrow 批交给 `RelationFactory`，不再用 `TabularSnapshot` 作为计算中转。Arrow 适配器复用 `PreparedConversion` 的转换与校验。
已物化的分类、顺序、日期时间及标识输出通过 `RuntimeValue::with_metadata` 保留含义、值域和已确定的时间表示。标注载荷只允许标量或平坦标量列表，字段私有，不能嵌套标注或包装资源句柄；后续转换继承元数据，展示端剥离标注投影原值。惰性数列通过 `RelationHandle::convert_series`
生成表达式，持有共享的预编译转换对象；每个批次继续执行值域、精度和范围检查。数值表示策略、Null 和失败规则见内置节点帮助。
目标语义为 Auto 时，内核使用调用方提供的已解析输出语义，不读取图连接或从输入值猜测目标；未确定或冲突的自动目标由 Graph 阻止进入执行计划。

Execution 的 [kernel_invocation.rs](../yss-graph-execution/src/kernel_invocation.rs) 负责从计划生成这些信息。它在准备好的资源绑定中解析资源参数，保留图端口地址、Schema 血缘和结果类别，并将返回值映射到对应输出。内核不接收 `GraphDocument`、`PlanOutputRef`、项目状态或资源授权服务。

`KernelError` 表达维度、参数、行对齐、预算、调用契约、科学计算、取消和超时等稳定原因，不携带图地址。Execution 的 `OperationExecutionError` 补充图来源与阶段，IPC 和前端保留对应 `RunFailure` 错误码。ResultStore、结果引用、租约、保存和运行生命周期仍属于其原所有者。

## 模块

| 模块                                           | 职责                                                      |
| ---------------------------------------------- | --------------------------------------------------------- |
| [identity](src/identity.rs)                    | KernelId、参数键及能力指纹                                |
| [invocation](src/invocation.rs)                | 中立调用、输出元数据、取消/deadline 和计算预算            |
| [value](src/value.rs)                          | 标量、列表、记录、关系/数列句柄与原生线性回归 运行值      |
| [registry](src/registry.rs)                    | 注册一致性、冻结指纹、能力查询、执行契约与输出数量检查    |
| [builtins](src/builtins/mod.rs)                | 内置注册装配及逻辑、常量、转换等适配                      |
| [numeric](src/builtins/numeric.rs)             | 已有标量/数列四则运算                                     |
| [relational](src/builtins/relational.rs)       | 已有 DataFrame/数列关系操作和按输出 Schema 拆列           |
| [statistics](src/builtins/statistics/mod.rs)   | 按回归、因果、面板、时间序列和诊断分类的 SCI Runtime 适配 |
| [visualization](src/builtins/visualization.rs) | 可视化数值输入准备与 SCI 绘图数据适配                     |
| [alignment](src/builtins/alignment.rs)         | 数据帧时间网格与面板对齐的原生关系计划请求                |
| [transforms](src/builtins/transforms.rs)       | 数列变换、排序、筛选、重塑与标量聚合结果消费              |

科学计算继续调用 `yss-sci-runtime`，数值算法属于 SCI，关系执行使用 `yss-relational-contract` 的句柄。这里不直接依赖 Graph、Project、Application、Tauri、DataFusion 或 Linalg。

`statistics::multivariate` 注册多元分析的七个目录 ID；数值列共同消费并证明行域，分类训练标签用 `TabularScalar::compare` 精确编码。
判别的新预测组独立验证其自身对齐，复用训练尺度并恢复原始标签/语义。输入、密集工作区及得分的运行值/Arrow 转换一起准入，再调用 Runtime。
摘要通过已有 typed finite-value 转换；得分/主坐标按 Invocation 中 Graph 已解析的字段，通过 `relational::materialize` 进入共享查询引擎，保持原行顺序但建立独立计算行域。
CCA 的 X/Y 轴组合成一张得分表以支持相互比较。观测数据不复制进内联摘要，坐标表和预测数列沿用现有分页。内核不计算输出 Schema 或图血缘。

`statistics::anova` 注册七个方差分析入口，通过已有 `series::load` 联合物化关系数列，或读取等长内存列。
响应与连续协变量要求无损数值转换；因素及受试者以 `TabularScalar::compare` 精确编码，宽整数标签不转浮点，结果恢复原因素标签。
因素数、重复组布局、平方和/交互选项与 Catalog 同步；输入、密集工作区和报告一起检查预算，再调用 SCI Runtime，并保留取消/deadline 错误。
只接受已证明对齐的关系列或等长内存列，不混合两者，不删除缺失观测；唯一输出为结构化 `statistics.report`。

泰尔指数适配位于 `statistics::descriptive`：个体形式只接收 `series`，分组形式必须另接一个 `weights` 数列（组人数或人口占比）。输入通过共用 `columns` 执行受控物化和行对齐检查，再调用 SCI Runtime；唯一输出为包含指数及描述字段的结构化 result。分组输入为组均值，仅计算组间差异。切换形式后，不适用或缺失的 weights 明确报错。

同一适配模块注册 Gini 和 Dagum Gini。Gini 复用数值列读取；Dagum 复用 `series::load` 联合投影数值与标签、证明行对齐，内存列复用 `series::column`。标签使用 `TabularScalar::compare` 精确分组，再以中立组编号调用 SCI，输出时恢复原始标签，宽整数不转为浮点。输入、排序缓冲和组对输出在调用前共同检查预算，不以固定组数截断。仅在执行时读取数据，算法与中断控制继续由 SCI 拥有。

统计结果进入 `common::value` 时保留原有数值类型；转换入口在 JSON 编码前遍历并拒绝
非有限浮点数，返回 `NonFiniteResult`，合法 `Option::None` 仍转换为空值。
ACF/PACF 与 Hausman 使用 typed report，避免先经 `json!` 把数值错误抹成 Null。
已经构造的 JSON 无法恢复被抹去的数值类型，生成这类报告的 owner 必须在编码前完成检查。

相关与一致性适配位于 `statistics::association`。数值列复用 `columns`，Ordinal/分类列复用 `series::load/column` 和语义元数据；配对列共同投影验证行域，Ridit 的独立样本分别读取。加权 Kappa 要求明确类别顺序，宽整数类别通过精确比较编码并在输出时恢复；不以 Float64 合并标签。Kappa 和 rwg 的条件参数在声明、内核可选性与执行校验中保持一致，inactive 参数不进入调用。排序、残差化、置换、方差与推断属于 SCI；调用前按实际规模检查工作区（包括 Cohen Kappa 的类别平方表和判别分析的类别协方差），执行中转发取消/deadline。
共享分类编码使用精确标量排序索引，避免逐行线性扫描全部类别，保留首次出现与显式有序元数据顺序。

## 注册与扩展

`builtins::visualization` registers all 19 plot adapters. Paired/matrix inputs use
joint relation projection or equal-length memory columns; box/violin groups are
independent samples. Missing values are rejected. ROC respects declared positive
Binary codes, including inverted Boolean meanings. Coefficient plots consume
native OLS/WLS/GLS models. Adapters share controlled materialization, budgets and
finite-result encoding, then call `yss-sci-runtime::visualization`.
Each result is one `plot.data` record; rendering does not run inside a kernel.

`KernelRegistryBuilder::register` 接收 KernelId、非零实现 revision、KernelContract 和执行函数。KernelContract 声明有序输入键及数量范围、实际参数键集合和输出数量范围；它不复制 Catalog 的分类、本地化文本或完整配置模型。

节点参数按参数 key 扁平传入，内核通过 `invocation.parameter(key)` 读取。参数组只属于声明与编辑呈现，
不会形成运行期 Record。`with_optional_parameters` 将已声明的条件参数标为可选；组装时同时核对参数键和
可选性，运行时拒绝未知参数及缺少必需参数。Graph 在准备计划时解析默认值并排除不适用的条件参数；
内核按所选算法检查适用字段。可选参数集合进入能力指纹，分布、比较、整数范围和线性 Fit 的实现 revision 已随调用契约更新。

`with_builtins` 组合已有内置实现；Application 可以在冻结前加入扩展。冻结后所有调用使用同一能力指纹。指纹继续采用 `yssbi.kernel-registry.v1` 编码，覆盖排序后的 ID、revision、输入布局、参数键和输出数量。具体已安装能力以 [builtins](src/builtins/mod.rs) 的注册表为准；未安装的节点仍返回缺少执行能力的诊断。

新增节点时在对应方法族模块实现适配，再加入内置装配或由应用 provider 注册；实际算法放回 SCI 或相应数据所有者。执行函数只使用已经解析的类型和资源，不重新求解图类型，也不自行读取项目资源。

实现行为变化需要递增 revision。输入布局、参数或输出形状变化需要同步节点声明和消费者，并复核解析与计划缓存的能力身份。透明重路由不产生执行操作，也不注册无效的同名内核。

## 验证

独立调用测试检查输入顺序、输出载体和资源控制；Application 集成测试检查内存表的组合、拆列顺序、关系计算与分页。扩展注册、配置、数值执行和结果查询测试继续覆盖跨边界行为。运行命令见 [Rust workspace README](../../README.md)，范围选择见[根规则](../../../.rules)。

## 内置节点语义与转换

逻辑比较统一为六个节点，标量返回 Binary，任一输入为 DataSeries 时逐元素输出 Binary 数列，支持任一侧标量广播。`NodeTypingSpec::BinaryPredicate` 要求输入元素语义一致并推导二元输出形状，供比较及 AND/OR 复用；反向约束同时支持自动转换经这些端口传播语义和形状需求。等于/不等于支持七种基础语义的值比较，排序比较支持 Numeric 和 Text，不根据分类编码隐式推断等级。
内存数列必须等长，惰性数列必须属于同一关系行域，不混用未对齐的内存列表。任一元素为空时输出空值。Tabular Contract 拥有精确标量比较，Kernel 处理内存值，DataFusion 对相同物理类型使用 Arrow 比较，对混合整数/浮点复用精确标量比较，避免优化器的隐式浮点提升；不支持的表示失败。比较实现 revision 参与已有能力指纹及执行缓存。
原数据序列数值/字符串比较节点已移除。
AND/OR/NOT 接受 Binary 标量或数列，采用 SQL 三值逻辑，内存数列要求等长并支持标量广播。NOT 保持输入类型和形状，使用类型恒等规则但仍是执行叶节点。惰性输入通过关系契约构造 DataFusion 原生 AND/OR/NOT 表达式，在结果消费时执行，不由布尔运算节点物化整列或封装自定义布尔 UDF；具备正值映射的非标准二元编码先复用语义转换规范化。关系行域必须一致。这些节点属于数据计算，不构成图的控制流或上游短路执行承诺。

类型转换由 `yss-data-contract::SemanticConversion` 定义目标语义、数值表示、显式值域、时间形式/精度与解析格式，支持全部七种 Semantic。
`ShapePreservingConversion` 按输入形状推导标量或 DataSeries 输出；原有按 Int64/Float64/String 组合拆分的数列转换定义已移除。
目标语义默认 Auto，通过解析器内部的单调类型域约束求解下游输入 Pin 的交集，支持重路由及泛型端口反向传播；不由输入语义或数据值猜测。未连线或交集仍有多个候选时保持未确定，空交集为类型冲突，均不生成可执行 specialization。反向约束进入自动转换节点的解析缓存指纹，重连及下游类型变化会重算目标；持久化参数仍为 Auto。内核只消费当前不可变计划中已确定的输出语义。
反向约束仅在包含自动转换节点的连通分量中分配类型域并求解；完整图仍由已有正向解析生成最终语义快照。
数值表示的自动模式保留已有 Numeric 物理表示，文本解析为 Float64，Binary 转为 Int64；整数/实数模式显式选择 Int64/Float64。
Null 保持 Null，非有限值、溢出及有损数值转换失败。目标配置、默认值和类型推导均由 Rust 拥有。
Kernel 的标量和内存数列通过精确授权的 Arrow 适配入口共用批次转换规则，中立值直接构建 Arrow 数组；惰性数列通过关系契约生成 DataFusion UDF。
`PreparedConversion` 在构造时解析字段、确定转换操作，并预编译值域及数值边界校验；每个批次只转换和验证实际值。其校验实现与数据集导入/编辑共用。UDF 持有不可变的共享转换对象和输出字段，执行时不重复解析字段元数据或重建值域。
UDF 的相等性和哈希包含源/输出字段元数据与实际转换操作，不包含未生效的配置；输出字段携带 Semantic 和 nullability，转换保持来源关系与行对齐。
分类值域可继承已声明的分类/顺序/二元值域；新建顺序必须显式配置等级或继承已有顺序。类别编码、标签及等级顺序不按批次重建。Identifier 保留原始值，不附加唯一性要求。Datetime 复用无时区日历转换，支持显式日期/时间形式、精度及格式，拒绝无提示的精度损失与数值时间戳猜测。
`RuntimeValue::with_metadata` 为已物化的分类、顺序、日期时间和标识值构造 `Annotated`；私有载荷只允许标量或平坦标量列表，不能嵌套标注或包装资源句柄。`ConversionMetadata` 的时间表示使用已确定的 `TemporalType`，不携带配置中的 Auto。执行缓存保留该标注，转换会重新使用它。结果展示/分页只投影原始值，通用值比较比较底层值；元数据不作为可变前端副本或新的图状态 authority。
构建表达式不读取整列或修改数据集；错误在实际消费时返回，结果预览失败不改写已完成的 Run。完整列校验不能从局部预览或 LIMIT 的成功推断。

## 关系运算与统计适配

频数、数据序列描述、数据帧描述和分组聚合由 `builtins::aggregation` 注册。内存数列复用既有 Arrow 物化入口，再与关系数列共用 `RelationHandle::frequency/describe/aggregate`；数据序列常量保留已声明的分类含义，Binary 元数据也保留，避免按整数编码误判 Numeric。关系执行由 DataFusion 原生聚合、排序、连接和分位数计划承担，输出保留来源绑定、租约及受控分页能力。节点执行仅构造计划，扫描及数值失败在消费时交付；数值输入无损提升，非有限结果通过类型化错误传播。完整口径见 [Catalog](../yss-node-catalog/README.md)。

仅在类型契约要求时提升为 Float64；数列的元素提升和标量广播由计算 kernel 处理，调度器不制造数列长度。
文档数列常量一次性导入 Arrow 字面量表达式，和范围生成节点共用显式位置坐标；等长常量数列可逐元素运算，不同长度返回 ShapeMismatch。带关系身份的数列保留固定行域和文件租约，
通过 DataFusion 原生表达式及 Arrow 批运算执行，不在节点求值时整列 collect。
原始列和计算数列都由 `SeriesHandle` 持有 adapter-owned 表达式，表达式不进入持久化 Graph 文档。
数据帧组合使用原生 Union、Join、Projection 及用于稳定顺序的 Window/Sort 计划。按行拼接保留重复行、输入顺序及各来源行序；列名模式补齐缺失列，位置模式使用首表列名。公共列要求相同物理类型和兼容语义，不进行隐式有损提升。连接支持多列键和 inner/left/right/full/semi/anti，空键不匹配。半连接和反连接只返回左列及稳定的左行顺序；其他连接保留全部匹配组合和两侧键，右侧重名列使用后缀消歧。
`RelationHandle::bindings` 保留全部来源绑定，组合执行检查项目会话、执行环境及同一数据集的快照一致性。来源租约由组合结果和输出流共同持有；资源准备入口仍要求每个数据源授权值只有一个匹配绑定。多来源结果不伪装成单一数据集，自动化只读检查按预算列出其来源。
DataFrame 字面量和内存列组合在 Kernel 内通过 Arrow 适配器物化列及语义元数据，以 `RecordBatch` 交给注入的 `RelationFactory`，一次性导入同一 DataFusion runtime，之后复用上述关系运算及分页。`TabularSnapshot` 只用于文档和展示边界，不作为内核物化的中转。字面量关系的来源绑定为空；空绑定不授权任何外部数据集。普通结构化值仍使用 Record。
DataFusion adapter 单独持有行域及域内列表达式。筛选列、重命名和数列投影保留行域证明；筛选行、截取、行拼接和连接建立新行域。按列拼接与组装只组合可证明对齐的惰性输入，不以长度或行号猜测独立数据帧的对应关系。组装也支持等长内存数列，保留其已物化形式，不混用惰性与内存数列。
源行顺序的内部字段仅留在 adapter 的域计划中；组合时由显式排序的行号窗口生成内部位置，确保分页稳定，不复用源表的可编辑行标识。输出只暴露用户列，并保留语义元数据；组合输出的字段身份由图的派生 Schema 负责。动态输入顺序进入 Schema 缓存指纹，连接键编辑器和校验分别使用对应一侧的 Schema。
同一关系的数列可以连续运算并联合投影，名称重复的计算列按投影位置区分；不同关系或无对齐证明的物化列表
不能按长度相同与关系数列拼接。Results 分页调用数列表达式投影，不按其显示名回查基表字段。
标量除零在节点求值时拒绝；数据内的 NULL、除零、非有限值或溢出在批流实际消费时返回类型化错误。
幂和任意底数对数复用同一算术契约及形状推导，两个操作数都是可连线的数值输入，输出为 Float64。`NumericOperation::evaluate_float` 统一内存和批执行的实数计算、定义域及非有限结果规则；整数输入仍须无损提升。独立指数函数节点由幂节点设置底数 e 替代。
自然对数、以 2/10 为底的对数、平方及平方根采用单输入 Numeric 操作，复用相同执行管线并保持输入形状；统一输出 Float64。`accepts_arity` 约束操作数数量，`evaluate_unary_float` 使用对应的实数函数并统一定义域/非有限值检查。数列按批惰性计算，空值和非法定义域均报错，不隐式填充 null。
分页、统计输入消费沿用各自的取消、deadline 和内存边界，不将非法计算值写成正常结果。

线性回归 Fit 从节点参数构造 `OlsOptions` 和 `LinearRegressionMethod`，由 Node Kernel 调用 `yss_sci_runtime::regression::linear::linear_regression`，传入本次执行的取消标记和 deadline。OLS/WLS 支持截距、Nonrobust、HC0–HC3、HAC、Newey-West、Fixed Scale 和 Cluster；Cluster 的单个 `clusters` 输入与响应、自变量及权重共同对齐。GLS 接收相对误差协方差矩阵并估计尺度，标准误仅支持 Nonrobust。Summary 读取上游原生模型，不调用拟合；Predict 复用训练系数和截距。

Logit/Probit/Prais、IV 2SLS/LIML、Panel、TWFE DID、ADF、VAR/VEC 及阶数/协整秩检验均有执行适配。模型以不可变 Record 保存中立拟合契约，Summary 在预算及执行控制检查后解码为对应契约，交给 Runtime 组装所选报告并调用需要的 SCI 分析；Fit 不附带完整报告。Logit/Probit Predict 使用既有系数。Panel/VAR/VEC 只输出模型，DID 输出模型和报告，不把未提供或多方程的观测结果伪装为单个拟合数列。参数组合和输出含义见 [Catalog](../yss-node-catalog/README.md) 与各节点帮助。

独立诊断复用上游线性模型的观测、设计列、协方差及 WLS 权重；VIF、杠杆值、BP/White/IM/RESET、BG、系数 t/Wald，以及序列正态性、DW、Ljung–Box、ACF/PACF 通过对应领域适配。VAR 的 Granger/IRF/FEVD 与非稳健 2SLS 的 Hausman 在对应节点执行时计算，IRF/FEVD 的 steps 控制分析范围，不重新拟合上游模型。IV/VAR/VEC Summary 只计算所选诊断。DID 伪处理组随机化显式接收 treat/post 与随机种子，在置换之间检查取消。KDE 经 visualization 适配输出绘图数据，默认 256 个网格点，可在结果面板和 Plot 窗口查看。

数据帧对齐接收表和列名参数，通过关系契约构造原生窗口、聚合、range/unnest 和连接计划，不收集输入批次、不调用 SCI 对齐。时间序列使用原始时间单位的等距网格；面板使用共享已观测时间序列的位置，在每个实体自己的起止位置内补齐。原列类型、列序和元数据保留，补入的非键值为 Null。重复键、缺失键、网格与输出预算校验保留在计划表达式中，在消费时交付失败。

`LinearRegressionValue` 共享不可变拟合模型；Summary 另持有本次 `LinearSummaryOptions` 和选中检验的不可变结果。ACF/PACF、序列相关和假设检验在 Summary 执行时按选项计算，未选项不调用 SCI。模型拥有有界 memo，每类分析只缓存最近一组参数；补选复用相同模型和参数的结果，参数变化重新计算，新 Fit 使用独立缓存。计算不持有 memo 锁，遵守调用预算并在分析间检查取消；旧 Summary 继续持有原分析快照。

Runtime 将普通数组交给 SCI 拟合；SCI 通过 `yss-sci-linalg` 封装的矩阵计算，faer 不越过 Linalg 边界。Summary 的 ACF/PACF、序列检验和假设检验统一由本 crate 的 `linear_summary` 从同一模型构造输入并调用 runtime。Application 只读取选中项的已存结果，保留请求范围、会话与结果有效性检查；SCI 完成约束解析、线性化和 t/Wald 检验。

概率分布目录的 23 个采样节点由 Node Kernel 解析配置，调用 SCI Runtime 的 `sample_into`，
再由 SCI 使用既有 statrs/rand 实现。中立的分布参数与整数/浮点采样值由 SCI Contract 定义。
采样节点无数据输入，执行时生成物化数列，保持 NonDeterministic 与 Disabled cache；图分析不采样。
Kernel 在分配前检查样本缓冲及运行时输出的总预算；SCI 写入调用方提供的切片，不另存全量副本。
分布构造前校验参数范围与有限性，各样本间检查取消/deadline；超几何整数抽取循环也定期检查控制。
底层库内部的单次采样不承诺即时中断。数值溢出或不能表示的样本使整次节点执行失败，不返回部分结果。

几何分布计数包含成功试验，负二项分布计数仅包含失败次数；Gamma 使用 shape/rate，
Inverse Gamma 使用 shape/scale，离散均匀分布包含两个端点。涉及浮点计数的二项试验数、
Erlang 形状及泊松率限制在 2^53 内，泊松/负二项/几何输出超出精确计数范围时报错。
离散均匀分布和超几何抽样使用精确整数，不通过 Float64 中转。

`yss-node-kernel` 的统计适配、`yss-graph-execution` 的结果分析及独立 OLS benchmark、`yss-application::ipc` 的独立统计命令直接调用 runtime。独立统计命令在 IPC 层转换中性请求/结果；ACF/PACF 命令保留会话准入检查和 60 秒 deadline。桌面入口和普通 Application 模块不注入或持有科学后端对象。取消与 deadline 保留同步计算前后的检查，不承诺中断正在进行的矩阵分解。通用数学语法由 `yss-math-expr` 拥有。

Drop NA 的检查列复用列选择器，参数投影通过 `allowEmpty` 允许清空选择以检查全部列；
已有的投影选列参数仍要求非空。输入列结构尚未确定时，选择器显示延迟确定提示，不触发扫描。

DataFrame 的 `dropna.rows` 生成原生 Null 过滤计划，保留列结构；`dropna.columns` 生成
数据依赖的延迟选列请求。两者的节点调用及 Graph Resolve 都不扫描行。Graph 用 `Deferred`
标记后者的列结构，并沿缺失值处理及 Limit 传播；依赖固定列名/类型的下游端口仍要求 Exact。
这一状态由 `GraphSemanticSnapshot` 拥有，消费结果不会回写或替代语义快照。

Relation 的 `schema_is_deferred()` 区分未确定的 Schema 与真正的零列表；Deferred 时 `schema()`
的空标记不能作为列清单。分页先通过 `resolve` 消费原生 count 聚合的一行结果，判断完整输入的候选列是否含 Null
或非 Null，再构造投影及分页计划。批流消费也经过同一解析入口。只缓存成功解析的不可变关系计划，
不缓存全表数据或失败；来源绑定、租约、取消、deadline 及批内存预算贯穿统计和输出阶段。
统计不把 NaN/空字符串/零值算作缺失；空表保留列，零列结果继续保留行数。

Node Kernel 已注册 DataFrame source/project/filter.rows/series.select/decompose/limit/rename kernel。它们组合
`yss-relational-contract` 的关系句柄，DataFusion 原生计划保持在 `yss-database-engine` 内；计划构造不 collect。

数据序列目录的整数序列、长度、非空计数、求和、均值、标准化、逆标准化、虚拟变量信息、
时间差分、变化率、滚动统计、滞后及面板差分均由 Node Kernel 请求原生关系运算。Graph Resolve 不执行计算。
数列变换返回 `SeriesHandle`；长度、计数、求和和均值构造原生聚合并只读取标量结果，不在内核保留整列。数值运算拒绝非有限值或有损提升；空值规则由各节点帮助明确。
标准化使用样本标准差并分别输出均值和标准差，不存在额外 Transform 句柄。
标准化与逆标准化统一通过 `SeriesTransform` 请求；窗口聚合的 `WindowOperation` 只表达数值聚合，排名和方向填充使用各自的变换请求。
标准化通过原生 avg/stddev 聚合取得两个标量，标准化和逆标准化输出保留原行域的惰性表达式，
通过窗口与投影计算，仍可与原列比较或组合。数列常量以显式位置坐标导入，已有等长常量数列的逐元素运算继续成立；该坐标不授权数据帧或数据集间按长度拼接。两步往返不保证浮点逐位相同。
六个逻辑比较节点默认精确比较，可在配置中切换到数值容差模式，并显示绝对/相对容差字段；默认 atol=1e-12、rtol=1e-9，
标量、物化数列与关系表达式共享 `NumericTolerance` 判定。Null 传播，容差必须有限且非负，
到 Float64 的有损转换被拒绝。容差模式采用对称公式 `abs(a-b) <= atol + rtol * max(abs(a), abs(b))`，先将容差内的两值视为相等，再应用比较运算符，容差外保留数值顺序。运算符及容差参数参与表达式身份，不能被优化器误合并。布尔运算不需要数值容差。容差关系不具备传递性，不用于排序或分组。

面板差分显式选择上下文数据帧的实体列和时间列，与待计算数列联合投影以证明行对齐；
拒绝缺失及重复键，组内排序计算后恢复原始行位置，不推断日历间隔或删除缺失行。
参照组提示由 `ConversionMetadata::dummy_base_level` 持有，组装 Arrow 列时写入
`yssbi.dummy_base_level` 字段元数据；不会改变分类值，也不会隐式启用回归的虚拟变量展开。
Decompose 的每个动态输出在 semantic snapshot 中携带单列 Schema（当前列名、类型和 lineage），
计划准备将其保留到输出契约。执行按该列名返回共享上游关系的 `SeriesHandle`，不按端口顺序或显示标签猜列，
也不为每列提前扫描数据；下游统计消费或 Results 分页时才交由 DataFusion 投影和读取。
文档内的 DataFrame 与 DataSeries 常量在常量入口导入，后续选列和变换均传递惰性句柄。
关系和数列持有固定 session、dataset snapshot/revision、查询上下文及文件租约。资源准备检查句柄内的
session/revision 与已授权资源一致。OLS 可以接收既有数值列表，或来自同一个关系句柄的原始/计算数列；后者共同投影、
消费异步 Arrow 批流，并使用独立输入内存预算准备数值矩阵。NULL/非有限值仍被拒绝，不隐式逐列删除缺失样本。
不同关系上的数列不能按长度相同直接拼接。统计方法与可用选项以各领域适配器、节点参数和实际冻结注册表为准。

Parquet 关系数据源要求精确 Schema 显式标记独立的 RowId 与 DisplayOrder 列；读取按 DisplayOrder、RowId
确定顺序，再向 Graph 投影用户列。统计输入与拟合值/残差不会以并行批次的到达顺序替代表格的显示顺序。
文档内的物化常量使用其不可变行域内的顺序，不将文件/batch offset 当成项目数据集的长期身份。

真实项目的 DataFrame 资源现由 DatabaseRuntime 捕获 catalog 快照，并以 Project grant 的版本封装关系句柄；
固定 Parquet 链路与真实项目的 CSV 导入 → Graph Execute → OLS → Results 分页均已通过集成验证。
最终提交使用准备时捕获的同一组资源授权，Project 在发布前再次检查版本；不以空授权跳过数据集依赖。
关系候选 Results 持有懒句柄，不保存所有中间批次；计划准备不表示全部行已经成功扫描，分页扫描失败通过结果读取错误交付。
数据存储与查询边界见 [Dataset store](../yss-database-store/README.md)，局部性能测量见[数据引擎基准](../../../docs/benchmark/DATA_ENGINE_BENCHMARK.md)。

The built-in classical hypothesis-test adapters live in `builtins/statistics/classical.rs`. They translate node inputs into neutral `yss-sci-contract::hypothesis` requests, invoke stateless SCI runtime functions, and expose one structured `result` output. The catalog owns localized node definitions and help; kernels do not duplicate formulas.

Binary Summary computes selected odds ratios (Logit only), continuous-regressor
margins, in-sample classification and coefficient restrictions through SCI. Prais
and IV Summary share the existing contrast engine with estimator-appropriate t/F
or normal/chi-square distributions. Data-series labels are captured at Fit.
Panel Fit emits estimator-scale fitted/residual lists and retains source-row groups;
Panel Predict consumes that scale, while Panel Compare reports each requested
estimator's success or scientific failure independently. ADF accepts multiple aligned
series with per-series outcomes. VAR exposes selected lag lists, contemporaneous
exogenous columns, intercept and covariance df adjustment. These changes directly
replace the current unpublished contracts; no compatibility path is maintained.

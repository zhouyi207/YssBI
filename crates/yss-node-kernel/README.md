# yss-node-kernel

> Status: Current
> Scope: 不依赖 Graph 的内核调用契约、运行值、冻结注册表和内置节点执行适配
> Canonical owners: 本 crate 源码拥有内核能力；图计划、资源授权、结果生命周期由 [Graph 与 Execution](../yss-application/src/graph/README.md) 定义
> Update when: 内核调用契约、注册/指纹规则、控制边界或内置适配改变时

本 crate 持有可执行函数。Node Protocol/Registry/Catalog 持有节点声明和目录；Application 的 `NodeComponents` 核对声明与内核契约，冻结一份注册表，供同一会话的能力检查和实际执行共同使用。

## 数列读取与逐项计算

`builtins::series` 是数列物化与长度检查的共同入口，保留整数精度、空值和语义元数据，并累计常驻输入与临时缓冲预算。配对列按当前行位置对应、要求等长；独立样本分别读取、允许长度不同。来源关系不决定数值能否参与计算。

`series::positional::prepare` 为算术、比较、布尔和数列组装准备输入；同一行域保留惰性表达式作为优化，独立来源或混合内存输入先物化再计算。`series::columns` 服务带语义的统计与绘图输入；`series::numeric` 保留紧凑 f64 读取路径，供回归和数值统计使用。Mann–Whitney、Kruskal–Wallis、Mood、方差齐性等独立组入口使用独立样本模式。
输入准备直接借用调用切片，在原输入中检查数列行域；准备后的值只在需要物化时取得独立内容。

条件选择、替换空值、日期差与文本拼接复用按位置组装的临时关系及既有变换算法。`series::positional::attach` 在设置列、掩码筛选及窗口上下文中按位置附加计算结果；源表通过 Arrow 批拼接保留物理类型、列顺序和字段元数据，内部附加列不会泄露到表输出。窗口继续按指定分组/排序键计算并恢复输入顺序。临时关系拥有物化数据，不授予额外数据源权限；Join、数据帧按列拼接及时间/面板网格对齐仍遵循各自显式关系契约。

## 调用边界

事后两两比较的表输出同时包含数值 `group_a/group_b` 和文本 `group_a_label/group_b_label`。标签直接取自本次计算的原始分组映射；宽整数按精确十进制文本保留，消费者不必通过均值差或摘要位置猜测组名。

`kernel_error` 在公共错误模块统一映射关系错误，叶内核与 Execution 的结果物化共用它，
保留取消、期限、预算及数值失败语义。执行边界和结果存储继续由 Execution 管理。

`RuntimeValue::matches_carrier` 为内核注册表及图函数调用共同校验运行载体；它不推断图类型或改写元素语义。
图函数的参数绑定、私有帧与嵌套调度属于 Execution，叶内核继续不读取图、函数定义或项目状态。

GroupBy 叶内核只构造 `RuntimeValue::Grouped`，检查源表及分组键，不遍历组或调用函数。
载体检查只允许该值匹配 `tabular.grouped_dataframe`；普通 Record 不能冒充分组值。
分组结构不一致和键列重名分别保留 `GroupSchemaMismatch`、`GroupKeyCollision`，关系与图执行共用映射。

统计节点的主因变量和主自变量输入键统一为 `y` 和 `x`，固定或重复输入仍由既有布局声明区分。
Catalog 与 Kernel 注册、分组读取和 Graph 调用共用同一命名；科学计算记录的领域字段与结果表字段保持各自契约。

`builtins/statistics/decision` 分别读取共同对齐的指标列与独立行域的指标权重向量，
按实际行列数、SVD/相关矩阵工作区和关系输出合并预算。权重长度匹配指标数量，
不要求权重行域与观测行域相同。摘要恢复指标名；逐行评分进入固定字段可分页表，
归一化权重另作数列输出，支持“赋权 → 评分”的图组合。

Decision 适配按 `columns`、`ranking`、`systems` 拆分；只给独立性赋权计入 SVD 工作区。
VIKOR 保留折中条件和集合，耦合/障碍度的无定义单元通过共享数值表转换保留 Null。
该转换仍拒绝 NaN/Infinity，固定表字段继续由 Catalog 持有。

`decision/preferences` 适配 NPS、KANO 与客户级 RFM；固定输入列复用共同对齐与
预算检查，KANO 无定义系数保留空值，RFM 全部观测评分输出为可分页关系。

`decision/market` 适配 PSM 全价格曲线与 TURF 精确组合结果；`matrices` 适配
AHP/FAHP、DEMATEL 与 ISM，按方阵分解和完整关系输出预算，不限制阶数。
`fuzzy` 单独处理隶属度矩阵以及独立行域的指标权重/等级分值；`experts` 输出
单轮德尔菲摘要和逐项评分统计。矩阵语义、循环结构和统计合成都由 SCI 持有。
`conjoint` 将评分和分类属性送入评分型联合分析，保留原始水平标签，
按设计矩阵和协方差工作区预算，输出效用摘要与完整拟合值关系。

`builtins/statistics/psychometrics` 将共同对齐的题项列用于信度、内容效度、
题项区分度和 KMO/Bartlett 检验，恢复题项名，输出可分页题项统计和全部观测分组。
只有相关矩阵诊断计入平方矩阵工作区；Alpha 使用线性内存总分统计。

`builtins/statistics/quality` 适配单值/移动极差控制图、过程能力和均衡交叉 Gage R&R。
子组及零件/操作者标识复用精确分类编码；控制图同时预算完整绘图点和可分页观测表，
不以显示抽样隐藏越界观测。统计假设和随机效应检验分母由 SCI 持有。
`builtins/statistics/doe/range` 保留实验因素原始水平，输出水平汇总关系。
`doe/design` 在生成前按完整设计矩阵和关系转换计费。`common/tables` 同时供试验设计和
多元坐标结果转换使用；不截断行或列。
`doe/models` 在二次项展开前计费，复用回归契约，只将紧凑拟合摘要放入报告；
完整观测、拟合值和残差通过关系输出。
预算按实际水平数量及正交性列联检查工作区计算，不限制试验次数或因素水平数。

`builtins/transforms` 将单次插补的方法交给关系引擎，均值、中位数、数值众数和常数
共用一个中立操作请求。适配器不收集整列，执行继续受关系查询的内存、取消与期限控制。

`builtins/statistics/inference` 适配置信区间、多重比较、单维聚类稳健标准误和调整预测。
前两者的逐行结果通过 `common/tables` 转换成 Catalog 声明的固定字段关系，报告只保留摘要。
`common/models` 统一借用线性拟合或预算化解码二元拟合，供模型诊断与调整预测复用。
原始观测通过共享物化路径按位置核对长度；聚类标识保留精确类型。输入、工作区和输出合并
预算准入，不设置行数上限。调整预测按已存设计评估，不隐式重建交互项。
聚类推断保留 SCI 的形状、样本不足与数据定义域分类；不足两个聚类返回数值输入错误，
不归为参数错误。`inference.cluster_robust` 使用 revision 6，并复用 SCI 的稳定 Student-t 尾概率。
共享设计准备的样本不足与不可辨识数据保留为数值输入错误，非法 tuning/迭代设置仍为参数错误。
回归模型按其实际参考分布维护能力版本：层次、逐步及 GLM 使用 revision 6；曲线、RCS、阈值、回归流程以及正态 Wald 推断的模型入口
使用 revision 5；两个非线性入口与 Deming 使用 revision 4。Ridge/Lasso/PLS 不输出系数 p 值，
沿用各自能力版本。共享设计校验的五个参数生存拟合内核
（Exponential、Weibull、Lognormal、Loglogistic、AFT）采用 revision 9。
复用该准备的 Mixed/GEE 及返回正态 Wald 推断的因果估计采用 revision 6；共线性诊断使用
revision 5。Meta 模型、逐项排除和敏感性使用 revision 8，Egger 与 Begg 使用 revision 7；
绘图和仅返回异质性统计的入口维护各自的能力版本。能力指纹涵盖共享输入错误契约和实际尾概率计算。中介 bootstrap 采用 revision 8，同时涵盖稳定分位数二分点和
共享 Student-t 尾概率。

`builtins/statistics/diagnostics/models` 接入共线性、Harman、NRI/IDI、残差/Cook、
AIC/BIC、LR/Score/嵌套比较及 Cox PH 诊断。模型输入复用原生线性值或预算化的二元模型
解码；原始列共同物化并检查配对长度，二元结局支持 Bool 与 0/1。模型、矩阵、编码和观测表
合并预算准入，取消和期限传给 SCI。残差/Cook 的逐行数组物化为一个可分页关系，
摘要只含汇总字段；未定义的影响指标为 Null。算法及推断由 SCI 拥有。

`builtins/statistics/spatial` 注册空间分析的 10 个入口。权重对象携带精确地区标识，
下游共同物化观测列后复用分类编码匹配地区，拒绝重复、未知地区及不平衡面板，
不以向量长度代替地区对应关系。计算按权重地区顺序、时期首次出现顺序组织，
观测结果恢复输入行序；地区效应保持权重顺序。密集权重、矩阵分解、似然 Hessian、
物化输入和结构化输出合并预算准入，并向 SCI 传递取消和期限。结果复用现有报告页面。

`builtins/statistics/time_series/forecast` 注册时间序列分类的 16 个新增入口。
共享物化路径按位置核对列长度，保留现有行顺序；SCI 接收取消和期限，工作区、预测长度、
Markov 状态平方矩阵与结构化结果均在计算前按预算准入，不设置固定行数上限。
Markov 恢复原始状态标签，时序图可省略时间列并使用从 1 开始的横坐标；
显式时间须严格递增。时序图与相关图复用现有 SCI 绘图数据，其他入口返回结构化报告。

`builtins/statistics/causal/models` 适配新增计量与因果分析入口，复用共享列对齐、
精确分组编码和有限结果序列化。Heckman 仅在未入选行允许空结果；其余输入不静默删行。
SUR 将方程自变量索引解析为中立列表，并按所有方程的总参数规模预算密集系统。
数值工作区、bootstrap 顺序重拟合、结构化输出与常驻输入合并准入，取消和期限传递给 SCI。
ATE/ATT 复用预算化结果解码，保留上游处理效应的方法、目标样本数及可空推断，
不会将普通报告或原始数列解释为处理效应结果。

`builtins/statistics/longitudinal` 注册 GEE、HLM、LMM、GLMM 及六个设计/分布预设。
复用 `statistics/common/inputs` 的对齐物化、精确标签编码与数值读取，分组标签不转换为浮点。
按算法估算输入、工作区与结构化结果峰值；Gaussian 混合模型计入密集协方差与方差分量秩检查，
GEE/GLMM 按组计算，不计入未使用的全样本平方矩阵。保留 SCI 的取消、期限和未收敛失败。
返回单个结构化结果，分组标签按首次出现排序、随机效应通过从 1 开始的分组/级别位置引用；
所有观测数组保持输入行序。算法、边界与推断由 SCI/Contract 拥有，不新增结果页面。

`builtins/statistics/regression_models` 为回归目录的 31 个新增入口提供无图适配：
按本地输入键按当前位置共同物化并检查配对长度，将分类响应和分组编码交给 SCI，返回时恢复
原始标签和声明的有序级别。输入、密集工作区、批量模型结果和结构化序列化均执行
预算检查；SCI 接收同一取消标记和 deadline。每个入口只有结构化 `result`，
不创建报告端口或专用页面。原有 Linear/Logit/Probit/Prais 生命周期继续由原适配器维护。

回归工作区按实际自变量、截距、类别、非线性参数及展开后的设计列估算，数值缓冲按
`f64` 计费。分阶段回归按本次配置保留的模型数计费，分组回归的观测结果按各组总行数
计费；分类概率、样条基列和结构化编码另计。输入常驻内存加上拟合与结果编码两阶段
的较大估算值用于准入，编码估算复用 `common::value` 的容器计费口径。
所有图节点统一使用 `KernelControl::new`，输入、工作区和结果编码默认没有固定字节上限；
`max_input_bytes = usize::MAX` 也传给关系读取、函数调用、分组计算和结果快照。
独立库调用仍可显式提供有限预算；尺寸算术溢出、实际分配失败、取消和期限检查继续生效。

`KernelInvocation` 接收已求值输入、固定端口或重复组的局部键、有序输出类型/字段、已解析参数、`KernelControl` 和中立 `RelationFactory`。参数可以借用现有 literal 和已授权的资源运行值。内核返回按调用局部输出顺序排列的 `Vec<RuntimeValue>`；注册表在调用前检查输入布局，在返回后检查输出数量和外层载体，不重新推导 Graph 的元素语义。

运行值的标量载荷统一为 `TabularScalar`，有限浮点表示为 `Float64(FiniteFloat64)`；没有内核自定义 Decimal 算术类型。文档的 `DecimalLiteral` 是精确数字文本，在求值时转换。列表和记录通过不可变 `Arc` 共享。调度槽、下游输入和结果发布不再逐元素复制；统计拟合值和残差只在转换成运行值时分配一次，不先复制原数值向量。紧凑数值缓冲仍属于[后续评估](../../TODO.md)，当前模型向量与通用运行值列表保持不同表示。

DataFrame 常量和内存列组合统一通过调用方注入的 `RelationFactory` 进入共享查询引擎，后续选列、筛选、拆列和分页均使用 `RelationHandle`。`Record` 承载配置、结构化统计模型和检验结果。字面量关系不声明数据集绑定，不能据此获得外部资源授权；组合仍检查实际数据集的会话、快照及执行环境。

导入裸分类字面量时建立无序值域，已有显式值域和顺序级别继续保持不变；日期时间文本复用 Arrow 适配器的日历转换，去除时区时保留墙钟字段。
数列常量按已解析的元素语义导入关系句柄；Identifier、Datetime 和全空列同样保留声明的语义，不从底层文本或首个非空值重新猜测。

`KernelControl::check_bytes/reserve` 统一执行预算检查和可失败的预留。统计内存列在转换前检查完整形状和总输入大小；GLS 辅助矩阵、密集工作区及两份输出使用合计准入估算，预测和转换循环周期性检查取消。估算不等同于进程 RSS 上限，也不承诺中断正在执行的矩阵分解。

输出契约通过 `ValueType` 引用七种 Semantic，数值端口统一为 Numeric。四则运算根据实际标量或
Arrow 字段的 Physical 选择整数/浮点表示，除法使用浮点表示，有损提升会被拒绝。
幂和任意底数对数复用算术的形状检查、标量广播及按位置准备输入，统一输出 Float64；实数定义域与非有限结果检查由 `NumericOperation::evaluate_float` 共享给内存和 DataFusion 批执行。
自然对数、以 2/10 为底的对数、平方和平方根通过同一 Numeric 内核执行；操作数数量由 `accepts_arity` 校验，单输入实数计算由 `evaluate_unary_float` 在内存和批执行间共享，不构造重复输入或模拟底数。
六个比较节点统一支持标量与等长数列，以及任一侧的标量广播；空值传播。不同来源的数据库数列及数据库与内存数列混合时，先受控读取，再按当前位置比较。整数/浮点混合比较复用 Tabular Contract 的精确比较，不经过有损浮点提升或 epsilon。文本按原值比较，排序采用大小写敏感的字典顺序。
六个比较节点对数列逐元素比较，支持精确模式与数值容差模式。可共用行域的惰性输入通过关系契约生成 DataFusion 表达式；其余组合先物化再逐项计算。Kernel 可以持有 Arrow 数组、字段和批数据，不持有 DataFusion 查询上下文。
AND/OR/NOT 同样支持标量和等长数列，允许独立来源及内存混合输入；内存计算复用 `BooleanOperation` 的三值逻辑，惰性计算通过关系契约生成原生布尔表达式。`false AND null` 为 false，`true OR null` 为 true，`NOT null` 为 null。内存数列检查长度、输出预算及取消状态；旧的纯标量布尔执行入口已移除。
物化 Binary 注解先复用转换适配器与 Arrow 的语义转换，按保留的正值映射归一化，再执行布尔运算；
因此类型转换的标量/列表结果可以继续连接 AND/OR/NOT，反向正值和空值不会因剥离注解而改变含义。
Graph 的广播说明不提前改写标量值。数值、转换、比较和关系筛选适配的行为变化会推进实现 revision。

类型转换支持七种 Semantic，保留标量/数列结构。`builtins::conversion` 仅通过
`yss-database-arrow::convert_semantic_values` 的精确能力调用处理已物化值，输入输出使用中立
`TabularScalar` 和 `ConversionMetadata`。表格物化复用 `materialized_column`，再直接组装 Arrow 批交给 `RelationFactory`，不再用 `TabularSnapshot` 作为计算中转。Arrow 适配器复用 `PreparedConversion` 的转换与校验。
已物化的分类、顺序、日期时间及标识输出通过 `RuntimeValue::with_metadata` 保留含义、值域和已确定的时间表示。标注载荷只允许标量或平坦标量列表，字段私有，不能嵌套标注或包装资源句柄；后续转换继承元数据，展示端剥离标注投影原值。惰性数列通过 `RelationHandle::convert_series`
生成表达式，持有共享的预编译转换对象；每个批次继续执行值域、精度和范围检查。数值表示策略、Null 和失败规则见内置节点帮助。
七个 To 节点的目标语义由各自 Kernel 注册项固定；内核只读取该目标相关的配置，共用同一转换入口。
To Categorical 和分类 Data Labels 在空配置且没有源值域时，复用 Arrow 的 `InferredCategoricalDomain` 收集完整非空取值，生成原值同名标签。标量/列表、裸分类字面量和分批读取的数列共用此收集器及 `ConversionDomain` 限额；内核控制数列扫描的取消、超时和内存预算，随后沿用原惰性数列转换，保留 Physical 与行对齐。显式或继承值域仍是完整约束；空输入允许空分类值域，Ordinal 不自动推断等级。

Execution 的 [kernel_invocation.rs](../yss-graph-execution/src/kernel_invocation.rs) 负责从计划生成这些信息。它在准备好的资源绑定中解析资源参数，保留图端口地址、Schema 血缘和结果类别，并将返回值映射到对应输出。内核不接收 `GraphDocument`、`PlanOutputRef`、项目状态或资源授权服务。

`KernelError` 表达维度、参数、行对齐、预算、调用契约、科学计算、取消和超时等稳定原因，不携带图地址。Execution 的 `OperationExecutionError` 补充图来源与阶段；Application 的类型化 `RunApplicationEventKind::RunErrored` 携带对应 `RunFailure`，经图活动和调用方 sink 交付原生宿主。ResultStore、结果引用、租约、保存和运行生命周期仍属于其原所有者。
纵向、生存、多元分析、推断、ANOVA、空间、线性回归、回归模型、时序预测、因果模型、相关一致性、不平等统计与可视化适配共用 `statistics::common::computation_error`，保持 SCI 的取消、超时、形状、参数范围与计算失败分类，不各自维护同一映射。SCI Contract 的 `DataOutOfRange` 保持为数值输入错误；非法置信度、覆盖率与推断选项保持为参数错误。能力指纹涵盖修正后的错误分类；ICC 另外复用稳定的共享 F 尾概率。

`statistics/survival` 适配 15 个生存分析节点，复用统计输入的联合物化、精确分类标签、
预算和取消协议。普通事件列支持布尔或 0/1；竞争风险原因保留整数代码。
观测时间、事件/治疗编码、计数过程区间、分组可辨识性和预测概率保持为数值输入错误；
数组或模型布局不符保持为形状错误。预测期限、分箱/刻度数及迭代设置仍为参数错误。
参数生存、Cox/计数过程及亚组使用 revision 9；分组曲线与 Log-rank 使用 revision 8；竞争风险、校准及
决策曲线使用 revision 7，Cox PH 诊断使用 revision 6，列线图使用 revision 5。
生存工作区分别计算设计宽度、矩阵维度和线性组元数据；受试者数和曲线组数不增加
模型列数。Log-rank 保留组协方差预算，亚组治疗效应保留设计矩阵预算；竞争风险输出
按行数与原因数计费。SCI 共用一次建立的精确分组行索引，
按组消费行，新增行索引落在现有生存节点工作区预算内。
`survival/input` 负责按实际输入角色完成联合物化、编码及预算准入，返回拥有数值数组和
标签的准备结果。原始标量列在 SCI 调用前释放；没有分组角色的节点不分配默认分组数组。
事件、起始时间、治疗和预测风险各读取一次，计算及输出组装只消费中立准备结果。
静态 Cox 和 AFT 将原行序的时间、事件和风险一起物化为 `predictions` 关系，供下游
评估节点选列；不把无血缘的内存预测向量与数据表按相同行数强行对齐。
列线图只解码静态 Cox 类型，并由 SCI 计算刻度；Graph 负责图形类别。

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

`statistics::panel::models` 绑定七个 `econometrics.panel.*` 入口。数值输入复用共同物化和配对长度检查，
FE/RE/FD/Between 直接取得共享 `PanelFit`，恢复列名并输出可连接现有 Summary 的模型。
动态面板及两项 Fisher 检验消费中立 `PanelData` 和执行控制；适配器按输入、设计/矩条件工作区和
结构化结果合计检查预算，再调用 Runtime。它不构造 GMM 矩阵或在图编辑时读取数据。
动态拟合保留真实列名及原始行索引；检验保留原数值实体编号，有限性验证先于 JSON 编码。

`statistics::multivariate` 注册多元分析的七个目录 ID；数值列按位置读取并检查长度，分类训练标签用 `TabularScalar::compare` 精确编码。
判别的新预测组独立验证其自身对齐，复用训练尺度并恢复原始标签/语义。输入、密集工作区及得分的运行值/Arrow 转换一起准入，再调用 Runtime。
摘要通过已有 typed finite-value 转换；得分/主坐标按 Invocation 中 Graph 已解析的字段，通过 `relational::materialize` 进入共享查询引擎，保持原行顺序但建立独立计算行域。
CCA 的 X/Y 轴组合成一张得分表以支持相互比较。观测数据不复制进内联摘要，坐标表和预测数列沿用现有分页。内核不计算输出 Schema 或图血缘。

`statistics::anova` 注册七个方差分析入口，通过已有 `series::load` 联合物化关系数列，或读取等长内存列。
响应与连续协变量要求无损数值转换；因素及受试者以 `TabularScalar::compare` 精确编码，宽整数标签不转浮点，结果恢复原因素标签。
因素数、重复组布局、平方和/交互选项与 Catalog 同步；输入、密集工作区和报告一起检查预算，再调用 SCI Runtime，并保留取消/deadline 错误。
接受等长且按当前位置配对的数据库与内存数列，允许混合两者，不删除缺失观测；唯一输出为结构化 `statistics.report`。

泰尔指数适配位于 `statistics::descriptive`：个体形式只接收 `series`，分组形式必须另接一个 `weights` 数列（组人数或人口占比）。输入通过共用 `columns` 执行受控物化和行对齐检查，再调用 SCI Runtime；唯一输出为包含指数及描述字段的结构化 result。分组输入为组均值，仅计算组间差异。切换形式后，不适用或缺失的 weights 明确报错。

同一适配模块注册 Gini 和 Dagum Gini。Gini 复用数值列读取；Dagum 复用 `series::load` 按位置读取数值与标签、核对配对长度，内存列复用 `series::columns`。标签使用 `TabularScalar::compare` 精确分组，再以中立组编号调用 SCI，输出时恢复原始标签，宽整数不转为浮点。输入、排序缓冲和组对输出在调用前共同检查预算，不以固定组数截断。仅在执行时读取数据，算法与中断控制继续由 SCI 拥有。

统计结果进入 `common::value` 时保留原有数值类型；转换入口在 JSON 编码前遍历并拒绝
非有限浮点数，返回 `NonFiniteResult`，合法 `Option::None` 仍转换为空值。
普通 JSON 的标量、列表和记录统一由 `RuntimeValue::try_from` 转为运行值，保留有符号整数、
无符号宽整数及浮点载体。内核报告复用同一转换，并在每个嵌套值检查执行控制；
编码前有限性校验和整体内存限额继续由统计适配入口执行。
ACF/PACF 与 Hausman 使用 typed report，避免先经 `json!` 把数值错误抹成 Null。
已经构造的 JSON 无法恢复被抹去的数值类型，生成这类报告的 owner 必须在编码前完成检查。

相关与一致性适配位于 `statistics::association`。数值列复用 `columns`，Ordinal/分类列复用 `series::columns` 和语义元数据；配对列按位置读取并验证长度，Ridit 的独立样本分别读取。加权 Kappa 要求明确类别顺序，宽整数类别通过精确比较编码并在输出时恢复；不以 Float64 合并标签。Kappa 和 rwg 的条件参数在声明、内核可选性与执行校验中保持一致，inactive 参数不进入调用。排序、残差化、置换、方差与推断属于 SCI；调用前按实际规模检查工作区（包括 Cohen Kappa 的类别平方表和判别分析的类别协方差），执行中转发取消/deadline。
共享分类编码使用精确标量排序索引，避免逐行线性扫描全部类别，保留首次出现与显式有序元数据顺序。

## 注册与扩展

`builtins::visualization` registers all 19 plot adapters. Paired/matrix inputs use
equal-length, positional database or memory columns (including mixed inputs); box/violin groups are
independent samples. Missing values are rejected. ROC respects declared positive
Binary codes, including inverted Boolean meanings. Coefficient plots consume
native OLS/WLS/GLS models. Adapters share controlled materialization, budgets and
finite-result encoding, then call `yss-sci-runtime::visualization`.
Each result is one `plot.data` record; rendering does not run inside a kernel.
Boxplot and violin use revision 6 for stable shared quantile midpoints; Delphi
uses revision 5, and both mediation bootstrap kernels use revision 8.

`KernelRegistryBuilder::register` 接收 KernelId、非零实现 revision、KernelContract 和执行函数。KernelContract 声明有序输入键及数量范围、实际参数键集合和输出数量范围；它不复制 Catalog 的分类、本地化文本或完整配置模型。

节点参数按参数 key 扁平传入，内核通过 `invocation.parameter(key)` 读取。参数组只属于声明与编辑呈现，
不会形成运行期 Record。`with_optional_parameters` 将已声明的条件参数标为可选；组装时同时核对参数键和
可选性，运行时拒绝未知参数及缺少必需参数。Graph 在准备计划时解析默认值并排除不适用的条件参数；
内核按所选算法检查适用字段。可选参数集合进入能力指纹，分布、比较、整数范围和线性 Fit 的实现 revision 已随调用契约更新。

`with_builtins` 组合已有内置实现；Application 可以在冻结前加入扩展。冻结后所有调用使用同一能力指纹。指纹继续采用 `yssbi.kernel-registry.v1` 编码，覆盖排序后的 ID、revision、输入布局、参数键和输出数量。具体已安装能力以 [builtins](src/builtins/mod.rs) 的注册表为准；未安装的节点仍返回缺少执行能力的诊断。

新增节点时在对应方法族模块实现适配，再加入内置装配或由应用 provider 注册；实际算法放回 SCI 或相应数据所有者。执行函数只使用已经解析的类型和资源，不重新求解图类型，也不自行读取项目资源。

实现行为变化需要递增 revision。输入布局、参数或输出形状变化需要同步节点声明和消费者，并复核解析与计划缓存的能力身份。透明重路由不产生执行操作，也不注册无效的同名内核。

OLS、WLS、GLS 和 Prais 未定义推断现在返回科学错误；线性 Fit 及实际复用该路径的面板、分阶段回归、曲线/RCS、路径分析、响应面与 ECM 内核同步推进 revision，能力指纹涵盖这项错误行为变化。统一线性 Fit（OLS/WLS/GLS）使用 revision 14，保留 WLS 权重和 GLS 协方差数据的形状、非有限及定义域错误分类，以及 SCI 的稳定 F/Student-t 尾概率；Prais Fit 使用 revision 9。Summary/Predict 不重新拟合。
DID 随机化的 nonrobust 拟合也保留 OLS 未定义推断错误，使用 revision 7。TWFE DID 保持默认 TwoWay/cluster 拟合，直接接收 typed `PanelFit`，在组装 JSON 报告前复用有限值校验；非有限模型返回 `NonFiniteResult`，稳定 F/Student-t 尾概率的实现使用 revision 9。
IV 2SLS/LIML Fit 直接接收共享 `InstrumentalVariableFit`，恢复响应、自变量与工具变量标签后交给既有输出转换；2SLS Fit 使用 revision 11，LIML Fit 使用 revision 13。SCI 的两种估计器共用 `IvModel` 与同一份工具变量/投影设计；LIML 的稳健协方差保留 κ 类交叉乘积逆矩阵和结构残差，并使用工具变量投影后的得分自变量，HC2/HC3 的杠杆值也采用该投影。标签仍由当前适配器和 Runtime 恢复，不进入数值输入。系数概率及 95% 区间默认采用正态参考分布，`small=true` 采用结构模型剩余自由度的 Student-t；整体检验在当前 `statistics.modelTest` 中明确记录 χ² 或 F、统计量、自由度及 p 值，F 为 Wald 统计量除以非截距系数数目。负或非有限系数方差、未定义或非有限统计量及区间在 SCI 返回计算失败，适配器映射为 `ScientificFailure`，不再使用零统计量掩盖错误；其他非有限模型仍在 JSON 编码前返回 `NonFiniteResult`。Fit 和 Summary 复用现有准入入口，按实际的线性观测工作区计算预算。Summary 从已存运行值解码当前模型并按所选内容计算报告，系数与可选约束检验使用一致的参考分布。独立 Hausman 读取同一模型契约，使用 revision 7。
IV 2SLS Summary 使用 revision 18，LIML Summary 使用 revision 16，保留 SCI 第一阶段、内生性、过度识别及 Wald 检验的稳定 F 尾概率。第一阶段要求正的剩余自由度及可定义的推断，饱和工具变量回归或零残差方差返回 `ScientificFailure`；未选择该分析时，结构模型仍可汇总。方程使用当前 `inference` 记录保存所选协方差和系数推断，另有实际 `df_residual`；系数表和排除工具变量的 F 检验复用同一协方差，分别采用第一阶段 OLS 剩余自由度的 t 及 F 参考分布，不受结构模型 `small` 控制。无常数模型采用未中心化 R²，正的微小方差保留原量纲。系数、拟合值及工具变量分解复用同一次设计准备，残差计算不构造观测数平方大小的投影矩阵。多内生变量的最小特征值通过 Cholesky 白化后交给对称特征值算法，保留列顺序和单位变换下的结果一致性。多内生变量矩阵保持观测行与变量列的对应关系，Shea 指标与报告展示继续使用共享类型字段，不从已编码的 JSON 重读系数。
结构模型与第一阶段的 R²/调整 R² 复用 SCI 同一个借用输入计算入口，以共同尺度计算残差与总变异的比值，微小响应量纲不再被固定方差下限改成零。截距模型中心化，无截距模型不中心化；采用实际剩余自由度，保留合法的负 IV R²。零或未定义总变异及非有限结果映射为 `ScientificFailure`，Summary 直接保留拟合模型指标。

2SLS 过度识别直接借用当前模型的结构残差，以共同尺度进行辅助回归；正的微小量纲保持可检验。Sargan 保留辅助拟合平方和与结构残差平方和的比值；Basmann 直接使用辅助残差平方和，避免 `n-Sargan` 相消。辅助残差变异或工具变量回归剩余自由度为零时，仅 Basmann 的统计量和概率为 null，Sargan 仍可报告；结构残差全零时保留整项不可用原因。SCI 复用同一工具变量准备、内生变量投影和自变量组装入口；nonrobust 只准备工具变量，Wooldridge 按需投影并直接求解得分交叉乘积，均不重建响应、系数或观测结构自变量。Summary 继续保留矩阵预算及执行控制准入。稳健分支借用 Linalg 分解参数维度的 `W'Z`，将其 `m` 个右零空间方向与已准备的工具变量相乘，覆盖完整约束空间；列顺序不再选到无效方向，也不需要高矩阵 SVD 或观测数平方的分解因子。SCI 现有协方差 owner 为 HC0–HC3 使用独立行得分协方差，为 cluster 使用组内得分和，为 HAC/newey 使用模型保存的核、带宽或滞后数；渐近得分检验不应用系数的有限样本或杠杆调整，参考 χ²(m)。原回归协方差复用同一原始 lag/cluster 计算，保留原 sandwich 和修正。无效聚类/滞后及奇异得分协方差映射为 `ScientificFailure`。Catalog 的 IV Fit 配置仍只提供 nonrobust 和 HC0–HC3；共享模型 API 的聚类/HAC 配置由 Summary 按实际保留值处理。

LIML Fit 通过正定的已包含自变量残差交叉乘积进行 Cholesky 白化，以最大倒数特征根计算 κ；奇异工具变量残差协方差不再产生接近零的伪根，响应量纲也不再受固定特征值下限影响。残差交叉乘积直接由共享投影后的列构造。

LIML 过度识别检验直接复用拟合后的 κ：Anderson–Rubin 为 `n*(κ-1)`，Basmann F 为 `(κ-1)*(n-k_z)/m`，`m` 为排除工具变量数减内生变量数。SCI 复用原模型准入校验，不重建矩阵和残差；无效 κ 或非有限统计量映射为 `ScientificFailure`。只选择该分析时不再计费拟合矩阵工作区，模型解码及输出的预算、取消和期限检查继续生效。第一阶段、2SLS 过度识别及内生性分析仍执行原矩阵工作区准入。

ADF 的无常数和趋势选项复用 SCI 的共享 MacKinnon 校准，修正原重复实现的多项式系数顺序；`yssbi.statistics.adf.test` 使用 revision 9。SCI 在构建设计矩阵前要求正的剩余自由度，饱和回归和未定义推断返回 `ScientificFailure`，多序列报告保留逐序列失败信息。Drift 和辅助回归复用同一个 Student-t 分布及稳定尾概率，不再将自由度改为 1 或钳制标准误；参考分布约定与临界值保持原契约。复用该回归的面板单位根和协整检验使用 revision 7，并保留其 MacKinnon 校准。
Panel Fit 使用 revision 11，Compare 使用 revision 9：双向随机效应 MLE 的似然计算按保留列映射读取紧凑系数，避免删除中间共线列后使用原列号索引系数。Fit 保留 SCI 的稳定整体 F 和系数 Student-t 尾概率；Summary/Predict 沿用已拟合模型。
系数约束的负或 NaN 对比方差在原 SCI 校验边界返回计算失败，不再把开方后的 NaN 交给参考分布。线性、Logit/Probit/Prais、IV Summary 和实际复用 Summary 检验的 diagnostic.wald 同步更新实现 revision。普通样本均值 t 检验使用另一算法入口。
稳定 F 尾概率由 SCI 分布模块统一计算。线性 Summary 使用 revision 14，Prais Summary 使用
revision 9，独立 Wald 使用 revision 7；RESET、嵌套模型比较、测量系统、ANOVA 与线性回归
效能规划使用 revision 5；七个 ANOVA 入口使用 revision 5；ICC 使用 revision 4；
独立 FE/FD/Between 面板入口使用 revision 9，RE 使用 revision 10。参数、控制和报告形状保持各适配器的原契约。
共享 Student-t 尾概率也用于样本均值/等效检验、Pearson/Partial/Spearman、多重比较、路径效果、
响应面/剂量反应、ECM、OLS/SLX 空间回归、调查回归以及均值/配对/整群效能规划。
注册实现拥有各入口的能力版本；返回这些推断结果的内核随计算行为更新指纹。
正态双侧尾概率复用 SCI 的单次 `erfc` 计算，卡方上尾直接复用 SCI 中已校验分布的 SF。
Logit/Probit Fit 使用 revision 7，Summary 使用 revision 9；VAR/VEC Fit、VAR 阶数选择和 VEC
Summary 使用 revision 7，VAR Summary 使用 revision 9。残差正态性、Ljung–Box 与
Breusch–Godfrey 的报告保留小概率；返回这些结果的内核按注册表更新能力身份。

## 验证

独立调用测试检查输入顺序、输出载体和资源控制；Application 集成测试检查内存表的组合、拆列顺序、关系计算与分页。扩展注册、配置、数值执行和结果查询测试继续覆盖跨边界行为。运行命令见 [Rust workspace README](../../README.md)，范围选择见[根规则](../../.rules)。

## 内置节点语义与转换

逻辑比较统一为六个节点，标量返回 Binary，任一输入为 DataSeries 时逐元素输出 Binary 数列，支持任一侧标量广播。`NodeTypingSpec::BinaryPredicate` 要求输入元素语义一致并推导二元输出形状，供比较及 AND/OR 复用。等于/不等于支持七种基础语义的值比较，排序比较支持 Numeric 和 Text，不根据分类编码隐式推断等级。
数列必须等长。独立来源或混合输入复用 `series::prepare` 读取数据库数列并保留语义元数据，再复用内存比较路径；按当前行序逐项比较，不推断键或行域对应关系，长度不同时返回 `ShapeMismatch`。已有内存输入、物化缓冲及输出共同纳入预算，读取和比较均检查取消与 deadline。任一元素为空时输出空值。Tabular Contract 拥有精确标量比较，Kernel 处理内存值，DataFusion 对相同物理类型使用 Arrow 比较，对混合整数/浮点复用精确标量比较，避免优化器的隐式浮点提升；不支持的表示失败。比较实现 revision 参与已有能力指纹及执行缓存。
原数据序列数值/字符串比较节点已移除。
AND/OR/NOT 接受 Binary 标量或数列，采用 SQL 三值逻辑，内存数列要求等长并支持标量广播。NOT 保持输入类型和形状，使用类型恒等规则但仍是执行叶节点。可共用行域的惰性输入通过关系契约构造 DataFusion 原生 AND/OR/NOT 表达式，在结果消费时执行；独立来源或混合输入复用共享物化入口后执行内存运算；具备正值映射的非标准二元编码先复用语义转换规范化。计算按当前位置配对。这些节点属于数据计算，不构成图的控制流或上游短路执行承诺。

类型转换由 `yss-data-contract::SemanticConversion` 定义目标语义、数值表示、显式值域、时间形式/精度与解析格式，支持全部七种 Semantic。
`ShapePreservingConversion` 按输入形状推导标量或 DataSeries 输出。To Numeric、To Text、To Categorical、To Ordinal、To Binary、To Datetime、To Identifier 分别固定七种目标；不再提供通用目标选择或下游反向推断。数值、值域、日期时间参数仅在相关节点上声明。Data Labels 继续使用独立参数选择分类或顺序。
数值表示的自动模式保留已有 Numeric 物理表示，文本解析为 Float64，Binary 转为 Int64；整数/实数模式显式选择 Int64/Float64。
Null 保持 Null，非有限值、溢出及有损数值转换失败。目标配置、默认值和类型推导均由 Rust 拥有。
Kernel 的标量和内存数列通过精确授权的 Arrow 适配入口共用批次转换规则，中立值直接构建 Arrow 数组；惰性数列通过关系契约生成 DataFusion UDF。
`PreparedConversion` 在构造时解析字段、确定转换操作，并预编译值域及数值边界校验；每个批次只转换和验证实际值。其校验实现与数据集导入/编辑共用。UDF 持有不可变的共享转换对象和输出字段，执行时不重复解析字段元数据或重建值域。
UDF 的相等性和哈希包含源/输出字段元数据与实际转换操作，不包含未生效的配置；输出字段携带 Semantic 和 nullability，转换保持来源关系与行对齐。
分类值域可继承已声明的分类/顺序/二元值域；新建顺序必须显式配置等级或继承已有顺序。类别编码、标签及等级顺序不按批次重建。Identifier 保留原始值，不附加唯一性要求。Datetime 复用无时区日历转换，支持显式日期/时间形式、精度及格式，拒绝无提示的精度损失与数值时间戳猜测。
`RuntimeValue::with_metadata` 为已物化的分类、顺序、日期时间和标识值构造 `Annotated`；私有载荷只允许标量或平坦标量列表，不能嵌套标注或包装资源句柄。`ConversionMetadata` 的时间表示使用已确定的 `TemporalType`，不携带配置中的 Auto。执行缓存保留该标注，转换会重新使用它。结果展示/分页只投影原始值，通用值比较比较底层值；元数据不作为可变前端副本或新的图状态 authority。
构建表达式不读取整列或修改数据集；错误在实际消费时返回，结果预览失败不改写已完成的 Run。完整列校验不能从局部预览或 LIMIT 的成功推断。

## 关系运算与统计适配

频数、数据描述节点 `yssbi.statistics.describe` 和分组聚合由 `builtins::aggregation` 注册。数据描述从同一个 `source` 输入接收数据帧或数据序列；节点无参数，数据帧自动统计全部受支持的列，数据序列直接统计自身。内存数列复用既有 Arrow 物化入口，再与关系数列共用 `RelationHandle::frequency/describe/aggregate`；数据序列常量保留已声明的分类含义，Binary 元数据也保留，避免按整数编码误判 Numeric。关系计算由 DataFusion 原生聚合、排序、连接和分位数计划承担。频数和分组聚合保留关系输出，扫描在消费时执行；数据描述使用 revision 5，由 `builtins::aggregation::description` 在节点执行中受控读取内部摘要批次并生成 `{ columns: { 列名: { position, semantic, ...统计指标 } } }` Record。列名保持原值，`position` 从 1 开始记录受支持列的输入顺序；各列只包含对应语义的指标，不适用字段不添加。分类、顺序及二元列复用 `frequency(column, false)` 获取全部非空类别的原值、频数及占比，按原字段的语义编码匹配标签；`categories` 以从 1 开始的编号组织明细 Record，保持 Ordinal 的声明顺序。列摘要与类别明细直接随 JSON 返回，最终结果不保留关系句柄或数组引用。统计失败随节点执行交付，读取和构造结果都检查取消、deadline 及内存预算；数值输入无损提升，非有限结果通过类型化错误传播。完整口径见 [Catalog](../yss-node-catalog/README.md)。

聚合列名复用 Data Contract 的 `TabularColumnName` 校验，保留含首尾空格的原始名称，不做 trim；
空白名称和重复选择仍被拒绝。分组聚合内核使用 revision 2。

仅在类型契约要求时提升为 Float64；数列的元素提升和标量广播由计算 kernel 处理，调度器不制造数列长度。
文档数列常量一次性导入 Arrow 字面量表达式，和范围生成节点共用显式位置坐标；等长常量数列可逐元素运算，不同长度返回 ShapeMismatch。带关系身份的数列保留固定行域和文件租约，
可通过 DataFusion 原生表达式及 Arrow 批运算执行；独立来源或混合逐项运算通过共享读取入口受控物化。
原始列和计算数列都由 `SeriesHandle` 持有 adapter-owned 表达式，表达式不进入持久化 Graph 文档。
数据帧组合使用原生 Union、Join、Projection 及用于稳定顺序的 Window/Sort 计划。按行拼接保留重复行、输入顺序及各来源行序；列名模式补齐缺失列，位置模式使用首表列名。公共列要求相同物理类型和兼容语义，不进行隐式有损提升。连接支持多列键和 inner/left/right/full/semi/anti，空键不匹配。半连接和反连接只返回左列及稳定的左行顺序；其他连接保留全部匹配组合和两侧键，右侧重名列使用后缀消歧。
`RelationHandle::bindings` 保留全部来源绑定，组合执行检查项目会话、执行环境及同一数据集的快照一致性。来源租约由组合结果和输出流共同持有；资源准备入口仍要求每个数据源授权值只有一个匹配绑定。多来源结果不伪装成单一数据集，自动化只读检查按预算列出其来源。
DataFrame 字面量和内存列组合在 Kernel 内通过 Arrow 适配器物化列及语义元数据，以 `RecordBatch` 交给注入的 `RelationFactory`，一次性导入同一 DataFusion runtime，之后复用上述关系运算及分页。`TabularSnapshot` 只用于文档和展示边界，不作为内核物化的中转。字面量关系的来源绑定为空；空绑定不授权任何外部数据集。普通结构化值仍使用 Record。
DataFusion adapter 单独持有行域及域内列表达式。筛选列、重命名和数列投影保留行域证明；筛选行、截取、行拼接和连接建立新行域。数据帧按列拼接保留行域约束。数列组装则按当前位置组合等长列，支持独立数据库来源及数据库与内存数列混合，不按键匹配行。
源行顺序的内部字段仅留在 adapter 的域计划中；组合时由显式排序的行号窗口生成内部位置，确保分页稳定，不复用源表的可编辑行标识。输出只暴露用户列，并保留语义元数据；组合输出的字段身份由图的派生 Schema 负责。动态输入顺序进入 Schema 缓存指纹，连接键编辑器和校验分别使用对应一侧的 Schema。
同一关系的数列可以连续运算并联合投影，名称重复的计算列按投影位置区分；独立来源或混合输入经共享物化入口按位置计算。Results 分页调用数列表达式投影，不按其显示名回查基表字段。
标量除零在节点求值时拒绝；数据内的 NULL、除零、非有限值或溢出在批流实际消费时返回类型化错误。
幂和任意底数对数复用同一算术契约及形状推导，两个操作数都是可连线的数值输入，输出为 Float64。`NumericOperation::evaluate_float` 统一内存和批执行的实数计算、定义域及非有限结果规则；整数输入仍须无损提升。独立指数函数节点由幂节点设置底数 e 替代。
自然对数、以 2/10 为底的对数、平方及平方根采用单输入 Numeric 操作，复用相同执行管线并保持输入形状；统一输出 Float64。`accepts_arity` 约束操作数数量，`evaluate_unary_float` 使用对应的实数函数并统一定义域/非有限值检查。数列按批惰性计算，空值和非法定义域均报错，不隐式填充 null。
分页、统计输入消费沿用各自的取消、deadline 和内存边界，不将非法计算值写成正常结果。

线性回归 Fit 从节点参数构造 `OlsOptions` 和 `LinearRegressionMethod`，由 Node Kernel 调用 `yss_sci_runtime::regression::linear::linear_regression`，传入本次执行的取消标记和 deadline。OLS/WLS 支持截距、Nonrobust、HC0–HC3、HAC、Newey-West、Fixed Scale 和 Cluster；Cluster 的单个 `clusters` 输入与响应、自变量及权重共同对齐。GLS 接收相对误差协方差矩阵并估计尺度，标准误仅支持 Nonrobust。Summary 读取上游原生模型，不调用拟合；Predict 复用训练系数和截距。
Cluster 标签独立读取为精确标量，复用 `common/inputs::categories` 编码，不提升为 f64；宽整数和文本标识保持分组身份。
响应、自变量和权重继续使用紧凑数值读取，标签读取计入这些已驻留缓冲，编码前核对长度和合计预算。Fit 使用 revision 11。

Dagum Gini 复用统计适配的受控物化、数值读取与精确标签编码，标签按首次出现编号，不再逐行线性扫描已有分组。原标签用于分组与组对报告；组对输出预算仍按分组数平方检查，输入缓冲在调用 SCI 前释放。
ANOVA 的响应和协变量也通过共享数值读取入口校验；重复测量的受试者可比较性与排序编码由 ANOVA 自身约束。
统计适配器将已物化列转换为数值数组和标签编码后，按值消费原始列或在最后一次使用后释放它们；
ANOVA、推断、问卷、质量分析、DOE、路径、抽样、联合分析、元分析、纵向/混合及生存模型计算不将这些临时列保留到 SCI 计算和报告构造阶段。
已编码标签、输入名、数值数组及原始预算准入由各自既有 owner 持有。
元分析和问卷的纯数值输入直接复用 `series::numeric` 读取为紧凑 f64 数组，不构建原始标量列。
数据库字段沿用数值语义校验，不把分类编码当作连续测量；共享行域共同投影，其他来源按位置配对。
输入预算按这些数值缓冲计算，工作区与报告准入继续合并检查；元分析、Forest/Funnel 和问卷内核使用 revision 5。
Association 的全局类别合并和逐行编码复用 `common/inputs::Category` 精确比较，以索引替代标签线性扫描；
未声明的类别保留首次出现顺序，排序型类别沿用可比较性要求，已声明的 Ordinal 等级仍按原代码索引。

Logit/Probit/Prais、IV 2SLS/LIML、Panel、TWFE DID、ADF、VAR/VEC 及阶数/协整秩检验均有执行适配。模型以不可变 Record 保存中立拟合契约，Summary 在预算及执行控制检查后解码为对应契约，交给 Runtime 组装所选报告并调用需要的 SCI 分析；Fit 不附带完整报告。Logit/Probit Predict 使用既有系数。Panel/VAR/VEC 只输出模型，DID 输出模型和报告，不把未提供或多方程的观测结果伪装为单个拟合数列。参数组合和输出含义见 [Catalog](../yss-node-catalog/README.md) 与各节点帮助。

独立诊断复用上游线性模型的观测、设计列、协方差及 WLS 权重；VIF、杠杆值、BP/White/IM/RESET、BG、系数 t/Wald，以及序列正态性、DW、Ljung–Box、ACF/PACF 通过对应领域适配。VAR 的 Granger/IRF/FEVD 与非稳健 2SLS 的 Hausman 在对应节点执行时计算，IRF/FEVD 的 steps 控制分析范围，不重新拟合上游模型。IV/VAR/VEC Summary 只计算所选诊断。DID 伪处理组随机化显式接收 treat/post 与随机种子，在置换之间检查取消。KDE 经 visualization 适配输出绘图数据，默认 256 个网格点，可在结果面板和 Plot 窗口查看。

White/IM 沿用 Catalog 的模型要求：原拟合必须含截距。SCI 使用模型已有的 `constant` 事实准入，无截距的 OLS/WLS 在独立节点返回 `ScientificFailure`，在线性 Summary 中保留对应项目的不可用原因；诊断不补建截距或把首个预测列当作截距。

IV Summary 保留 SCI 明确返回的不可用诊断，不用 JSON 将 NaN 转为空值来表达缺失。第一阶段无定义的调整 R²、零秩 Hausman 或自由度不足的内生性检验使用既有可选结果，其他可用分量与原模型仍保留。独立 Hausman 节点在检验不可用时返回 `ScientificFailure`。

数据帧对齐接收表和列名参数，通过关系契约构造原生窗口、聚合、range/unnest 和连接计划，不收集输入批次、不调用 SCI 对齐。时间序列使用原始时间单位的等距网格；面板使用共享已观测时间序列的位置，在每个实体自己的起止位置内补齐。原列类型、列序和元数据保留，补入的非键值为 Null。重复键、缺失键、网格与输出预算校验保留在计划表达式中，在消费时交付失败。

`LinearRegressionValue` 共享不可变拟合模型；Summary 另持有本次 `LinearSummaryOptions` 和选中检验的不可变结果。ACF/PACF、序列相关和假设检验在 Summary 执行时按选项计算，未选项不调用 SCI。模型拥有有界 memo，每类分析只缓存最近一组参数；补选复用相同模型和参数的结果，参数变化重新计算，新 Fit 使用独立缓存。计算不持有 memo 锁，遵守调用预算并在分析间检查取消；旧 Summary 继续持有原分析快照。

Breusch–Pagan / Koenker 与 VIF 等诊断共用上述执行控制，BP 不再单独绕过预算入口，使用 revision 7。
Logit/Probit 的边际效应和 VAR 稳定性计算已移除固定计算量门槛，继续使用原 SCI 算法及取消/期限检查；
三个 Summary 内核使用 revision 8。统计输入和数值合法性检查保持原契约。

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

`yss-node-kernel` 的统计适配直接调用 `yss-sci-runtime`；`yss-graph-execution` 仅在独立 OLS benchmark 中使用该开发依赖。Application 通过图执行与已组装的 Kernel registry 编排统计节点，项目与结果身份仍归各自 owner。取消与 deadline 保留同步计算前后的检查，不承诺中断正在进行的矩阵分解。通用数学语法由 `yss-math-expr` 拥有。

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
标准化通过一次原生 avg/stddev 聚合取得两个标量，并将同一均值和样本标准差交给变换及标量输出，
惰性结果表达式只执行 `(x - mean) / standard_deviation`，保留空值和原行域。该内核使用 revision 6。
标准化和逆标准化输出仍可与原列比较或组合。数列常量以显式位置坐标导入，已有等长常量数列的逐元素运算继续成立；该坐标不授权数据帧或数据集间按长度拼接。两步往返不保证浮点逐位相同。

Database Engine 仅作为标准化后端回归的测试依赖，用真实关系验证统计输出、空值和往返计算；不进入生产依赖。
六个逻辑比较节点默认精确比较，可在配置中切换到数值容差模式，并显示绝对/相对容差字段；默认 atol=1e-12、rtol=1e-9，
标量、物化数列与关系表达式共享 `NumericTolerance` 判定。Null 传播，容差必须有限且非负，
到 Float64 的有损转换被拒绝。容差模式采用对称公式 `abs(a-b) <= atol + rtol * max(abs(a), abs(b))`，先将容差内的两值视为相等，再应用比较运算符，容差外保留数值顺序。运算符及容差参数参与表达式身份，不能被优化器误合并。布尔运算不需要数值容差。容差关系不具备传递性，不用于排序或分组。

面板差分显式选择上下文数据帧的实体列和时间列，与待计算数列按当前位置配对，独立来源通过共享上下文附加入口物化；
拒绝缺失及重复键，组内排序计算后恢复原始行位置，不推断日历间隔或删除缺失行。
参照组提示由 `ConversionMetadata::dummy_base_level` 持有，组装 Arrow 列时写入
`yssbi.dummy_base_level` 字段元数据；不会改变分类值，也不会隐式启用回归的虚拟变量展开。
Decompose 的每个动态输出在 semantic snapshot 中携带单列 Schema（当前列名、类型和 lineage），
计划准备将其保留到输出契约。执行按该列名返回共享上游关系的 `SeriesHandle`，不按端口顺序或显示标签猜列，
也不为每列提前扫描数据；下游统计消费或 Results 分页时才交由 DataFusion 投影和读取。
文档内的 DataFrame 与 DataSeries 常量在常量入口导入，后续选列和变换均传递惰性句柄。
关系和数列持有固定 session、dataset snapshot/revision、查询上下文及文件租约。资源准备检查句柄内的
session/revision 与已授权资源一致。OLS 可以混合既有数值列表与独立来源的原始/计算数列，按当前位置核对观测数；共享行域时共同投影，否则独立受控读取，并使用累计输入内存预算准备数值矩阵。NULL/非有限值仍被拒绝，不隐式逐列删除缺失样本。
配对计算只使用当前行位置；按键匹配须显式执行 Join 等操作。统计方法与可用选项以各领域适配器、节点参数和实际冻结注册表为准。

Parquet 关系数据源要求精确 Schema 显式标记独立的 RowId 与 DisplayOrder 列；读取按 DisplayOrder、RowId
确定顺序，再向 Graph 投影用户列。统计输入与拟合值/残差不会以并行批次的到达顺序替代表格的显示顺序。
文档内的物化常量使用其不可变行域内的顺序，不将文件/batch offset 当成项目数据集的长期身份。

真实项目的 DataFrame 资源现由 DatabaseRuntime 捕获 catalog 快照，并以 Project grant 的版本封装关系句柄；
固定 Parquet 链路与真实项目的 CSV 导入 → Graph Execute → OLS → Results 分页均已通过集成验证。
最终提交使用准备时捕获的同一组资源授权，Project 在发布前再次检查版本；不以空授权跳过数据集依赖。
关系候选 Results 持有懒句柄，不保存所有中间批次；计划准备不表示全部行已经成功扫描，分页扫描失败通过结果读取错误交付。
数据存储与查询边界见 [Dataset store](../yss-database-store/README.md)，局部性能测量见[数据引擎基准](../../docs/benchmark/DATA_ENGINE_BENCHMARK.md)。

The built-in classical hypothesis-test adapters live in `builtins/statistics/classical.rs`. They translate node inputs into neutral `yss-sci-contract::hypothesis` requests, invoke stateless SCI runtime functions, and expose one structured `result` output. The catalog owns localized node definitions and help; kernels do not duplicate formulas.

Classical adapters forward the invocation's cancellation/deadline control into all
four SCI families and use the common computation-error conversion to retain
observation, option, numerical-failure and interruption classifications.
Before SCI dispatch they
admit retained numeric columns, ranks/order/ties and sort scratch, deviations,
per-condition rows and report sizes. Categorical encoding admits retained prior
columns and batch/scalar temporaries before allocation. Dictionary/Utf8View labels
are expanded one row at a time through the existing Arrow converter, so repeated
references do not create an unadmitted full-column string expansion. Count-table preparation
admits linear category indexes before constructing them, then uses actual row and
column cardinalities for the dense table. These are conservative workspace
estimates, not process RSS limits. Classical kernels use revision 6, except the
paired t, McNemar, CMH, categorical independence, Pearson contingency, chi-square goodness-of-fit and
multiple-proportion and Bartlett kernels,
which use revision 7;
Mann–Whitney, Kruskal–Wallis, Friedman, Runs and Mann–Kendall also use revision 7,
covering corrected rank moments and tie handling. The rank-family selector uses
revision 9, including Mood's shared Pearson calculation, data-error contract and
stable shared median. Mood median uses revision 8 for that shared median.
Levene uses revision 8 and Brown-Forsythe revision 9 for the stable shared F tail.
Fisher exact, exact Binomial and Poisson use revision 8. Binomial/Poisson count
admission uses invocation resources and execution control; SCI retains numeric
representation checks and samples the shared control during exact enumeration.
Item discrimination reuses the same Welch computation errors and the stable
shared quantile midpoint, using revision 7.
CMH exposure and outcome reuse the controlled scalar-column and binary numeric
reader, without encoding numeric observations as category strings. Each temporary
column is released after conversion; admission includes retained earlier columns,
the current scalar buffer and its numeric output. Strata keep their exact typed keys.
Independence and Fisher count tables reuse SCI's borrowed ordered label indexes;
their lexical cell ordering and existing workspace admission remain unchanged.
Fisher reports the sample odds ratio; an unbounded ratio is Null while the exact
p-value and table counts remain usable. The common report path preserves this
optional statistic instead of rejecting the entire result as nonfinite.
Goodness-of-fit retains SCI's positive, matching frequency-total requirement and
roundoff tolerance through the existing numeric-input error boundary.
Zero-total Pearson count tables use the same numeric-input error boundary.
Multiple-proportion input preserves ordered success/trial pairs for SCI's shared Pearson test.
Variance-homogeneity adapters preserve SCI's small upper-tail probabilities and
its p-value of one for a zero F statistic.

Paired t and McNemar tests use the shared numeric-column reader to pair equal-length
measurements by their current positions. Database series from independent sources and
materialized lists may be mixed. Shared row domains retain the joint-projection fast
path; independent inputs are read under a cumulative memory budget. Independent-sample
tests read groups separately and allow different sample sizes. Both paired kernels use
revision 7.

`builtins/statistics/meta/` owns study-input alignment, numeric conversion,
workspace admission and output relations. Registrations, effect preparation,
analysis dispatch and columnar output conversion are separate modules. It
passes cancellation/deadline control to SCI and charges numeric workspaces,
study-table conversion and structured reports together, without a fixed row cap.

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

Panel Fit and Compare consume Runtime's typed `PanelFit` directly, restore labels,
and validate typed numbers before JSON output can turn NaN/infinity into null.
Fit uses the existing output conversion; Compare reuses that finite-value validator
before assembling its JSON report. Nonfinite model results return `NonFiniteResult`,
including cases previously rejected as `ScientificFailure` during JSON decoding;
explicit optional `None` fields remain valid. Panel category and model-diagnostic adapters
share `common::computation_error`, preserving cancellation, deadline and typed input failures.
Linear Fit, association and inequality adapters use the same mapping. SCI data-domain violations
remain `InvalidNumericInput`, while invalid confidence/coverage and inference options
map to `InvalidParameter`. Association kernels retain this error contract; ICC
also retains the stable shared F tail.

`builtins/conversion` shares one controlled scalar/list/lazy-series conversion path between
the seven fixed-target To nodes and Data Labels. The label entry accepts only categorical and ordinal target meanings; semantic-domain validation remains in the existing data owner.

`builtins/statistics/path` separates moderation, mediation and recursive-equation adapters.
They align source columns, admit fitting/bootstrap/effect-table storage, and map compact
equation reports separately from full fitted-observation/effect relations. Output variable
indices are one-based. `common/models::regression_outputs` remains shared by moderation
and DOE; `path/reports` is the shared compact equation projection for mediation and paths.

`builtins/statistics/survey` aligns every role before numeric conversion. Weight validation
and inverse-inclusion-probability output preserve the source relation via existing numeric
series expressions. Estimator adapters account for fit, PSU score and covariance storage.
Shared `regression_outputs` receives explicit predictor labels, excluding survey design
columns from coefficient-axis names; complete observations remain paged relations.

`builtins/statistics/power` converts model-specific scalar parameters to neutral designs.
There are no observation inputs; bounded report admission and cooperative scientific
control apply independently of the proposed sample count.

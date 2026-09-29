# yss-node-catalog

> Status: Current
> Scope: 内置节点定义、创建描述、分类、文档与节点目录本地化
> Canonical owners: 本 crate 的源码拥有内置目录；Node 与 Graph 的边界见 [Graph 与 Execution](../yss-application/src/graph/README.md#module-ownership)
> Update when: 节点目录装配、创建描述、本地化接口或依赖边界改变时

Node 由三个 crate 组成：

| Crate               | 职责                                                           |
| ------------------- | -------------------------------------------------------------- |
| `yss-node-protocol` | 节点类型、端口、参数、类型和 Schema 约束、执行语义声明与值校验 |
| `yss-node-registry` | 节点、类型和提供者注册，注册一致性校验与指纹                   |
| `yss-node-catalog`  | 内置节点定义、分类、创建描述、帮助文档与节点本地化目录         |

`build_builtin_node_system()` 组装并校验内置注册表和节点目录，返回共享的 `NodeRegistry` 与
`BuiltinCatalog`。`localize_with_resources` 合并调用方提供的资源创建描述；`text` 查询节点元数据文本。
扩展定义使用 `register_builtin_nodes(&mut NodeRegistryBuilder)` 将内置节点加入同一个 builder，
再注册应用 provider 并冻结；不另建内置定义副本。执行函数及其冻结注册表由 [yss-node-kernel](../yss-node-kernel/README.md) 拥有，目录不依赖该执行实现 crate。应用的 `NodeComponents` 校验已安装 kernel 的
参数字段与输出数量。Application 按当前会话的冻结内核注册表标注目录项的 `available`；结构节点无需叶节点内核。
GUI 创建目录保留完整分类与节点，兼容节点目录在端口匹配后同样保留不可用节点；缺少实现的节点置灰并标注“暂不可用”，禁止点击、键盘选择和拖拽创建。AI 搜索只返回可用项。
已有图中的缺少实现节点仍由编辑解析返回阻断诊断。
目录不读取项目文件，不维护图中实例，也不推导连接后的类型、Schema 或血缘。

“数据 → 常量”提供固定的 π 和 e 节点，无输入、无参数，输出 Numeric 标量；运行值由 Kernel 使用 Rust 标准库的 Float64 常量提供。它们不引用图内自定义常量，后者仍通过“读取常量”节点访问。

“数据处理”另有数据标签、数据编码、单次插补、多重插补和 MICE 插补五个目录入口，由 `src/dataframe/inventory.rs` 维护。它们复用目录骨架装配，保留用途与范围说明，端口、参数和内核待实现，当前显示为不可用。数据编码不等同于基础类型转换；MICE 是多重插补的具体算法，二者保留独立入口。

“可视化 / Visualization”使用 `plot` 分类 ID，包含原有散点、折线、ECDF、KDE、直方、相关性和自相关图节点。
另有箱线图、词云、误差线图、P-P/Q-Q 图、ROC 曲线、象限图、帕累托图、组合图、气泡图、小提琴图、热力图和系数图共 12 个新增目录入口。
新增入口由 `src/plot/inventory.rs` 维护；与统计目录骨架复用 `catalog_entry` 装配及说明逻辑，端口、参数和内核待实现，按缺少内核显示为不可用。已登记内容从 SCI 的 CSV/Excel 待办清单中移除。
KDE 已接入数值执行，`result` 输出为包含 256 个密度点的 `statistics.report` 结构化结果；当前不打开交互图窗。

统计节点的 `Family` 表示方法族，`Stage` 表示对该方法执行的操作，目录分类表示用户在哪里找到节点。
线性回归的 Fit、Summary、Predict 均使用 `Family::Linear`，预测由 `Stage::Predict` 表达。
当前操作还包括独立统计检验使用的 `Test`；独立诊断与后估计由 `statistics/analyses.rs` 按实际输入模型或序列定义，不新增伪造的模型方法族。
`category(spec)` 仅提供目录位置映射，不参与端口、模型类型或操作语义的判定。

已执行的独立检验、诊断和后估计节点使用 `src/docs/zh/`、`src/docs/en/` 的 Markdown 正文，
由 `src/documentation.rs` 映射并统一选择语言。`statistics/analyses.rs` 的简短本地化说明不替代完整帮助。
正文包含输入与参数、逐方法的假设与计算公式、输出口径、结果判读、适用条件及参考资料；
非检验类诊断说明其统计含义，不虚构原假设或 p 值。维护要求见 [节点文档规则](src/docs/.rules)。

统计目录在 `statistics` 下按以下顺序注册，分类不表示相应算法已实现。每个节点有一个主分类；跨领域检索复用节点别名，不重复注册节点。

| 目录                 | 分类 ID                       | 内容与边界                                     |
| -------------------- | ----------------------------- | ---------------------------------------------- |
| 描述统计             | `statistics.descriptive`      | Theil T；Gini、Dagum Gini 待实现               |
| 假设检验             | `statistics.tests`            | 均值、比例、列联表、分布检验、非参数检验       |
| 相关与一致性         | `statistics.association`      | 相关、偏相关、一致性、Kappa、ICC 等            |
| 回归模型             | `statistics.regression`       | 线性、广义线性、离散响应、正则化、非线性       |
| 方差分析             | `statistics.anova`            | 单因素、多因素、协方差分析、重复测量等入口     |
| 多元分析             | `statistics.multivariate`     | 主成分、因子、判别、典型相关等                 |
| 纵向与多层模型       | `statistics.longitudinal`     | GEE、LMM、GLMM、多层模型；统一导航但不混同方法 |
| 面板模型             | `statistics.panel`            | 固定效应、随机效应、组间与差分估计等           |
| 计量与因果分析       | `statistics.causal`           | 工具变量、GMM、DID、断点、匹配等子领域         |
| 时间序列             | `statistics.timeseries`       | 平稳性、协整、单变量与多变量模型、预测         |
| 生存分析             | `statistics.survival`         | 生存曲线、风险模型、参数生存模型等             |
| 空间分析             | `statistics.spatial`          | 空间设计对象、空间相关、空间回归               |
| 测量、问卷与结构方程 | `statistics.psychometrics`    | 信效度、测量模型、结构方程等                   |
| 综合评价与决策       | `statistics.decision`         | 赋权、排序、综合评价、决策方法                 |
| 机器学习             | `statistics.machine_learning` | 树、集成、聚类等；与回归目录交叉检索           |
| Meta 分析            | `statistics.meta`             | 效应量、合并模型、异质性、敏感性分析           |
| 实验设计与质量控制   | `statistics.design_quality`   | 实验设计、过程能力、控制图相关分析             |
| 功效与样本量         | `statistics.power`            | 按研究设计和检验目标组织                       |
| 复杂抽样分析         | `statistics.survey`           | 抽样设计、加权估计、设计型方差与回归           |
| 推断与重抽样         | `statistics.inference`        | 重抽样过程、区间构造、多重推断等               |
| 模型诊断与比较       | `statistics.diagnostics`      | 残差诊断、模型检验、模型比较                   |
| 预测与估计后分析     | `statistics.postestimation`   | 新数据预测、边际效应、调整后预测等             |

现有 IV 和 DID 节点归入“计量与因果分析”，Panel 模型归入“面板模型”；ADF、VAR、VEC 与协整检验归入“时间序列”。现有 Predict 节点归入“预测与估计后分析”；Summary 跟随方法所在主分类，不因包含诊断指标就归入“模型诊断与比较”。尚无节点的分类保留注册，展示与筛选由通用目录树处理。

所有统计 Summary 的唯一输入为对应方法的已拟合 `model`，输出 `result` 和 `report`，不接收原始数据或估计参数。
Linear Summary 在 Parameters 的 Configure 分组声明内容开关及条件可见的检验参数，默认选模型概览、系数表、方程和 ANOVA。IV、Panel、VAR、VEC Summary 同样按所选内容组装报告；IV/VAR/VEC 的可选诊断默认关闭，仅在勾选后计算。Panel 默认包含模型、系数、效应及估计器统计，VEC 默认包含协整统计。内容选择是图参数，参与语义失效、历史与保存。Logit/Probit/Prais Summary 投影拟合统计，不在模型中重复保存完整报告。
原始输入与估计配置属于 Fit；IV 2SLS、IV LIML、Panel、VAR 均有对应 Fit 定义，Panel DID 的 TWFE 节点也属于 Fit。
ADF 使用 `adf.test`，输入 `series`，以 `lags`、`regression` 配置检验，输出 `statistics.result.adf` 类型的 `result` 和 `report`。
`adf.summary` 已删除，不提供旧节点或旧端口的兼容转换。上述方法及 Logit/Probit/Prais、VEC 的模型节点均已注册执行内核。Logit/Probit 的截距、迭代次数和容差，Prais 的 Prais–Winsten/Cochrane–Orcutt 变换，以及 IV 的非稳健/HC0–HC3 和 small 参数均进入实际计算。IV 支持多个内生变量与排除工具变量，外生自变量可为空；Summary 可选择第一阶段与过度识别检验，2SLS 还可选择内生性检验。OLS/WLS 的 Cluster 标准误使用可选 `clusters` 输入，只有选择 Cluster 时才允许且必须连接。

Panel 统一选择 FE、LSDV、FD、RE FGLS、RE MLE 或 Between，并选择 entity/time/two_way 维度。FD 仅支持 entity，Between 不支持 two_way；MLE/Between 仅支持 nonrobust，LSDV 必须有截距。不适用组合明确拒绝。Panel、VAR、VEC 输出模型；TWFE DID 输出模型和报告。它们不声明尚无正确数列投影的 fitted/residuals 端口。VAR 使用截距和连续滞后；IRF/FEVD 独立节点的 steps 默认 8，可选 1–1000。VAR/VEC Summary 的残差诊断启用后使用 serial_lags，默认 2，可选 1–40。VEC/协整秩支持 none/constant/trend。TWFE DID 的 treatment 是已构造的 Treat×Post，伪处理组随机化另用独立节点输入 treat/post、置换次数和种子；没有事件研究参数。

独立 BP/White/IM/RESET/VIF/杠杆值/BG/系数 t–Wald 节点接收线性模型；正态性、DW、Ljung–Box、ACF/PACF 接收数值序列。VAR Granger/IRF/FEVD 接收已拟合 VAR，IV Hausman 接收非稳健 2SLS 模型，不代替面板 FE/RE Hausman。检验和后估计输出可查看的结构化统计结果及报告，复用实际拟合事实。

方法清单中的统计入口由 `src/statistics/inventory/entries.rs` 维护，运行时不读取规划 CSV。频数与描述已归入数据处理，分类汇总由 GroupBy 承接，独立基线分析入口已移除；FEVD 的两条来源共享一个入口。
这些来源记录继续保留方法身份；已经实现的诊断及后估计由 `statistics/analyses.rs`、描述统计由 `statistics/descriptive.rs` 完善原 ID 的端口、参数和内核绑定，不重复生成骨架。其余入口保留名称、搜索别名、分类、用途、来源编号和范围说明，尚无内核，仍在目录中显示为不可用且不进入 AI 可执行节点搜索。

泰尔指数沿用 `yssbi.statistics.inequality.theil`，计算自然对数 Theil T。Detail 的 `theil_form` 默认个体等权，分组形式输入组均值，并通过已有可选输入配置添加一个 `weights` 数列，表示组人数或人口占比。权重自动归一化，零权重组不计入计算；零值允许，负值、缺失值及非正加权均值拒绝。`result` 为 Numeric 标量，`report` 为包含指数、形式和输入观测数的结构化报告。分组结果仅反映组间差异，不推断组内差异或总体分解，详见节点帮助。
工作流模板、模型预设、结果指标、统计专用绘图和原理说明也按本轮要求登记入口；该登记不表示模板执行、参数预设或绘图能力已经实现。普通数据处理、缺失数据处理、通用绘图和 AI 模块未纳入本轮统计入口。
实现某项方法时直接完善其既有定义及内核绑定；只有存在可复用模型时才提供 Fit/Summary 分工，不从入口名称推断或批量生成端口契约。

“运算”下按“算术”“逻辑”“转换”排列；“类别转换”节点位于“转换”中，处理标量和数列的语义转换。分类及其中英文名称由 Rust 目录统一提供。

“算术”包含双输入的“幂”和“对数”，底数、指数或真数均可连线，支持标量和数列广播，输出实数。独立“指数函数”节点已移除，其用途由“幂”设置底数 e 表达；已有自然对数、以 2/10 为底的对数、平方和平方根保留。
自然对数、以 2/10 为底的对数、平方和平方根均有执行内核，支持数值标量、内存数列和惰性数列，保持输入形状并输出 Float64。定义域、精度及失败规则见对应节点帮助。

“逻辑”的六个比较节点同时支持标量和逐元素数列比较，默认精确比较，也可选择数值容差模式。容差内视为相等，再应用各比较运算符；相同配置下等于与不等于互补，小于与大于均为假，小于等于与大于等于均为真。数据序列目录不另设数值或字符串比较节点。

“概率分布”下分为“连续分布”和“离散分布”，分别包含 16 个连续分布采样节点和 7 个离散分布采样节点；分类在目录中显式声明，不通过参数或输出的数值表示推断。
23 个采样节点均已接入 SCI 执行：正态、连续均匀、指数、Gamma、Beta、
学生 t、柯西、卡方、对数正态、Weibull、Laplace、Pareto、逆 Gamma、三角、F、Erlang，
以及 Bernoulli、二项、泊松、几何、负二项、离散均匀和超几何分布。
参数在 Detail 中分为“分布参数”和“采样设置”两组，唯一输出为 Samples；样本数必须为正整数，分配前检查内存预算。
每次执行重新随机采样，不缓存结果、不在图编辑或分析时采样；输出连续分布的 Float64
或离散分布的 Int64 数列。参数须有限且符合分布定义，标准差、尺度、形状和自由度必须为正。
Bernoulli/二项允许概率 0 和 1；几何/负二项要求 0 < p ≤ 1；泊松率允许零。
超几何总体、成功数和抽取数为非负整数，后两者不能超过总体，零抽取返回零。
二项试验数、Erlang 整数形状和泊松率的精确计数上限为 2^53；几何、泊松及负二项生成值
超出精确计数范围时报错，离散均匀的端点则支持完整 Int64 范围。分布参数化详见各节点帮助。

“数据处理”下“数据帧”和“数据序列”为同级目录，按主要操作对象归类。时间序列对齐、面板对齐归入“数据帧”；时间序列差分、百分比变化、滚动均值、滞后及面板差分归入“数据序列”。面板差分的对齐数据帧用于提供分组上下文，实际变换和输出对象是数列。“时间序列”和“面板数据”不再单独设目录，节点定义和标识保持不变。
数据序列节点均已注册执行内核。选列保留惰性执行；标准化扫描统计量后保留关系行域，逆标准化沿用同一行域；其他变换按目录契约在执行时计算，
不会在图编辑或分析阶段扫描数据。整数范围包含起点、不包含终点，支持非零负步长。
长度包含 Null，非空计数排除 Null，求和忽略 Null 且空输入返回零，均值无有效值时返回 Null。
标准化使用样本标准差（ddof = 1），输出标准化值、均值及标准差；无有效方差时报错。
关系数列的标准化与逆标准化支持 Null 保留且可与原数列比较。浮点往返校验使用“等于”的容差模式，
默认绝对容差 1e-12、相对容差 1e-9，可在 Detail 配置中修改；默认精确模式不保证浮点往返全为真。
时序节点使用当前行顺序；高阶差分重复一阶差分，变化率采用比例值（0.1 表示 10%），
滚动均值要求完整非空窗口，滞后保留类型与元数据。面板差分通过实体列和时间列选项指定分组上下文，
不要求上游先执行面板对齐；结果恢复原始行顺序。虚拟变量信息只附加参照组提示，不生成指示列。

时间序列和面板对齐均接收一个 DataFrame，通过 time_column、entity_column（仅面板）及正整数 interval 配置。输出保留列顺序、类型和元数据，新增非键单元格为 Null；重复键、缺失键、偏离网格或超出内存预算会失败。面板网格使用共享已观测时间的位置，仅补齐各实体自己的起止范围，详见节点帮助。
数据帧的筛选入口为“筛选行”（配置列条件）和“筛选列”（选择并排列列）。
“移除行”复用行条件配置，仅删除条件为真的行，保留条件为假或 NULL 的行；“为空”条件可显式移除缺失值。“移除列”复用列选择器，删除指定列并保留其余列的顺序、类型与血缘；至少选择一个现有列且至少保留一列。两个节点均产生新的计算结果，不修改源数据集。
数据帧组合分为“按行拼接”“按列拼接”“连接”和“组装数据帧”。按行拼接支持列名/位置对齐；连接支持多列键及四种连接类型，复用列选择器分别展示左右输入的列；组装允许混合语义数列。字段命名与输入顺序由 Graph 的组合 Schema resolver 推导，执行由 Kernel 经关系契约交给 DataFusion。

数据序列增加“频数”和“描述”，数据帧增加“描述”和“分组聚合”。四者输出可分页、可连接的数据帧；原统计目录的频数、分类汇总、描述和基线分析占位定义已移除，不保留旧 ID 转换。
描述支持 Numeric、Categorical、Ordinal、Binary；后三者采用分类摘要，整数编码不改变语义。Text、Datetime、Identifier 不自动作为类别；数据帧描述默认选择全部受支持列，显式选列必须受支持。
数值摘要为有效数、缺失数、均值、样本标准差（ddof=1）、最小值、q25、中位数、q75、最大值；分位数在 `(n-1)p` 位置线性插值。
分类摘要为有效数、缺失数、不同取值数、众数及其频数和有效样本占比。众数保留原始编码的文本；并列时 Ordinal 取声明顺序最先者，其余取原值最小者。两种描述入口复用同一结果列契约，每个变量一行；不适用或未定义的指标为 Null。
频数输出原类型的 `value`、`frequency`、`proportion`，不截断为 Top N；默认包含 Null，关闭时同时排除空值组及其分母贡献。Ordinal 按声明等级排序，其余按原值升序，Null 最后；空字符串保留。
GroupBy 必选一个或多个分组键，始终输出 `row_count`，并可分别选择 count/sum/mean/min/max/std/median 的列。同一列可参加多种聚合，输出为 `列名_操作`；名称冲突拒绝执行。count 排除 Null，其他聚合只接受 Numeric、忽略 Null，全空数值组返回 Null；Null 分组键保留。
数值统计要求无损提升为 Float64，非有限输入、溢出或非有限结果使结果消费失败，不转换为成功空值。选择器的可选列、参数诊断与输出 Schema 都由 Graph 的 Rust 解析产生；编辑阶段不扫描数据。

“删除缺失行”和“删除缺失列”位于“数据帧”，分别对应 `yssbi.dataframe.dropna.rows` 和
`yssbi.dataframe.dropna.columns`。`subset` 是检查列名列表，空列表表示全部列；`how` 为 `any` 或
`all`，删行默认 `any`，删列默认 `all`。只检查 Null，NaN、空字符串及零值不视为缺失。
两者均生成惰性关系，不修改原数据集。空表保留列；全部列被删除时仍保留行数。
删列消费完整输入后确定列，再进行分页，不能用当前页推断整列是否缺失。

删列输出的 Schema 为 Deferred，Graph 不扫描数据，也不伪造静态列清单。Results 可直接分页；
可继续连接删除缺失行、删除缺失列或限制行。依赖固定列结构的选择列、拆分、数列选择及组合节点
在图分析阶段保持阻断，不能把未确定的列当作固定列使用。

三个 crate 均不依赖 Graph。Node Protocol 依赖序列化基础库和 Data Contract 的唯一基础语义定义；Registry 另用规范化哈希；
Catalog 消费 Protocol、Registry 和 SCI 的中立配置契约。
图文档与语义快照属于 Graph，计划构建与缓存属于 Execution；图诊断定义、校验和前端模板生成
属于 Graph 诊断链路，不会装配进节点目录。

## 参数声明

动态节点 ID 和本地化 key 使用拥有型字符串，由装配片段、协议和目录持有并随其释放。
节点 key 由 `builtin::node_key` / `node_key_text` 统一构造；静态文案仍借用常量，不要求动态 key 具有永久生命周期。

`yss-node-protocol` 使用 `Parameters → ParameterGroup → Parameter` 声明节点参数。
分组 key 在节点内唯一，参数 key 在所有组之间唯一；声明顺序决定 Details 中的组和字段顺序。
组拥有本地化标题和可选说明，参数拥有类型、默认值、约束、编辑器和 `visible_when` 条件。
无参数节点使用空 `Parameters`，有参数的组不能为空。注册阶段重新校验分组、默认值、条件引用与本地化 key。

```rust
fn normal_parameters() -> Result<Parameters, ParametersError> {
    Parameters::new([
        ParameterGroup::new("distribution", [
            Parameter::number("mean").float().default(0.0),
            Parameter::number("standard_deviation").float().positive().default(1.0),
        ]),
        ParameterGroup::new("sampling", [
            Parameter::number("sample_count").int().positive().min(1).default(100),
        ]),
    ])
}
```

构造器用于可信节点声明，非法 key 或数值字面量立即报错。数值 builder 的 `int()` 增加整数约束，
`min(i64)` 设置整数下界；`float()` 使用 Numeric 浮点默认值。参数标题 key 默认为
`parameters.{key}.title`，组标题默认为 `parameter_groups.{key}.title`，均须登记本地化资源，
也可用 `title(I18nKey)` 指定目录自己的 key。

参数值始终按参数 key 扁平保存，分组不创建对象值或执行参数。分组名称、顺序和字段归组不进入
协议执行指纹；参数类型、默认值、约束及条件显隐参与指纹。条件可以引用同节点其他组的无条件参数，
使用显式值或协议默认值判断。编辑、条件清理及执行投影见 [Graph analysis](../yss-graph-analysis/README.md)。

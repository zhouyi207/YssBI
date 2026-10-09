# yss-node-catalog

`statistics/psychometrics` declares Cronbach-alpha reliability, KMO/Bartlett
screening, expert content validity and item discrimination. Its documentation
states scoring, tie handling and undefined-statistic rules; existing exploratory
factor nodes remain the owner of factor extraction interfaces.
`statistics/quality` declares I/MR plots, capability with optional rational
subgroups, and balanced crossed continuous-measurement Gage R&R studies.
`statistics/doe` owns experimental factor-level analysis and coded design generation interfaces, plus quadratic response surface and four-parameter
continuous dose-response fits, separately
from process monitoring and measurement-system declarations.
Full factorial and orthogonal designs require at least two levels per factor; parameter validation enforces the same lower bound as SCI execution.

> Status: Current
> Scope: 内置节点定义、创建描述、分类、文档与节点目录本地化
> Canonical owners: 本 crate 的源码拥有内置目录；Node 与 Graph 的边界见 [Graph 与 Execution](../yss-application/src/graph/README.md#module-ownership)
> Update when: 节点目录装配、创建描述、本地化接口或依赖边界改变时

`dataframe/labels` exposes categorical and ordinal value-label authoring over the
existing semantic conversion contract. It is separate from column and node display names.

Post-hoc comparison tables declare numeric group IDs and explicit text columns
`group_a_label` / `group_b_label`, followed by the existing numeric comparison statistics.

Node 由三个 crate 组成：

`node_creation_ports` 从同一协议导出固定、用户可配置和派生端口的创建信息，GUI 与 Harness 共用。
Protocol 的 `initial_port_counts` 解释创建请求中的最终数量，统一校验上下界和 member group 联动；
固定及派生端口不能覆盖数量，未指定的可变模板使用协议最小数量。Catalog 不维护第二套端口规则。

内置数据端口由 `data_connections` 统一声明连接数量：输入只接收一个来源，输出允许无固定数量上限的多个下游。重路由、分布和绘图节点复用同一声明。
重路由以 Registry 的透明角色及 Identity 类型规则登记；标量、数列和表格共享同一协议，
不声明仅适用于表格的 Schema。编辑器插入复用这里的 canonical 协议校验，分析与执行按注册角色处理。

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

`catalog_search_text` 从同一条目录元数据生成只读检索文本，覆盖标题、别名、技术术语、后端关键词、资源名和节点类型 ID。
[search](src/search.rs) 复用 `unicode-normalization` / `unicode-casefold` 统一宽度、大小写和组合音标；
标点与空白作为分词间隔。[`pinyin_pro`](https://github.com/argsno/pinyin-pro-rust) 的词组字典生成无声调全拼和首字母，保留“重复”“重庆”等上下文读音。
调用方对查询使用 `normalize_catalog_search_text`，按所有查询词均为子串匹配；搜索不改变创建描述或资源身份。
Application 在构建活动面板投影的 worker 中生成该文本；原生视图只筛选索引，不在每帧重复转写或另存目录实体。

本地化语言标签按大小写不敏感匹配，`_` 与 `-` 分隔均可；精确语言词条优先，其次同语言词条，最后回退 `en-US`。已命中的语言包直接返回词条，节点标题、别名和参数文案复用同一个查询入口。

嵌入帮助同时提供中英文正文，静态帮助与生成帮助共用语言选择规则；只有主语言为 `zh` 的标签选择中文，其他语言使用英文。
`node_documentation` 按类型直接读取同一份嵌入或生成帮助，不构造目录、参数或资源条目。
当前会话的注册状态由 Application 校验；纯文档读取不表示该类型可创建或已有执行内核。

`authoritative_static_descriptor(protocol)` 从协议生成普通创建描述。带有尚未填写的必填文本、
数值或结构化参数的节点仍可作为 `parameterizedStatic` 进入目录；参数不必先有实际输入。
资源绑定节点只通过捕获了资源身份和 revision 的目录描述创建，managed 和 hidden 节点维持专用边界。
创建请求的实例参数由 Graph Editor 接收，不属于目录描述；参数声明、默认值及约束仍只来自 Protocol。

`NodeProtocol::configuration_schema()` 将同一声明投影为 `parameters` / `portCounts` 的 JSON Schema。
固定结构复用 Schemars 和 Data Contract 类型；列列表与筛选谓词的结构由 Protocol 的 nominal 类型提供。
所有参数允许省略或用 Null 重置，必填项以 `x-yss-requiredForExecution` 标明；
`x-yss-activeWhen` 描述条件字段，`x-yss-linkedPorts` 描述初始数量联动。
Hidden 参数不进入配置 Schema，固定和派生端口不能通过 `portCounts` 配置。
Application 只补充当前语言的标题与说明，Harness 按需交付 Schema，不维护另一套参数声明。
这是未连接时的配置描述；连接后的列选项、nominal codec、端口组一致性和执行完整性仍由原有
Protocol、Registry 与 Graph owners 校验，生成 Schema 不执行节点或读取数据。

`View Data` 的输入只接受已连接的输出，不接受内联字面量或默认值；Catalog 声明这一输入策略，
Analysis 和 Editor 共用协议校验。View 观察已有输出结果，不创建独立的结果值；查看标量时可连接常量输出。

注册失败分为注册一致性错误与规范编码错误。接口和执行声明验证产生的 `ProtocolError`
通过 `RegistryValidationError::InvalidNodeProtocol` 保留节点身份与原始错误来源。

`NodeInterfaceProtocol::port_instance_bounds` 提供接口内模板的数量范围：声明端口为 1，
用户创建端口使用所属成员分组或自身的上下限，派生端口保持开放范围。Editor 的初始候选筛选与
Application 的内核输入/输出数量校验共用此读取；它不保存实例、计算当前成员数或代替完整分组的创建与删除。

`PortSpec` 直接以 `y` / `x` 声明主因变量 / 主自变量输入，标题为 `Y` / `X`。
Catalog、Kernel 和图调用共用这些端口 key；固定与重复输入通过既有 cardinality 区分。
重复输入由 Analysis 按当前顺序显示为 `X₁、X₂…` 或 `Y₁、Y₂…`，权重、分组、时间等辅助输入保留原名。

Registry 的 `StructuralNodeRole` 拥有函数角色的引用字段约定：Call 使用 `target`，Entry/Return
使用 `function`，节点类型 ID 不参与角色判定。`RegisteredNode::function_reference_parameter`
提供声明字段，`function_reference` 通过 Protocol 的 `Parameters::effective_text` 借用当前适用的
显式值或默认值；非法显式值不回退到默认值。这些读取不解释项目路径或访问资源。
四个函数 interface resolver ID 及其引用角色映射也由 Registry 唯一提供，Catalog、Analysis、Editor 和 Project
共用这些声明。resolver 复用同一字段约定，也可由只消费函数签名的叶节点使用；使用 resolver 本身不构成函数调用。

GroupApply/GroupTransform 同样以 `target` 引用函数，`calls_function` 统一识别直接调用及按组调用，
供依赖捕获、递归检查和 Project 引用追踪使用。`dataframe/grouping` 声明独立的 GroupBy 分组值、
Apply 和 Transform；后两者使用固定分组输入/表格输出，由 Execution 调用一个 DataFrame 参数且
返回 DataFrame 的图函数。创建时可同时填写函数和 Apply 的 `key_prefix`（默认 `group.`）。
原有 `yssbi.dataframe.groupby` 保持固定聚合，英文标题为 Grouped Aggregation；不替换其原生查询路径。

`statistics/decision` 声明客观赋权与评分入口，共用指标方向、评分标准化和明确的
权重方法参数。外部权重使用独立 `criterion_weights` 输入，计算权重由 `weights`
输出；摘要和逐行评分分别输出，TOPSIS 评分表还包含两个理想点距离。

同一目录按 `preferences`（NPS/KANO/RFM）、`market`（价格敏感度与 TURF）、
`matrices`（判断矩阵、影响关系与隶属度）、`experts`（德尔菲单轮统计）分开声明。
`parameters` 只持有本地化文案，算法定义在 SCI。PSM 明确区分两种价格区间约定；
矩阵行列顺序、空值含义及可继续连接的权重/隶属度均在双语帮助中说明。

`statistics/inference` 声明置信区间、多重比较、聚类稳健标准误和调整预测。
`statistics/ports` 共享分类标签输入、Numeric/Binary 数列联合类型、线性/二元拟合联合类型及固定表输出声明。
分组、类别、空间标识和离散状态端口复用同一标签数列联合类型。
固定表按列名与语义类型直接声明；纯数值表复用 Numeric 声明，多重比较表直接声明文本标签列，无需按列位置二次修改 Schema。
区间与比较表的字段由声明唯一持有，Graph 统一解析；中英文帮助明确推断自由度、
多重校正方式及均值置信区间的含义。

单次插补由 `dataframe/transforms` 声明，导航属于“统计 → 缺失值处理”。输入与输出均为
Numeric 数列，帮助说明全部样本估计填补值、空列规则和单次插补的推断局限。

“数据 → 常量”提供固定的 π 和 e 节点，无输入、无参数，输出 Numeric 标量；运行值由 Kernel 使用 Rust 标准库的 Float64 常量提供。它们不引用图内自定义常量，后者仍通过“读取常量”节点访问。

单次插补、多重插补和 MICE 插补归入“统计 → 缺失值处理”（`statistics.imputation`，英文 `Missing Value Handling`）。多重插补和 MICE 入口由 `src/dataframe/inventory.rs` 维护；MICE 是 MI 的具体算法，二者保留独立入口。

“数据处理 → 数据序列”中的“数据标签”由 `src/dataframe/labels.rs` 声明。数据编码入口已实现为数据序列目录中的“虚拟变量生成”，沿用 `yssbi.dataframe.encode`，不代替基础类型转换。

“可视化 / Visualization”使用 `plot` 分类 ID，19 个节点均在 `src/plot/mod.rs` 声明并由 Kernel 执行：散点、折线、ECDF、KDE、直方、相关性、自相关、箱线、词云、误差线、P-P/Q-Q、ROC、象限、帕累托、组合、气泡、小提琴、热力和系数图。
直方图、经验累积分布图和箱线图分别使用独立节点，可将同一数列连接到三个节点以查看分布。
输出统一为 `plot.data` 结构化绘图数据，由既有 Result 查询端口交付给宿主；原生绘图与结果呈现由 [GPUI host](../yss-desktop-gpui/README.md) 拥有。
数据帧和绘图节点 ID 直接作为叶内核身份；绘图端口声明不提供默认字面量。
DataFrame 固定输入工厂不接收未使用的 Schema 参数；输出仍按各方法声明 Schema。变换 `Kind` 统一判定表格变换，分类和接口构造复用同一判定。
配对输入须等长并按当前位置对应，可混合数据库与内存数列；箱线/小提琴组为独立样本，可不等长；缺失值不隐式删除。
KDE 默认 256 个网格点，系数图接收线性 Fit 的 OLS/WLS/GLS 模型。所有节点均有中英文帮助，说明参数、计算和展示点数限制。

“时间序列”分类的 ARIMA/SARIMA、ECM、ARCH/GARCH/EGARCH/GJR-GARCH、
指数平滑/ETS/Holt–Winters、灰色与马尔可夫预测、PP/KPSS 以及时序图/相关图
由 `statistics/time_series.rs` 提供可执行接口，保留原清单 ID。算法形式、默认参数、
条件初始化、样本要求、检验原假设和输出索引均在双语帮助中说明。没有固定样本行数
上限；相关图仅沿用现有显示滞后预算。图形入口输出 `plot.data`，其余输出 `statistics.report`。

“生存分析”分类由 `statistics/survival.rs` 实现 15 个原有入口：KM、NA、Log-rank、
Cox、四种参数生存回归、AFT、Aalen–Johansen 竞争风险、时变 Cox、分层 Cox 亚组、
列线图、校准和决策曲线。静态 Cox 的 `result` 为 `statistics.model.cox`，可接列线图；
Cox 和参数模型另输出 `predictions` 表（`time`、`event`、`risk`），供评估节点保持行对齐。
该表与残差/Cook 的 `observations` 都复用 `statistics/ports::fixed_numeric_table`，由 Catalog
声明字段与 Numeric 语义。Graph 统一解析 `SchemaExpr::Fixed`，没有对应的逐方法 Schema resolver。

“空间分析”分类由 `statistics/spatial.rs` 实现 10 个原有入口：权重、Moran、
OLS/SLX、SLM/SEM/SAC/SDM/SDEM 和空间面板。权重节点输出
`statistics.design.spatial_weights`，下游通过地区标识显式对齐；其余节点输出
`statistics.report`。权重支持平面 K 近邻、距离阈值与反距离；空间面板支持平衡
实体固定效应的 SLM/SEM。双语帮助说明稳定区间、推断、效应和结果索引。
三个绘图入口输出 `plot.data`，其余分析输出结构化报告，沿用原节点 ID 和分类。
完整的删失约定、风险集、检验及限制由映射的中英帮助说明。

统计节点的 `Family` 表示方法族，`Stage` 表示对该方法执行的操作，目录分类表示用户在哪里找到节点。
线性回归的 Fit、Summary、Predict 均使用 `Family::Linear`，预测由 `Stage::Predict` 表达。
同一方法族的 Fit、Summary 和 Predict 端口复用模型类型映射；预测操作另校验支持的方法族。
当前操作还包括独立统计检验使用的 `Test`；独立诊断与后估计由 `statistics/analyses.rs` 按实际输入模型或序列定义，不新增伪造的模型方法族。
`category(spec)` 仅提供目录位置映射，不参与端口、模型类型或操作语义的判定。

已执行的独立检验、诊断和后估计节点使用 `src/docs/zh/`、`src/docs/en/` 的 Markdown 正文，
由 `src/documentation.rs` 映射并统一选择语言。`statistics/analyses.rs` 的简短本地化说明不替代完整帮助。
`statistics/classical` 中相同的备择方向参数共用一份声明。
正文聚焦输入与参数、逐方法的假设与核心公式、输出口径和必要的判读条件；
符号随公式简要说明，不展开基础知识章节、完整推导或内部数值分支。
非检验类诊断说明其统计含义，不虚构原假设或 p 值。维护要求见 [节点文档规则](src/docs/.rules)。

统计目录在 `statistics` 下按以下顺序注册，分类不表示相应算法已实现。每个节点有一个主分类；跨领域检索复用节点别名，不重复注册节点。

| 目录                 | 分类 ID                       | 内容与边界                                       |
| -------------------- | ----------------------------- | ------------------------------------------------ |
| 描述统计             | `statistics.descriptive`      | 数据描述、Gini、Dagum Gini 分解、Theil T         |
| 假设检验             | `statistics.tests`            | 均值、比例、列联表、分布检验、非参数检验         |
| 缺失值处理           | `statistics.imputation`       | 单次插补、多重插补与 MICE 插补                   |
| 相关与一致性         | `statistics.association`      | Pearson/偏相关/秩相关、Kappa、ICC、W、Ridit、rwg |
| 回归模型             | `statistics.regression`       | 线性、广义线性、离散响应、正则化、非线性         |
| 方差分析             | `statistics.anova`            | 单因素、多因素、协方差分析、重复测量等入口       |
| 多元分析             | `statistics.multivariate`     | 主成分、因子、判别、典型相关等                   |
| 纵向与多层模型       | `statistics.longitudinal`     | GEE、LMM、GLMM、多层模型；统一导航但不混同方法   |
| 面板模型             | `statistics.panel`            | 固定效应、随机效应、组间与差分估计等             |
| 计量与因果分析       | `statistics.causal`           | 工具变量、GMM、DID、断点、匹配等子领域           |
| 时间序列             | `statistics.timeseries`       | 平稳性、协整、单变量与多变量模型、预测           |
| 生存分析             | `statistics.survival`         | 生存曲线、风险模型、参数生存模型等               |
| 空间分析             | `statistics.spatial`          | 空间设计对象、空间相关、空间回归                 |
| 测量、问卷与结构方程 | `statistics.psychometrics`    | 信效度、测量模型、结构方程等                     |
| 综合评价与决策       | `statistics.decision`         | 赋权、排序、综合评价、决策方法                   |
| 机器学习             | `statistics.machine_learning` | 树、集成、聚类等；与回归目录交叉检索             |
| Meta 分析            | `statistics.meta`             | 效应量、合并模型、异质性、敏感性分析             |
| 实验设计与质量控制   | `statistics.design_quality`   | 实验设计、过程能力、控制图相关分析               |
| 功效与样本量         | `statistics.power`            | 按研究设计和检验目标组织                         |
| 复杂抽样分析         | `statistics.survey`           | 抽样设计、加权估计、设计型方差与回归             |
| 推断与重抽样         | `statistics.inference`        | 重抽样过程、区间构造、多重推断等                 |
| 模型诊断与比较       | `statistics.diagnostics`      | 残差诊断、模型检验、模型比较                     |
| 预测与估计后分析     | `statistics.postestimation`   | 新数据预测、边际效应、调整后预测等               |

问卷多选题统计与单选/多选题型交叉组合不设独立目录入口。量表题项诊断入口显示为
“题项分析（区分度）”（`yssbi.statistics.psychometrics.item_analysis`），支持“题项分析”
和“区分度分析”搜索别名；内核调用 SCI 题项分析，输出汇总、题项指标表及观测分组。

“纵向与多层模型”的十个既有 ID 由 `statistics/longitudinal.rs` 提供完整接口及内核绑定。
GEE 支持 Gaussian/二项/Poisson，独立或可交换工作相关，输出按组稳健协方差。
LMM、随机截距和随机斜率使用 ML/REML；随机斜率采用独立方差分量，HLM 支持从低到高的
一个或多个嵌套分组，交叉模型支持至少两个分组。GLMM 及 Logistic/Poisson/NB2 预设提供
单分组随机截距的 Laplace ML，NB2 同时估计过度离散参数。
默认含固定截距，可选固定自变量；每因素至少两个实际组，不设固定行数、列数或组数上限。
数据规模由可识别性与执行预算决定；独立相关的 GEE 支持单行组。
唯一 `result` 保留推断、方差、原分组标签及原行序拟合/残差数组，复用 Inspect；
完整输入边界、近似推断和协方差约定见已注册的中英文节点帮助。

“方差分析”的七个既有入口均由 `statistics/anova.rs` 完善为可执行节点，保留原 ID。
单因素、双因素和三因素分别要求 1、2、3 个分类因素；多因素与 ANCOVA/MANOVA 支持一个或多个因素。
因素输入接受数值编码、分类、有序、二元、文本及标识符数列，每项有 至少两个观测类别。
多因素模型可选主效应或完整因素交互及 I/II/III 型平方和，默认完整交互与 III 型；设计含截距，秩亏或无残差自由度明确拒绝。
ANCOVA 另接 至少一个连续协变量，使用中心化的加性平行斜率；MANOVA 接 至少两个响应，输出 Wilks/Pillai/Hotelling–Lawley/Roy 检验和 SSCP。
重复测量使用长表的 response、subjects 与 一个或多个受试者内因素，要求每个受试者在全部组合上恰好一行，默认 Greenhouse–Geisser 校正。
七者均只输出可由 Inspect 查看数值与 JSON 的结构化 `result`，完整中英文帮助由 `src/documentation.rs` 映射。

“多元分析”的七个既有 ID 由 `statistics/multivariate.rs` 完善：典型相关、探索性因子、PCA、对应分析、判别分析、RDA 和 MDS。
典型相关保留 `yssbi.statistics.association.canonical`，目录位置为 `statistics.multivariate`；聚类与分层聚类仍属于机器学习分类。
每个节点输出结构化 `result`；PCA、主轴因子、CCA 和 RDA 另有得分表，CA 有行/列主坐标表，经典 MDS 有坐标表，LDA/QDA 有保留原类别语义的预测数列。
得分与坐标字段由 `components` 参数在 Graph 中推导，编辑不扫描数据；可继续选列及绘图，观测得分不塞入摘要 JSON。CCA 将 X/Y 得分置于同一表以保留可证明的相互对齐。
数值列、类别和观测数不设固定上限；保留维数由实际数据维度、秩及方法的可识别性确定，计算受内存预算及执行控制约束。
PCA 支持相关/协方差形式；主轴因子支持无旋转/正交 varimax、KMO/Bartlett 与回归得分；LDA/QDA 支持先验、收缩和可选独立新数据；RDA 提供可复现行置换。
各方法的范围、默认值、假设、报告口径与可连接数据输出见已注册的中英文 `multivariate_*.md` 帮助。

现有 IV 和 DID 节点归入“计量与因果分析”，Panel 模型归入“面板模型”；ADF、VAR、VEC 与协整检验归入“时间序列”。现有 Predict 节点归入“预测与估计后分析”；Summary 跟随方法所在主分类，不因包含诊断指标就归入“模型诊断与比较”。尚无节点的分类保留注册，展示与筛选由通用目录树处理。

所有统计 Summary 的唯一输入为对应方法的已拟合 `model`，只输出结构化 `result`，不接收原始数据或估计参数。统计检验、诊断和描述结果同样不另设 `report` 端口；Inspect 在同一结果上切换数值与 JSON 报告。Fit 的模型、拟合值与残差等独立数据输出保留各自契约。
共享参数说明按其用途区分汇总内容选择、响应期数、滞后阶数和系数约束；独立检验与估计后分析复用对应说明。

IV 2SLS/LIML、Panel、VAR、VEC Summary，以及 Panel Compare 和 VAR Lag-order Selection 的 `result` 声明为既有 `statistics.report`；报告不能再连接需要拟合模型的 Summary、Predict 或估计后分析输入。真正 Fit 的模型类型保持独立，报告呈现仍由 Graph 的输出类别决定。

独立的 Panel DID (TWFE) 使用 `statistics.result.panel_did` 类型的唯一 `result`，其 `model` 与 `summary` 字段分别保留拟合模型和汇总。
Linear Summary 在 Parameters 的 Configure 分组声明内容开关及条件可见的检验参数，默认选模型概览、系数表、方程和 ANOVA。IV、Panel、VAR、VEC Summary 同样按所选内容组装报告；IV/VAR/VEC 的可选诊断默认关闭，仅在勾选后计算。Panel 默认包含模型、系数、效应及估计器统计，VEC 默认包含协整统计。内容选择是图参数，参与语义失效、历史与保存。Logit/Probit/Prais Summary 投影拟合统计，不在模型中重复保存完整报告。
原始输入与估计配置属于 Fit；IV 2SLS、IV LIML、Panel、VAR 均有对应 Fit 定义，Panel DID 的 TWFE 节点也属于 Fit。
ADF 使用 `adf.test`，输入 `series`，以 `lags`、`regression` 配置检验，唯一输出为 `statistics.result.adf` 类型的结构化 `result`。
`adf.summary` 已删除，不提供旧节点或旧端口的兼容转换。上述方法及 Logit/Probit/Prais、VEC 的模型节点均已注册执行内核。Logit/Probit 的截距、迭代次数和容差，Prais 的 Prais–Winsten/Cochrane–Orcutt 变换，以及 IV 的非稳健/HC0–HC3 和 small 参数均进入实际计算。IV 支持多个内生变量与排除工具变量，外生自变量可为空；Summary 可选择第一阶段与过度识别检验，2SLS 还可选择内生性检验。OLS/WLS 的 Cluster 标准误使用可选 `clusters` 输入，只有选择 Cluster 时才允许且必须连接；端口复用共享标签数列类型，接受数值、分类、顺序、二元、文本及标识语义，宽整数标识不经浮点转换。

Panel 统一选择 FE、LSDV、FD、RE FGLS、RE MLE 或 Between，并选择 entity/time/two_way 维度。FD 仅支持 entity，Between 不支持 two_way；MLE/Between 仅支持 nonrobust，LSDV 必须有截距。不适用组合明确拒绝。通用 Panel Fit 输出模型及估计尺度的 fitted/residuals；源行分组保留在模型中。VAR 使用截距和连续滞后；IRF/FEVD 独立节点的 steps 默认 8，可选 1–1000。VAR/VEC Summary 的残差诊断启用后使用 serial_lags，默认 2，可选 1–40。VEC/协整秩支持 none/constant/trend。TWFE DID 的 treatment 是已构造的 Treat×Post，伪处理组随机化另用独立节点输入 treat/post、置换次数和种子；没有事件研究参数。

`statistics/panel_models.rs` 完善“面板模型”中七个既有 `econometrics.panel.*` ID。
FE、RE FGLS、FD、Between 复用 Panel 估计器，唯一 `model` 可直接连接现有 Summary/Predict；
四者支持一个或多个自变量，实体和时间继续使用数值数列。动态面板输出结构化 `result`，采用
折叠工具的一步 Arellano–Bond 差分 GMM，支持可选的严格外生自变量及实体稳健/非稳健协方差。
它要求平衡、连续的整数期次、至少四期，实体数大于工具数；不声明系统/两步 GMM 或工具有效性检验。
面板单位根为 Fisher–ADF，协整为 Fisher–Engle–Granger（1–5 个自变量），均返回各实体检验与合并结果。
两项检验支持非平衡面板，但实体内部须连续；合并推断要求横截面独立，使用正确的 MacKinnon 校准。协整的五自变量上限来自校准表适用维度；ADF 滞后由实际样本自由度约束。
这三项的期次、缺失、重复键、秩、资源预算及执行控制明确校验，14 篇中英文帮助说明默认参数、
输出坐标、假设与范围；不从原占位名称推定其他面板检验方法。

独立 BP/White/IM/RESET/VIF/杠杆值/BG/系数 t–Wald 节点接收线性模型；正态性、DW、Ljung–Box、ACF/PACF 接收数值序列。VAR Granger/IRF/FEVD 接收已拟合 VAR，IV Hausman 接收非稳健 2SLS 模型，不代替面板 FE/RE Hausman。检验和后估计输出可查看的结构化统计结果及报告，复用实际拟合事实。

“模型诊断与比较”的其余 11 个入口由 `statistics/analyses/models.rs` 提供完整契约，沿用原 ID。
共线性接收数值设计列；Harman 明确使用未旋转相关矩阵 PCA；NRI/IDI 接收同一验证样本的
二元结局和两组概率，提供连续/风险分类 NRI 与 IDI 点估计。AIC/BIC 和 LR/Score/嵌套比较
支持 OLS/WLS、Logit/Probit，比较校验同方法、响应行序、相对权重及设计嵌套。
残差和 Cook 距离接收 OLS/WLS 模型，另输出 `observations` 数据表供分页与下游选列。
PH 节点从原始对齐列拟合静态无分层 Cox，使用与拟合一致的 Efron/Breslow 时间交互 Score。
22 篇中英文帮助明确公式、推断、默认值及未定义值；新增接口不设置固定样本行数上限。

Meta 分析的 20 个既有入口由 `statistics/meta/` 提供端口与参数，40 篇中英文帮助
由 `documentation.rs` 映射。效应量换算、合并/回归和敏感性节点同时输出可分页、
可连接的研究明细表；森林图复用系数区间图，漏斗图复用带参考线和反向纵轴的散点图。
固定输出表由 Catalog 通过 `SchemaExpr::Fixed` 声明字段；Registry 校验唯一非空列名
并拒绝预置运行时血缘，Graph 统一解析，不再为这些表重复注册逐方法 Schema resolver。
具体方法、尺度与推断口径由节点帮助说明。复杂节点的暂缓范围见
[暂缓节点](DEFERRED_NODES.md)；暂缓节点保留目录身份与不可用状态。

方法清单中的统计入口由 `src/statistics/inventory/entries.rs` 维护，运行时不读取规划 CSV。频数归入数据序列，数据描述归入描述统计，分类汇总由 GroupBy 承接，独立基线分析入口已移除；FEVD 的两条来源共享一个入口。
装配先收集具体节点定义，再按已声明的节点身份补齐清单中的占位入口，不另维护“已实现”分类函数或状态清单。节点能否执行仍由 Application 使用当前冻结 Kernel 注册表判断，声明存在不代表内核已经安装。
这些来源记录继续保留方法身份；已经实现的诊断及后估计由 `statistics/analyses.rs`、描述统计由 `statistics/descriptive.rs` 完善原 ID 的端口、参数和内核绑定，不重复生成骨架。其余入口保留名称、搜索别名、分类、用途、来源编号和范围说明，尚无内核，仍在目录中显示为不可用且不进入 AI 可执行节点搜索。

泰尔指数沿用 `yssbi.statistics.inequality.theil`，计算自然对数 Theil T。Detail 的 `theil_form` 默认个体等权，分组形式输入组均值，并通过已有可选输入配置添加一个 `weights` 数列，表示组人数或人口占比。权重自动归一化，零权重组不计入计算；零值允许，负值、缺失值及非正加权均值拒绝。唯一 `result` 为包含 `theil_t`、`form` 和 `observations` 的结构化数据。分组结果仅反映组间差异，不推断组内差异或总体分解，详见节点帮助。

Gini 与 Dagum Gini 沿用 `yssbi.statistics.inequality.gini`、`yssbi.statistics.inequality.dagum_gini`，均已注册执行内核，使用个体等权、未经小样本修正的经验 Gini。Gini 输入 `series`；Dagum 另需等长且按当前位置对应的 `groups` 标签列，仅接受 Numeric、Categorical、Ordinal 和 Binary 语义。Text、Identifier 必须显式转换为分类后接入，端口候选与执行前检查共用 Graph 类型约束；分类标签仍可保留字符串原值。唯一结构化 `result` 可通过既有 Inspect 查看数值或报告；Dagum 返回组内、组间净差异、超变密度、贡献占比及分组/组对明细。全零子组的未定义 Gini 和零总体差异下的贡献占比保留为 null，非正总体均值拒绝计算。输入与解释见各节点中英文帮助。

`statistics/association.rs` 完善原“相关与一致性”目录的 10 个 ID：Pearson、偏相关、Spearman、Kendall tau-b、Kappa、ICC、Bland–Altman、Kendall W、Ridit、rwg。全部只有结构化 `result`，复用 Inspect。配对和评定者列必须对齐，Ridit 的样本/参考总体允许独立读取。Spearman/Kendall/W 支持显式 Ordinal 顺序；Kappa 支持 Cohen（含线性/二次加权）和 Fleiss，ICC 显式选择六种常用模型/测量定义，rwg 显式选择均匀或指定方差的零假设。秩检验自动在不超过 9 个观测时使用精确位置置换，否则采用渐近方法；Bland–Altman 统计量用完整样本，展示点最多 2000 个。方法定义、样本要求与推断限制由 20 篇中英文节点帮助维护，不另建专用报告页面。
工作流模板、模型预设、结果指标、统计专用绘图和原理说明也按本轮要求登记入口；该登记不表示模板执行、参数预设或绘图能力已经实现。普通数据处理、缺失数据处理、通用绘图和 AI 模块未纳入本轮统计入口。
实现某项方法时直接完善其既有定义及内核绑定；只有存在可复用模型时才提供 Fit/Summary 分工，不从入口名称推断或批量生成端口契约。

`statistics/regression_models.rs` 完善“回归模型”中的 31 个原占位入口，包括
稳健、岭/Lasso、单响应 PLS、曲线/非线性及自定义公式、离散/计数/比例模型、
GLM、Deming、分位数、单门槛和 RCS 分析。分层、逐步、单因素与多因素、分组、
基准入口分别落实为分块 OLS、AIC/BIC 选择和明确的 OLS 批量分析；不推定多层模型
或抽象工作流引擎。全部唯一输出为结构化 `result`，复用 Inspect 数值/报告切换。
方法身份、中英文标题和搜索别名来自同一静态声明，节点 ID 按该方法身份构造。
RCS 接收响应和单个自变量，结果保留节点及数值基函数设计并给出 OLS 拟合。
分类响应保留原始标签和概率，有序 Logit 遵守显式 Ordinal 顺序；条件 Logit 为
分层二元定义。GLM 只声明实际支持的分布/链接组合，条件参数进入真实计算。
62 篇中英文帮助维护参数、模型公式、系数推断和当前边界；惩罚模型不伪造常规
系数显著性，非线性活动边界不提供无约束协方差，选择后的推断按条件结果解释。

“运算”下按“算术”“逻辑”“转换”排列；“转换”提供 To Numeric、To Text、To Categorical、To Ordinal、To Binary、To Datetime、To Identifier 七个固定目标节点，保留输入的标量或数列结构。数值节点仅配置数值表示，分类/顺序/二元节点仅配置各自值域，日期时间节点配置形式、精度和格式，文本与标识节点无参数。目标由节点声明确定，不提供目标选择或按下游推断目标的 Auto 模式。分类、名称、参数及中英文帮助由 Rust 目录统一提供。
To Categorical 的取值与标签默认为空：优先继承已有值域，否则在执行时按完整输入的非空原值去重，自动生成同名标签。分类 Data Labels 复用此规则；显式值域仍严格校验，Ordinal 仍要求明确等级。自动映射保留原始编码、空值与数列对齐，不生成类别编号，也不修改源数据库或图参数。

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
数据序列节点均已注册执行内核。选列、数列变换、虚拟变量信息及整数范围均返回惰性数列句柄，使用 DataFusion 投影、窗口或 range/unnest 计划；同一行域可保留惰性执行；独立来源或混合数列在需要时受控物化并按当前位置配对。长度、非空计数、求和、均值及标准化的标量统计输出消费原生聚合的一行结果。文档中的数列常量一次性导入查询引擎，以显式位置坐标保持等长常量数列原有的逐元素语义；数据帧和数据集不通过长度建立对齐证明。
图编辑和分析不扫描数据。整数范围包含起点、不包含终点，支持非零负步长。
长度包含 Null，非空计数排除 Null，求和忽略 Null 且空输入返回零，均值无有效值时返回 Null。
标准化使用样本标准差（ddof = 1），输出标准化值、均值及标准差；无有效方差时报错。
关系数列的标准化与逆标准化支持 Null 保留且可与原数列比较。浮点往返校验使用“等于”的容差模式，
默认绝对容差 1e-12、相对容差 1e-9，可在 Detail 配置中修改；默认精确模式不保证浮点往返全为真。
时序节点默认使用当前行顺序；高阶差分表达重复一阶差分，变化率采用比例值（0.1 表示 10%）。
滚动节点支持 mean/sum/min/max/std，默认均值及完整非空窗口；`min_periods` 为零时要求完整窗口，否则按指定有效值数计算。滞后节点支持 lag/lead 并保留类型与元数据。二者可连接等长的可选上下文，按当前位置对应后选择分组列和排序列；窗口结果恢复原始行顺序。面板差分通过实体列和时间列选项指定分组上下文，
不要求上游先执行面板对齐；结果恢复原始行顺序。虚拟变量信息只附加参照组提示，不生成指示列。
虚拟变量信息的输入限定为 Categorical、Ordinal 或 Binary 数列；Graph 在执行前阻断其余语义，
输出通过类型恒等规则保留输入的精确元素类型，不成为三种类型的未定联合。

时间序列和面板对齐均接收一个 DataFrame，通过 time_column、entity_column（仅面板）及正整数 interval 配置。输出保留列顺序、类型和元数据，新增非键单元格为 Null；重复键、缺失键、偏离网格或超出内存预算会失败。面板网格使用共享已观测时间的位置，仅补齐各实体自己的起止范围，详见节点帮助。
数据帧的筛选入口为“筛选行”（配置列条件）和“筛选列”（选择并排列列）。
“移除行”复用行条件配置，仅删除条件为真的行，保留条件为假或 NULL 的行；“为空”条件可显式移除缺失值。“移除列”复用列选择器，删除指定列并保留其余列的顺序、类型与血缘；至少选择一个现有列且至少保留一列。两个节点均产生新的计算结果，不修改源数据集。
数据帧组合分为“按行拼接”“按列拼接”“连接”和“组装数据帧”。按行拼接支持列名/位置对齐；连接支持多列键及 inner/left/right/full/semi/anti，半连接和反连接仅返回左表列且保留左表顺序；组装允许混合语义及来源的数列，要求等长并按当前位置逐项组装。字段命名与输入顺序由 Graph 的组合 Schema resolver 推导，执行由 Kernel 经关系契约交给 DataFusion。

`src/dataframe/transforms.rs` 注册排序、行去重、添加或替换列、按数列筛选行、转长表、转宽表、时间重采样，以及条件选择、缺失判断、填充、值映射、文本处理、日期处理、数值限制、分箱、累计、排名和前后向填充。全部接入执行内核，并提供中英文帮助。普通取值列表和数值边界列表在 Details 中逐项编辑，列参数复用 Rust 投影的列选择器。
排序与去重保留同值行的源顺序；添加或替换列、条件选择、掩码和窗口上下文支持按位置接收独立来源或内存计算结果；输入长度须匹配，保留既有值语义及分组/排序规则。转长表要求取值列物理类型及语义兼容；透视及虚拟变量生成显式声明类别值与输出列名，不扫描数据发现列。Graph 的 `schema.transform` 解析固定输出字段、名称冲突与选择列诊断。限制行同时支持非负 `offset`，窗口表达式先计算再截取，分页不改变窗口结果。

数据序列提供“频数”，数据帧提供“分组聚合”；统一的“数据描述”位于“统计 → 描述统计”，由 `statistics/descriptive.rs` 声明 `yssbi.statistics.describe`。其 `source` 输入接受数据帧或数据序列，`result` 输出 `statistics.report` 结构化结果，`columns` 对象以原始列名为键保存各列统计字段，`position` 从 1 开始记录受支持列的输入顺序；已移除两个分立的描述节点。
数据描述支持 Numeric、Categorical、Ordinal、Binary；后三者采用分类摘要，整数编码不改变语义。Text、Datetime、Identifier 不自动作为类别；数据描述节点无需配置参数：数据帧按原列顺序统计全部受支持的列，数据序列直接统计自身。
数值摘要为有效数、缺失数、均值、样本标准差（ddof=1）、最小值、q25、中位数、q75、最大值；分位数在 `(n-1)p` 位置线性插值。
分类摘要为有效数、缺失数、不同取值数，并在 `categories` 中直接返回每个已出现的非空类别的原值、已声明标签、频数和有效样本占比。类别对象以从 1 开始的连续编号为键，Ordinal 按声明等级排列，其余按原值升序；宽整数原值在 JSON 中保留十进制文本。每列只返回对应语义的字段，不适用字段不添加；适用但未定义的指标为 Null。全空分类列返回空类别对象和类别数 0。
频数输出原类型的 `value`、`frequency`、`proportion`，不截断为 Top N；默认包含 Null，关闭时同时排除空值组及其分母贡献。Ordinal 按声明等级排序，其余按原值升序，Null 最后；空字符串保留。
GroupBy 必选一个或多个分组键，始终输出 `row_count`，并可分别选择 count/sum/mean/min/max/std/median 的列。同一列可参加多种聚合，输出为 `列名_操作`；名称冲突拒绝执行。count 排除 Null，其他聚合只接受 Numeric、忽略 Null，全空数值组返回 Null；Null 分组键保留。
数值统计要求无损提升为 Float64，非有限输入、溢出或非有限结果会报错，不转换为成功空值；数据描述节点在执行时完成统计并交付错误。选择器的可选列、参数诊断与输出 Schema 都由 Graph 的 Rust 解析产生；编辑阶段不扫描数据。

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

## 计量与因果分析节点

`statistics/causal_models.rs` 将该类别余下 13 个既有 ID 接入真实内核：线性 IV GMM、
锐断点 RDD、PSM、Heckman 两步法、组间处理效应异质性、半正态 SFA、SUR、IPW、RA、
AIPW、ATE、ATT 和合成控制。原有 IV/DID 入口继续使用其方法族。
四种处理效应估计器返回 `statistics.result.treatment_effect`；ATE/ATT 仅从该类型提取
对应目标量及已有推断，不从原始均值构造因果结论。其他入口返回单个结构化 `result`，
通过通用 Inspect 查看。SUR 用参数中的一基索引列表指定各方程的自变量。
输入行对齐、选择样本空值、估计范围、默认值与推断限制由配套中英文 `causal_*.md` 帮助说明。
重复输入没有固定数量上限，仍受可识别条件与执行预算约束。

## 参数声明

动态节点 ID 和本地化 key 使用拥有型字符串，由装配片段、协议和目录持有并随其释放。
语义 ID 的 JSON 读取复用其构造校验，图文件与类型化 IPC 共用同一入口，合法 wire 仍为字符串。
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

Protocol 对 DataSeries 参数逐项检查声明的元素类型。Graph Editor 创建和修改允许仍缺少必填字段，
但拒绝已提供值的类型、固定约束和 nominal codec 错误；Analysis 和执行仍检查完整性。
单列名及有序列列表分别声明 `ColumnName` / `ColumnNames` 约束；列表名称必须有效且唯一。
可空列表与必选的 ProjectColumns 共用协议层列名检查，图分析复用同一解析，不依赖实际表判断格式是否合法。

Classical tests in `statistics.tests` are executable catalog nodes. Their stable IDs and ports are assembled in `statistics/classical.rs`, and Rust kernels are registered in `yss-node-kernel`. Each of these 30 nodes has its own Chinese and English `test_*.md` help page, selected by `src/documentation.rs`, with its inputs, parameters, hypotheses, statistic, reference distribution, outputs and current usage limits. The existing normality node retains its own help page. Treatment-effect `heterogeneity` is implemented by `statistics/causal_models.rs`; Kappa and Kendall W are implemented separately under `statistics.association`. `t.summary_input` accepts `[n, mean, sd]` for one-sample or paired summaries and six values for independent groups; its `design` parameter determines the interpretation.

`statistics/path` declares continuous interaction, single-mediator and recursive observed
path interfaces. Domain-specific files share node assembly and parameter localization.
Help specifies equation syntax, percentile inference, probing, extrapolation and the
distinction between observed recursive OLS and latent-variable SEM.

`statistics/survey` declares weight validation/inversion, three mean/proportion design
interfaces, and Gaussian/logit/Poisson survey regressions. Optional single stratum/PSU
ports state the design explicitly; these weights are not precision or treatment weights.
Bilingual help documents with-replacement variance, lonely strata and degrees of freedom.

`statistics/power` exposes scalar planning parameters and bilingual model-specific help.
The principles entry has a concrete known-variance normal design; generalized-model power
explicitly means two-group Poisson rate planning. Help distinguishes exact distribution
calculations, approximations, per-group/pair/cluster units and unsupported extensions.

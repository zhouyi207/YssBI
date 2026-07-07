# 在正式打包分发前，可以等渲染完毕再显示窗口
# 在目前的开发环境中，不要这样做，可以取消 debug
# 项目未发布 不做任何迁移处理

每次更新版本都需要

由于历史代码重构原因，目前项目中存在许多的历史遗留代码，或多余或逻辑重复或实现低效；请检查整体项目，寻找出项目中的重复逻辑和未使用的逻辑，分析必要性，如果有更高效的更干净的架构请添加到 todo 的 v1.0 待办中，如果单纯的逻辑重复或者多余，也请添加到 v1.0 待办中

```
同步改四处版本（例如 0.1.1）：

src-tauri/Cargo.toml
src-tauri/tauri.conf.json
package.json
src/app/appConfig/appLinks.ts


提交并 push，或手动再跑 publish.yml
```

# DOLIST

## 2026.02.27

- [x] 竞态条件导致的，应该使用 CQRS 架构，避免直接获取数据：~~打包后 addGraph: Graph "8c010862-6023-4fb5-96f7-ea4ebdfecfc0" already exists 还是会出现，说明在dev阶段就调用了两次~~
- [x] 在 dataseries 结构体中添加了新的字段来显示 dummy 信息，~~同时 ols 在这里并没有处理 string 类型变量，string 类型变量应该 dummy 后然后去剔除一个设置哑变量参与到回归中去，同时在这里需要备注 dataseries 剔除哪个哑变量，以及其属性是否是 时间属性，如果是时间属性的话那么公式中应该要用 t 表示；~~
- [x] 之前是使用 draggedPin.type 这个残缺的来获取 datatype 的，现在使用了 typedisplay 来精确重建 ~~ouput pin 拉出后出现的节点菜单是正常的，input pin 拉出后出现的节点菜单好像有点不正常~~
- [x] 都使用了 Input 框来显示，并对数值类型和字符串类型做了一些限制，~~pin input 需要处理 还有常数节点~~

## 2026.02.28

- [x] ~~ols 功能我希望分为两点，一个是 ols，一个是 ols_summary；ols_summary 是对 ols 能做出的所有报告的总结；~~
- [x] ~~ols 只返回 ols model, 而 ols_summary 返回 ols result, ols result 包含了 ols~~
- [x] 在这里添加了四个库 katex react-markdown rehype-katex remark-math @types/katex ~~ols summary view 可以添加一个公式~~
- [x] 在这里添加了两个库 @types/d3 d3 ~~ols summary view 中再添加一个残差图呗，就简单的 scatter 图就好了使用 d3 吧，拓展性太强了比 chartjs 要好很多~~
- [x] 修复了测试报错情况，~~测试目前由于 execution::new() 修改了导致测试报错~~
- [x] 获取 unique name 迁移到后端，~~在快速创建的时候，可能由于后端的数据并没有反应到前端，导致前端数据不及时从而创建两个相同的 event~~
- [x] 给 dataviewWindow 添加了 column stats 和 overview ，并使用了直方图和条形图进行处理
- [x] 后端依旧使用 path 作为 title，已修复， ~~editorviewer 导入 database 后 title 和 dataviewer 不同步~~
- [x] 给 editorviewer 的 data sidebar 中的 database 添加一个 eye 图标，点击就可以打开 data viewer
- [x] 修复 dataviewer 数据表格的显示问题，可以显示足够多的列，使用 react-virtual 行也虚拟化了 ~~无法显示足够多的列~~

## 2026.03.03

- [x] 添加 plot 支持
- [x] 添加 随机分布 支持 ~~随机分布的随机样本，设置数量出现随机样本~~
- [x] 添加对 dataseries 运算符的支持（蒙特卡洛模拟） ~~catelog math 节点应该是 any~~

## 2026.03.04

- [x] 添加假设检验 ast 转化公式文本
- [x] 动态节点生成 pin，~~动态节点生成 pin 应该怎么操作？使用了一个很蠢的方法调用 extract_schema_from_node_input_connections 函数完成~~

## 2026.03.05

- [x] 添加 vce 配置的实现
- [x] 添加 wls, gls 的实现
- [x] 添加 bp ，添加 lmtest 来检验异方差，不过实现得稀烂

## 2026.03.06

- [x] 添加了模块时间
- [x] 修真了 aic, bic 的计算，~~aic, bic 计算不一致~~
- [ ] gls 所有结果都没有被检验
- [x] 加上 aic, bic, Omnibus, Durbin-Watson, Jarque-Bera 就完成了 statsmodel 的 ols summary 了
- [x] 时间序列算子设计问题
- [x] string→categorical：DataView 编辑后通过 DuckDB `save_changes` 持久化改后数据即可，无需 database 映射层；图内按需转换仍用 `String to Categorical` 节点，string→categorical：DataView cast 后 DuckDB ENUM 持久化，重开恢复 Categorical；图内按需仍可用 `String to Categorical` 节点  ~~目前暂时使用 string to categorical 的节点形式展现出来了，但是认为可能需要在 database 层面添加一层映射层将数据进行持久性转化，可以保证项目在加载的时候 database 可以恢复为之前的形式。但是如果对数据进行修改了呢？？难说 由于添加了 categorical 之后，需要将 string 类型转化未 categorical 类型，但是这些在 dataview 中的操作，在保存项目并恢复的时候并没有恢复，这就导致之前是 categorical 的类型变成了 string 类型但是却在 ui 上并没有体现出来~~
- [x] 创建一个 string to categorical 节点

## 2026.03.07
- [x] ~~关于动态和静态节点，我认为对数据处理操作都是可以预测的，其生成的形状和 pin 都是可以知道的，不需要计算，因此在数据处理层面我认为可以使用静态节点也应该使用静态节点；对于 predict 节点，其 model 的传入有两种方式，一种是自己配置另一种是连线，连线的 model 必然是 output 节点，那么其在形式上必然有 pin 的生成，我可以使用其上一个节点的 pin 来生成这里的 pin，一种是自己配置的，那么其在连接线的时候必然要解析这里的 model 可以动态生成节点；因此，动态节点在某种程度上必然是不现实的，其会造成卡顿等等一系列的问题？？又或者说在计算的时候对于 data pin 来一个即时使计算，而对于 exec pin 同时在前端出现等待样式；这样的话好像在打开项目节点的时候会很卡顿，不应该这样操作。**既然都可以预测，那么解决问题的最好的方法就是在流动的过程中添加信息层，这里的信息层取决于连接了什么？？？也就是在每次连接的时候进行链式更新，即一个信息的传输作用。例如 ts align 节点，其输入 dataframe 会传入一个 schema 信息给 ts align，在连接的时候其 output dataframe 就会拥有这个信息 schema，以便于 decompose dataframe 在连接 output dataframe 的时候会自动生成 output pin**~~
- [x] Schema 与 Pin 解析架构原则（结构 / 信息 / 数据三层；connect 时链式传播 schema；schema 派生 pin 非 exec 动态）→ 见 [DESIGN_RULE.md §3.7](./docs/DESIGN_RULE.md#37-schema-与-pin-解析)
- [x] `load_graph` 打开项目：保留 `propagate_schemas`，将全图 `resolve_all_dynamic_pins` 改为延迟/分帧物化（打开 tab 或视口内节点），避免大图首屏卡顿
- [x] 审查各 dataframe / predict 节点是否均已注册 `output_schema_resolver` / `pin_resolver`，与 §3.7 一致（如 TS Align 输出 schema 是否完整）
- [x] 完成 ols_summary 和 wls_summary 的测试编写方便后续重构检验正确性
- [x] 完成 ols, wls, gls 的 dropna

## 2026.03.08

- [ ] 我感觉可以借鉴 ue 中的 compile 形式，完成 ipynb 形式，即生成时编译运行并缓存结果，比如常见的 decompose dataframe 节点，我可以将其在ui上设置一个状态表示需要编译才会生成 data output pin
- [ ] 我感觉像 ols, wls, gls summary 中缺失值不应该直接 dropna 处理就没了，可能输出的序列中对 dropna 位置的值添加 nan 信息更好一些，例如残差 [nan, 1, 2, nan, ...] 这种形式感觉要很好一些
- [ ] 目前处理并没有考虑缺失值的情况，在生成自相关检验的时候应该考虑缺失值并给出一个缺失值警告
- [x] 创建一个生成 1..n 的 int 64 dataseries 的节点，该节点有一个起始值和一个长度还有 col name 三个 pin 组成，output 是一个 int64 类型的 dataseries pin
- [ ] 类型限制问题，例如 ts lag 中的 time series 接受 int64, date, 那么其输出的 time series 应该也是其中的一个而不是 oneof，不过好像 one of 好像也没有什么不好？？
- [x] 完成 acf, pacf 以及 dw 检验，gb 检验，还有 q 检验
- [x] 完成 newey - west 回归，使用 vce::newey 配置
- [ ] ivreg2 其实现是使用 z 统计量而不是 t 统计量，因此在这里 hac 的实现是由问题的
- [ ] 在考虑要不要将 ols, wls, gls 合并为 regression 然后在 configure 中配置

## 2026.03.09

**Stata 手册，全部基于 ρ 变换后的变量和残差 计算，而不是原始 y 和原始残差**

- [x] 完成 prais y x1 x2, nolog 和 prais y x1 x2, corc 两个命令
- [x] prais summary 组件搭配，imtest还一系列的自相关检验都可以删掉了
- [x] reset 检验
- [x] 多重共线性检验
- [x] 杠杆作用
- [ ] chow 检验，这个直接虚拟变量实现就好，目前已经实现，但是好像单独提取一个节点实现更为方便 ？？
- [ ] 线性插值，插值部分后续还会有很多，后续进行批量添加
- [x] 创建一个节点，该节点可以将 dataseries 组合成一个 dataframe，与 decompose dataframe 相反

## 2026.03.10

- [x] compute_output_schema_for_node_inner 能不能使节点自己完成这个功能

## 2026.03.11

- [x] 2sls
- [x] liml
- [ ] 前端结果展示中 公式展现形式太差了

## 2026.03.12

- [x] logit
- [x] probit
- [x] 二值选择模型的公式修正
- [x] logit 添加 adds ratio 并加上了 tooltip
- [x] margin
- [ ] 多值选择
- [ ] 计数数据
- [ ] 排序数据
- [ ] 断尾回归，归并回归，样本选择模型

## 2026.03.13

- [x] 面板数据形式设计
- [x] 个体固定效应模型，时间固定效应模型，双向固定效应模型
- [ ] DID
- [x] 面板是不是也要加 xt align 进行对齐
- [ ] 获取输出的 p 值不使用四舍五入而是使用截断会导致变小 那么更适合通过检验
- [ ] 目前好多地方的 F 检验并没有展示其自由度 F-statistic，应该是 F(1, 26) 这样的形式才对
- [ ] 严格多重共线性，需要一种处理范式
- [ ] 目前感觉系数不太准确，但是很接近，但是系数的 std err 相差很多；同时 LR test of sigma_u=0: chibar2(01) 相差很近，Log likelihood 相差很近；LR chi2(9) 相差有点多，同时 mle 并没有标识残差服从什么分布，请仔细分析 stata 的实现并修正这里的错误
- [ ] 在这里添加了置信区间的相关内容的限制，在我看来有这个限制可能会抑制错误的检查，我认为不要可能要好一些

## 2026.03.14

- [ ] 面板 iv
- [ ] 双向随机效应模型没有验证，stata 好像无法实现

## 2026.03.16

- [ ] polars 条件过滤
- [ ] polars 平方，对数，指数
- [ ] 图形处理
- [ ] panel Effect Type (Entity / Time / Two-Way) 未验证
- [x] theta min, theta avg, theta max 可以使用一个结构体包裹，同样的还有 obs_per_group_min, obs_per_group_avg, obs_per_group_max；sigma_u, sigma_e, rho; 这些字段


## 2026.03.17

- [ ] 拉取数据库我希望是懒加载同时加上数据懒式加载
- [x] 目前只加了 line ~~对 plot 组件添加工具栏，可以在数据生成之后继续对图形进行操作~~: 
- [x] 一元数学函数节点：ln, log2, log10, exp, sqrt, square：~~添加 ln log2 log10 math 组件可以将 dataseires 求对数~~
- [ ] 添加了 correlogram 节点
- [ ] 解决了默认值问题，使用 default_value 解决的，但是我感觉好像有问题，应该使用 user_value，如果 user_value 为 None，那么就显示 default_value
- [ ] 创建一个 arima 节点，其有 dataseries input pin, p value input pin, d value input pin, q value input pin，这里和 ols 节点一样输出模型结果；然后创建一个 arima summary 节点，其相较于之前多了一个 dataseires(boolean) input pin 的 condition 节点；主要的作用是将符合 condition 的数据类似于做训练集，然后去预测测试集的数据并作对比，请实现

## 2026.03.18

- [ ] ARIMA 暂时搁置
- [ ] 给 polars 添加条件 filter，分为 dataview 和图节点的形式，在这里先处理图节点形式
- [ ] 每打开一个新窗口就会在 WindowDataStore 中添加一个字段，我需要关闭窗口时删掉这个字段并释放内存
- [x] 修复 ols summary 中 robust 后 F 统计量没有发生变化的问题
- [ ] AR ADL&ARDL 模型添加这里好像都可以使用 ols 解决，因此他们都说没必要 我主要是想将 AR 放置到 arima 中，但是好像其实现方法不一样；
- [ ] ECM 误差修正模型好像可以和 ADL ARDL 结合在一起，不过前提是需要 y 和 x 的均衡关系
- [x] 独立出来 CoefficientsBlock.tsx 来包裹 bar chart 以及 CoefficientTable 
- [ ] CoefficientsBlock.tsx 来包裹 bar chart 以及 CoefficientTable 感觉这样实现也不太好
- [x] 添加 VAR 向量自回归模型，并添加脉冲响应函数 IRF
- [ ] 目前 VAR 向量自回归模型的 IRF 置信区间算不出来
- [ ] 时间趋势项和季节调整

## 2026.03.19

- [ ] 随机模拟的例子，可能需要用到函数，我觉得这一步应该是很重要的，这对于框架的完整性而言
- [x] DF & ADF 单位根检验，添加了单个和 summary 节点
- [x] 接下来我需要处理协整节点
- [ ] 目前 vecm 中的 Equation Summary 中的 R-sq 数值和 stata 对不上，不知道 stata 的这个 R-sq 是怎么计算的
- [ ] EG-ADF 检验未完成
- [x] 检验 var 模型的滞后阶数 varsoc x y z ..... 目前不是完整命令
- [ ] 检验协整秩 vecrank x y z, lags(#) max trend(none) trend(trend)
- [ ] vecm 就是利用上面这两玩意确认阶数和秩

## 2026.03.20

- [x] did 平行趋势检验 安慰剂检验
- [ ] did 平行趋势检验 安慰剂检验 并没有完成检验
- [x] 创建 float64 -> int64, int64 -> categorical，目前已经有了 categorical
- [ ] 在按下按钮涉及到大量计算的时候，页面会卡死，我认为应该在后端开启一个线程计算还是怎么样，至少在点击其他东西的情况下，禁止某些功能可以查阅其他信息
- [x] 类型收敛，使用 Int64 和 float64 代替所有的 number（详见 `## 2026.07.03`）

## 2026.04.28

- [x] 重构界面，使用了 shadcn 组件来完成

## 2026.04.29

- [x] 添加 i18n 翻译
- [x] 添加 zustand 存储

## 2026.04.30

- [x] 重构 dataView 界面
- [x] 添加了 项目管理 页面
- [x] 数据视图表格切换为 https://grid.glideapps.com/ 的逻辑，行号可以选择出现选择框

## 2026.05.06

- [x] 修复 dataView 界面
- [x] 修复多窗口主题变化时其他窗口没有反应
- [ ] Residuals vs Fitted 感觉可以切换数值显示为进度条

## 2026.05.07

- [x] 记录的窗口大小和位置，下次打开会还原大小和位置
- [x] 还原之前窗口就已经打开了，进而又会恢复，这会导致卡一下，因此在恢复之前设置 hidden 在完毕之后再显示
- [x] 在项目选择界面点击进入项目时，出现一个进度条表示目前正在加载的东西
- [x] 数据库加载使用后台异步加载方式，项目打开后立即进入编辑器，后台线程逐个 build lazy 源，完成后事件推送更新前端 schema

## 2026.05.13

- [x] 修复右键菜单 i18n 没有处理的问题

## 2026.05.20

- [x] 我认为需要仔细考虑一下数据存储的问题，初步想法是使用 duckdb 按列存储在文件目录内，具体方式是在导入数据的时候，将数据尽可能的存储在当前文件夹下的 database (如果有) 中，在这里可能需要分为两种情况，一、文件类型数据：对于这种数据可以按照上述方式处理，转存为一种新的格式到本地来读取；二、数据库数据：对于这种数据直接按照数据库的方式处理就好了
- [ ] 对于图像等处理，我更希望使用一个 sidebar 的方式，在这里我们可以选取我们需要的数据列以及图标类型来操作图像显示，这样就类似于 tableau 了
- [ ] 架构，架构，架构

## 2026.06.23

**背景**：项目是列式分析（Decompose → Series → OLS 等），不需要整表全列进内存。当前 CSV 路径在 `Execution` 时会 `lazy.collect()` 整表物化，与列分析模型不匹配。目标：**DuckDB 作项目内列存 + 按列/按页 I/O，Polars 仍作 yss-sci / 图节点计算层**（承接 2026.05.20 的 DuckDB 设想）。

**目标架构**：**DuckDB = SQL / 存储引擎**，**Polars = 数据处理 / 建模引擎**，中间以 **Arrow 零拷贝** 互转；所有文件型数据源（CSV、Excel、Parquet 等）统一 ingest 为项目内 `.duckdb` 列存，运行时不再直接依赖原始文件路径读取。

### Phase 1 — 导入 ingest（基础设施）

- [x] 添加 `duckdb` Rust 依赖（桌面嵌入，`bundled` 特性）
- [x] 定义 `DatabaseEngine::DuckDb { path, table }` 及前后端 DTO / `LoadDatabaseEngineSpec` 同步
- [x] 导入 CSV 时 ingest 到 `{project}/database/project.duckdb` 内新表（`db-{uuid}`），不再以 Polars `LazyCsvReader` 作为主路径
- [x] 导入 Parquet 时同样 ingest 到 `project.duckdb` 新表，不再以 Polars `scan_parquet` Lazy 作为主路径
- [x] `load_database` 返回元数据（id / name / rowCount / columns），不整表进内存
- [x] 项目 save/load 包含 `database/` 目录；reopen 扫描 `database/*.duckdb` 绑定实例（Phase 4.5 起不再在 manifest 写 catalog）
- [x] 新建项目时自动创建 `{project}/database/` 目录

### Phase 2 — 读路径替换（元数据 / DataView）

- [x] 新增 `DatabaseState::DuckDb { duckdb_path, table, row_count, columns }`（Phase 1 已落地；Execution 仍会物化到 `Loaded`）
- [x] `get_database_meta` 对 DuckDB 数据源读缓存元数据，不再 Preview collect
- [x] `build_schema_provider` 改为 `DatabaseInstance::data_schema()`，DuckDB 读缓存列元数据，不再 `ensure_loaded()`
- [x] `get_database_rows` 改为 `DatabaseInstance::query_page()`（DuckDB `LIMIT/OFFSET`），分页不触发整表物化
- [x] `count_lazy_rows` / `load_database_direct` / `DatabaseState::Lazy`·`Pending` 已移除；`load_database` 仅 DuckDB ingest，`set_data` 仅绑定 DuckDb
- [x] 项目 reopen 时枚举 `project.duckdb` 内用户表并绑定（`discover_databases_from_root` + `bind_duckdb_instance`；集成测试已覆盖）

### Phase 3 — 图执行列裁剪（核心收益）

- [x] `DatabaseInstance::load_columns(&[&str])`：DuckDB `SELECT col1, col2, ...` → Polars DataFrame（当前仍经临时 Parquet 中转，Arrow 零拷贝待 Phase 5）
- [x] `Decompose DataFrame` 改为按列 `load_database_series`，不再 `get_dataframe` 整表后遍历全部列
- [x] `Get DataSeries` 改为按列 `load_database_series`，不再整表加载
- [x] `graph_runtime.get_dataframe` 保留整表路径并注明仅用于 Filter 等确需整表的节点；按列分析走 `load_database_series`
- [x] 图执行缓存（`data_store`）按 `{db_id}::{column}` 缓存 Series，避免重复 I/O

### Phase 4 — 统计与大表

- [x] `get_column_stats` / `get_column_distribution` / `get_dataset_overview` 优先 DuckDB 按列聚合，按需拉单列进 Polars（`duckdb_analytics.rs`；DuckDB 路径不 `ensure_loaded`）
- [x] 评估超内存场景：DuckDB 列存 + SQL 聚合/spill 承担大表 I/O；`duplicated_rows` 在 DuckDB 路径暂跳过全表 DISTINCT（返回 0）；内存编辑仍走 `Loaded` 小表路径
- [x] **Parquet 导入 ingest 到 DuckDB**：`load_database(Parquet)` → `read_parquet` 写入项目 `.duckdb`；`DatabaseDecl.engine` 持久化为 `DuckDb` + 原始 Parquet 路径溯源；移除 `DatabaseState::Lazy` Parquet 主路径
- [x] Excel 等其余文件型数据源 ingest 到 DuckDB（`DuckDbSource::Excel` + calamine → 临时 Parquet → DuckDB；与 CSV / Parquet 统一）

### Phase 4.5 — 数据集目录即持久化（无 JSON catalog）

- [x] **`metadata.yssbi` 不再持久化 `databases`**：manifest 仅保留 `projectName` / `appVersion` / `exportTime`
- [x] **移除 `DuckDbSource` / `engine.source`**：显示名写入 DuckDB `_yssbi_meta`（按 `table_id`），reopen 从 meta 读取
- [x] **导入 / 删除 / save**：save 不写 catalog JSON；delete 删表不写 manifest
- [x] **单文件 DuckDB**：每个项目仅 `{project}/database/project.duckdb`；每个导入数据集 = 库内一张表（`table` = `db-{uuid}`）；reopen 枚举库内用户表（排除 `_yssbi_meta`）

### Phase 5 — 编辑与导出语义

- [x] **`save_database_changes`**：DuckDB 数据集将内存编辑写回 `project.duckdb`（`ingest_dataframe_to_duckdb` 替换表），保存后回到 `DatabaseState::DuckDb`；`export_database` 仍导出到用户指定的外部 CSV/Parquet（含未保存的内存编辑，不写回项目库）
- [x] **远程 SQL 源（SQLite/PG/MySQL）**：导入时 snapshot ingest 到 `project.duckdb`（与 CSV/Excel 一致），reopen 通过表枚举恢复；不再依赖 session-only `Lazy` + `Pending` 物化路径

### 架构约束（实施时遵守）

- [x] **DuckDB（SQL / 存储）→ Arrow C Data Interface → Polars（数据处理）**：`query_to_dataframe` 走 `query_arrow` + FFI 导入 Polars，无临时 Parquet 读路径；写入 ingest 仍经临时 Parquet（`ingest_dataframe_to_duckdb`）
- [x] **文件型数据统一 DuckDB 存储**：CSV、Parquet、**Excel** ingest 到项目内 **`database/project.duckdb`**（每数据集一张表；manifest 不写 catalog）
- [x] **不**把 OLS/面板/时序等迁到 DuckDB SQL 内计算；DuckDB 只负责 I/O 与列裁剪（yss-sci 回归仍在 Polars/Rust 侧）
- [x] **不**引入 ClickHouse 作为默认桌面导入链路
- [x] 前端 `DatabaseService` API 尽量保持稳定；变更集中在 Rust `database/` 模块

## 2026.06.24

- [x] 给 sidebar 为数据的情况下，左侧数据表添加右键菜单；右键菜单格式与图的右键菜单保持一致
- [x] 移除 Lazy / Pending 遗留路径（`count_lazy_rows`、`load_database_direct`、`build_lazy`、`is_lazy_friendly`、`DatabaseState::Lazy` / `Pending`；`set_data` 仅绑定 DuckDb）
- [x] 节点 node 和 pin 的右键菜单功能
- [x] repeatable pin「移除 Pin」：前端 `PinSlotDTO` 与持久化 `PinSlot` 分离（camelCase 字段供前端，`snake_case` 不破坏项目反序列化）
- [x] repeatable pin 删除后重命名同步：`remove_repeatable_pin` 重索引后通过 `NodePinsUpdated.updatedPins` 下发，前端更新 pin 名称（修复 Add/OLS 等删除中间 pin 后再添加出现 A C C / X 1 X 3 X 3 等问题）
- [x] 配置 GitHub Actions 发布流水线（`.github/workflows/publish.yml`，推 `release` 分支或手动触发）
- [x] Polars → DuckDB 写入统一为 Arrow：`ingest_dataframe_to_duckdb` 去掉临时 Parquet，改为 Arrow C Data Interface + DuckDB Appender（`appender-arrow`）；读侧仍为 `query_arrow` → Polars；外部 CSV/Parquet 文件导入路径不变
- [x] DataView String→Categorical 保存时写入 DuckDB ENUM（`_yssbi_enum_*`），重开 schema 与数据均恢复为 Categorical，不再降级为 String
- [x] DuckDB ENUM 读写类型映射文档：`src-tauri/src/database/README.md`；写侧 Appender 仅接受 Utf8（Categorical 经 String 桥接），读侧 `query_arrow` 为 `Dictionary(UInt8, Utf8)`；spike 测试 `duckdb_enum_*`

### Phase 6 — 大表内存边界（DuckDB SQL 编辑，避免整表 Loaded）

- [x] DataView 行定位改用 DuckDB `rowid` 伪列；ingest 不再写入物理内部行键列
- [x] DataView 分页返回 `rowIds`；`edit_cell` / `delete_rows` 走 DuckDB SQL，不再 `ensure_loaded`
- [x] `DatabaseState::DuckDb` 挂 `EditHistory`；`save_changes` 大表仅刷新元数据 + 清历史，不全量 `ingest`
- [x] 小表（≤ `MAX_IN_MEMORY_EDIT_ROWS` = 50_000）保留 `Loaded` 路径（cast / 复杂 schema 编辑）
- [x] `ingest_dataframe_to_duckdb` 分 batch append（`INGEST_CHUNK_ROWS` = 50_000），降低保存峰值
- [x] `get_dataframe` 超 `MAX_GET_DATAFRAME_ROWS`（500_000）拒绝整表；图节点继续优先 `load_database_series`
- [x] Excel 导入改道：calamine → 临时 CSV → DuckDB `read_csv`（不经 Polars 全量）
- [x] `database/README.md` 补充内存边界说明；集成测试 `test_duckdb_sql_edit_without_full_load`

## 2026.06.25

- [x] light 模式下 exec pin 连接状态去除中心白点，与 dark 模式一致（`Pin.tsx`）
- [x] light 模式下 Summary Equation KaTeX 文字过浅：FormulaBlock 等改为 `[&_.katex]:text-foreground`
- [x] light 模式主题兼容补全：`App.css` 覆盖 hover 变体、语义 accent 色、tooltip、`[&_.katex]` 兜底、分隔线 `bg-gray-700/800`
- [x] 图表主题：`shared/theme/chartTheme.ts` + PlotView / InfoView D3 轴网格画布随 light/dark 重绘（12 个图表）
- [x] InfoView 旧深色类名迁移为 shadcn token（`bg-card` / `bg-muted` / `text-foreground` / `border-border` 等；新增 `shared/infoViewTheme.ts`）
- [x] 窗口控制按钮统一：`WindowChromeControls` + `WindowTitleBar` / `WindowTitleBarActions`；hover 背景铺满标题栏高度（去掉 `buttonVariants` 默认 `h-7`，改用 `self-stretch`）
- [x] 各页面标题栏与 Edit 对齐：`h-10`、`bg-[var(--workbench-bg)]`、`shadow-xl`（Editor Menubar、ProjectPicker、DataView、Info / Plot / Log）
- [x] 关闭钮右上角与系统窗口圆角贴合：子窗口移除 CSS `rounded-tr-lg`，直角贴边由 OS 裁剪（与 Edit 主窗口一致）
- [x] Edit 菜单栏「显示详细」按钮垂直居中：补 `self-center`（与主题 / 设置按钮一致）
- [x] 修复 npm audit esbuild 漏洞（`package.json` overrides `"esbuild": "^0.28.1"`）
- [x] 画布视口按 graphId 存取/恢复：前后端与 `graph.canvas` 对齐；移除按 editor groupId 的错误 key；切换 tab / 加载项目时 `ensureGraphViewport` / `syncGraphViewportsFromRecords`
- [x] Tauri 前后端版本对齐：Rust `tauri` 2.11.3 ↔ `@tauri-apps/api` 2.11.x；同步 `plugin-dialog` / `plugin-fs` / `plugin-opener`
- [x] 修复 `useCanvasDrop` 视口迁移遗留 `_graphId` 引用导致 Canvas 崩溃
- [x] Rust 1.92 闭包引用模式：`ast/parser.rs`、`graph_instance.rs` 拓扑排序 `|&(_, &v)|` 修复编译错误
- [x] 修复 `typeVarBindings` 推断泄漏：concrete pin 不再创建临时 TypeVarId；`commit_to_graph` 仅保留 `node.type_var_map` 中的绑定
- [x] `.yssbi-event` 不再持久化 `typeVarBindings`（`#[serde(skip)]`）；图加载后 `prepare_graph_runtime` 重跑 `infer_types` 重建缓存
- [x] `.yssbi-event` 体积优化（Phase A）：连接仅存 `links`；跳过 `pinTypes` / `resolvedSchema`；节点存 `nodeType` 替代完整 `definition`；静态 pin 存 `pinContract`；保存前 `reconcile_connections` 清理孤立连接；紧凑 JSON（`to_string`）

## 2026.06.26

- [x] sequence 节点目前只能执行前三个 then，then 4 这个 pin 无法执行（flow_processor 改为动态读取全部 Then pins）
- [x] Detail 侧边栏按类型拆分 Panel（`Layout/Detail/panels/`：Variable / Event / Function / Data / Log / Node；`Detail.tsx` 薄路由）
- [x] 画布单选节点同步 Detail：`useEditorStore` 增加 `node` 类型 + `selectedGraphId`；`syncDetailFromNodeSelection` 桥接 canvas 选择
- [x] Node Detail：Pin 接口列表（类型、optional/required、repeatable/derived、连接状态）+ 中英 Markdown 文档（`react-markdown` + KaTeX）
- [x] 后端 `NodeMetaData.documentation { zh, en }` + `with_documentation()`；`catalog/docs/` 目录；OLS / OLS Summary 首批完整文档
- [x] 修复 Detail 点击节点报错：`nodeMetadata`（camelCase）与 `node_metadata` 字段不一致；Zustand pins selector 无限循环（`useShallow`）
- [x] 修复 Detail 左侧 Sash 拖动卡顿：拖动过程 DOM 直改 + rAF 节流，松手再写 layout store；文档 Panel `memo` + 拖动时 `contain`
- [x] Detail 面板 UI i18n：`detail.*` 键（`en-US` / `zh-CN`）；NodeDetailPanel、Pin 接口、文档区、空状态等按当前语言展示
- [x] 节点短描述 i18n（已废弃并移除）：曾用 `NodeMetaData.localized_description { zh, en }` + `with_localized_description()`；catalog 已全部改为仅 `with_documentation()` + `catalog/docs/en|zh/*.md`；Detail 仅展示 Markdown 长文档
- [x] OLS / OLS Summary Markdown 文档去重：移除与 Pin 接口重复的 Input/Output 表格，保留公式与 Usage
- [x] Tableau 式 Worksheet 工作区（Phase 1）：ActivityBar **Charts**（位于 Data 与 Commands 之间）；Sidebar 折叠 **Worksheets** 列表（与 Event/Variable 同风格）；Menubar **Data → 新建 Worksheet**
- [x] Worksheet 配置迁至 **Detail** 面板（数据集 / 图表类型 / X·Y 编码 / 列列表）；打开 Worksheet Tab 自动展开 Detail
- [x] 中间 **Worksheet Tab** 全屏嵌入预览（Scatter / Line / Histogram，`embedded` 模式无边框圆角）；空状态居中提示
- [x] 后端 `worksheets/*.yssbi-worksheet` CRUD + `get_plot_column_pair`；项目 reopen 扫描恢复；Tab dirty 关闭提示
- [x] Ctrl+S / 批量保存：Worksheet Tab 走 `saveWorksheet`，不再误调 `save_project_graph`
- [x] Tab 切换同步 Detail：`syncDetailFromEditorTab`（worksheet / event / function）；修复切回 graph Tab 时 Detail 仍显示工作表的问题

## 2026.06.27

- [x] **shadcn P0 — Detail 面板 token 迁移**：去掉 `#cccccc` / `bg-white/5` / `text-gray-*`；统一为 `text-foreground` / `bg-muted/50` / `text-muted-foreground`；抽 `DetailFieldRow` / `detailStyles` 共享样式（Variable / Event / Function / Data / Log / Node / Worksheet / PinEditor / DetailEmptyState）
- [x] **shadcn P0 — Sidebar token 迁移**：树行、折叠区、hover/active 改用 `--sidebar-*` 语义 token，减少 `text-gray-*` / `bg-gray-*`
- [x] **shadcn P1 — 补装 primitives**：Switch、Tabs、ToggleGroup、Checkbox、Tooltip（`npx shadcn@latest add …`）；App 根节点包裹 `TooltipProvider`
- [x] **shadcn P1 — PlotView 控件与外壳**：Line 工具栏 Switch + Button；图表容器 `#13151a` / `border-gray-*` → `border-border` / `bg-card`；新增 `plotShellStyles.ts`
- [x] **shadcn P1 — InfoView 交互控件**：PanelComponent pill → ToggleGroup；MarginsBlock 原生 `<select>` → `shared/ui/Select` + shadcn Input/Label
- [x] **shadcn P2 — ContextMenu 统一**：`shared/ui/contextMenu/ContextMenu.tsx` 使用 fixed portal + shadcn Button/Separator 样式（Radix 受控 ContextMenu 无法跟随光标）；Sidebar / Node / Pin / DataView / Canvas 变量菜单共用
- [x] **shadcn P1 — InfoView 公式块 Toggle**：抽 `InfoSegmentedToggle` / `InfoAccentButton`（`InfoViewControls.tsx`）；FormulaBlock / BinaryFormulaBlock / VARFormulaBlock / FormulaBlock2SLS pill → ToggleGroup
- [x] **shadcn P1 — InfoView 其它控件**：HypothesisTestBlock / ACFPACFBlock / SerialTestsBlock / DID / DFADF 原生 button → Button 或 InfoAccentButton；输入 → shadcn Input
- [x] **shadcn P1 — Checkbox 统一**：VariableDetailPanel / SettingsView SettingItem / SerialTestsBlock → shadcn Checkbox + Label
- [x] **shadcn P1 — Sidebar 树行**：列表行、折叠头应用 `buttonVariants(ghost)`（保留 DnD ref 结构）
- [x] **shadcn P2 — TabBar 标签**：TabItem 应用 `buttonVariants(ghost)`（保留 DnD）
- [x] **shadcn P2 — ImportModal / PinEditor**：ImportModal 分类与类型选项 → Button；PinEditor `title` → Tooltip
- [x] **shadcn P1 — InfoStatsTable**：抽 `InfoStatsTable` 共享外壳；CoefficientTable / VifTable / AnovaTable 原生 `<table>` → shadcn `Table`
- [x] **shadcn P2 — BottomBar Tooltip**：StatusItem `title=` → shadcn Tooltip
- [x] **shadcn P2 — PinInput Switch**：bool pin 自定义 checkbox → shadcn `Switch`（size=sm）
- [x] **shadcn P2 — ImportModal Tabs**：分类栏 Button 组 → shadcn `Tabs`（variant=line）
- [x] **shadcn P2 — Shell Tooltip**：TabBar 分屏/关闭、Sidebar 行内图标按钮 → Tooltip
- [x] **shadcn P2 — NodePalette token**：CategoryRow / 空状态 `text-gray-*` → 语义 token
- [x] **shadcn P1 — InfoView 表格 batch 2**：抽 `FormulaMappingTable`；ClassificationTableBlock / MarginsBlock / DFADFSummaryList / VecRank / VARSoc / FormulaBlock / BinaryFormulaBlock / PanelFormulaBlock / VARFormulaBlock → shadcn `Table` 或 `InfoStatsTable`
- [x] **shadcn P2 — LogView Tooltip**：工具栏 refresh / auto-scroll / filter / clear / close → shadcn Tooltip
- [x] **shadcn P2 — Canvas 执行栏 Tooltip**：debug / replay / pause / stop / execute → shadcn Tooltip
- [x] **shadcn P1 — InfoView 表格 batch 3**：抽 `IvFirstStageSummaryTables` / `VarModelTable`；2SLS / LIML / VAR / VEC / DataViewComponent 剩余原生 `<table>` → shadcn `Table`
- [x] **shadcn P2 — 零散 token 清理**：Pin / PlotWindow / SidebarDragOverlay / LayoutNodeRenderer / 数据导入选择 Modal `text-gray-*` → 语义 token
- [x] **shadcn P2 — ImportModal Tooltip**：comingSoon 类型选项 `title=` → Tooltip
- [x] **shadcn P2 — Shell Tooltip batch**：ActivityBar / Menubar / WindowChromeControls / Pin / PanelComponent 警告 / Line 工具栏 → shadcn Tooltip
- [x] **shadcn P2 — DataView Tooltip**：Toolbar 图标按钮、TitleBar 单元格预览、SQL 表选择 Modal 路径截断 → Tooltip
- [x] **shadcn P2 — Log token 清理**：`logPresentation` / LogPanelContent `text-gray-*` / `border-gray-*` → 语义 token
- [x] **shadcn P2 — 共享 ToolbarIconButton**：抽 `shared/ui/ToolbarIconButton.tsx`；Log / DataView / Menubar / Line 复用
- [x] **shadcn P2 — ProjectPicker Tooltip**：refresh / clear search / theme / settings / favorite `title=` → shadcn Tooltip；项目路径截断 → Tooltip
- [x] **shadcn P2 — ExcelSheetSelectModal Tooltip**：文件路径截断 `title=` → Tooltip（对齐 SqliteTableSelectModal）
- [x] **shadcn P2 — InfoView 截断 Tooltip**：CoeffBarChart 变量名、CoefficientTable 几率比说明 `title=` → Tooltip
- [x] **shadcn P1 — 零散控件收尾**：DIDComponent / ResidualPlot 原生 number input → shadcn Input；SettingsView 分区标题 token；SidebarDragOverlay token
- [x] **shadcn P2 — PlotView 统一**：plotShellStyles 壳层；chartTheme 序列色（accent 驱动）；Sash token；LoadingOverlay Card+Progress；RegressionShared Badge/Card；Select `id` prop
- [x] **TabBar 标签样式 token 化**：新增 `editorTabStyles.ts`（`editorTabItemVariants`、shell/action 类名）；`TabBar.tsx` 由 `--workbench-bg` / `--accent-color` 迁移为 `border-border`、`bg-background`、`before:bg-primary` 等语义 token（保留 DnD / 关闭 / 跨组，未改用 shadcn Tabs）
- [x] **Sidebar 右键「在资源管理器中打开」**：后端 `project/resource_reveal.rs` + command `get_project_resource_path(kind, resourceId)`（graph / database / worksheet）；前端 `ProjectService.revealProjectResource()` → `invoke` + `@tauri-apps/plugin-opener` `revealItemInDir`；Sidebar / Worksheet 右键菜单 + i18n `contextMenu.sidebar.revealInExplorer` / `revealInExplorerFailed`
- [x] **ContextMenu 点击无响应修复**：菜单项改用 `onMouseDown` + `preventDefault`；invoke 扁平参数；错误走 `formatErrorMessage` + `uiStore.showToast`
- [x] **Data Sidebar 扁平化**：去掉每个 database 的 `CollapsibleSection` 展开/列子树；与 Worksheet 一致用 `renderDataItem` 单行展示
- [x] **Sidebar 行样式统一**：抽 `Layout/sidebarUi/`（`sidebarStyles.ts`、`SidebarListItem`、`SidebarDraggableItem`、`SidebarCollapsibleSection` variant=`stacked|nested`）；`Sidebar.tsx` 的 event/function/variable/data/worksheet 行与折叠区统一复用（Windows 导入路径 `./sidebarUi`，避免与 `Sidebar.tsx` 冲突）
- [x] **资源默认名固定英文**：新增 `shared/constants/defaultResourceNames.ts`（`New Event` / `New Function` / `New Variable` / `New Worksheet` / `New Folder` / `New Project`）；创建 event、function、variable、worksheet、folder、project 不再随 UI 语言用中文默认名；移除 i18n `contextMenu.defaults.*`、`worksheet.defaultName`、`projectPicker.newProjectModal.defaultName`；新建文件夹对话框预填 `"New Folder"`（标题仍 i18n）
- [x] **Help 菜单 + About 对话框**：`app/appConfig/appLinks.ts` 集中外链；Help 按文档 / 发行说明 / GitHub 仓库 / 反馈问题 / 关于 布局；About 弹窗含版本与 GitHub / Issues 按钮；`openExternalUrl` + Tauri `openUrl`；`capabilities` 允许 `https://github.com/**`
- [x] **ProjectPicker 标题栏**：「返回编辑器」补 `self-center` 垂直居中；主题切换左侧新增 GitHub 按钮（跳转仓库）
- [x] **ProjectPicker 项目列表右键菜单**：复用 `ContextMenu` + `usePositionedContextMenu`；`projectPickerContextMenu/` 构建菜单（进入 / 在资源管理器中打开 / 收藏 / 从列表移除 / 移到回收站）；`ProjectService.revealProjectPath`
- [x] **ProjectPicker 删除项目确认对话框**：`DeleteProjectConfirmDialog`；command `delete_registered_project_files`（删注册表 + 删当前加载项目时 `ProjectCleared`）；**移到系统回收站**（`trash` crate 替代 `remove_dir_all`），非永久删除
- [x] **ProjectPicker 布局调整**：新建项目 / 导入 / 扫描移至右侧操作栏；移除「从列表移除」下方冗余路径展示（列表行已显示路径）
- [x] **ProjectPicker 标题栏筛选左对齐**：搜索框与排序（最近打开）紧挨 Logo 左侧，不再居中
- [x] **ProjectPicker 导入行为**：`importProjectFromDisk` 仅 `registerProject` 加入列表，不 `loadProject`、不跳转编辑器；**进入** / 双击列表项才打开项目
- [x] **ProjectPicker 文件夹扫描**：选文件夹 → `scan_projects_in_directory` 递归发现 `metadata.yssbi`（跳过 `.git` / `node_modules` / `target` 等）→ 注册到列表；`project_scan.rs` + `ProjectService.pickProjectScanDirectory`
- [x] **执行动画连线样式区分 exec / data**：`Edge.tsx` 新增 `edgeKind: 'exec' | 'data'`；data 线执行高亮翠绿 `#10b981` + 长虚线缓流（`edgeFlowData`）；exec 线琥珀 `#f59e0b` + 短虚线快流（`edgeFlowExec`）+ 更强脉动光晕；`EdgesOverlay` 按 `fromPin.type === 'exec'` 传入类型

## 2026.06.28

- [x] **ProjectPicker 扫描进度蒙层**：`scan_projects_in_directory` + Tauri Channel 推送扫描/注册进度；`ProgressOverlay` + `projectPickerProgress.ts` 统一打开/新建/扫描/清理进度生命周期；扫描可取消（`ProjectPickerTaskCancelRegistry` + `cancel_project_picker_task`）
- [x] **ProjectPicker 清理失效项目**：command `cleanup_invalid_registered_projects` 校验 `metadata.yssbi` 存在性并从注册表移除；侧栏「清理项目」+ 进度蒙层与后端取消；`project_picker_task.rs` 共用任务取消注册表
- [x] **ProjectPicker 侧栏按钮重排**：扩充列表（新建 / 导入 / 扫描）→ 选中项操作（进入 / 收藏）→ 维护（清理 / 从列表移除 / 移动到回收站）
- [x] **ProjectPicker 列表空白右键菜单**：`ProjectPickerContextMenuTarget` 区分 `project` / `list`；空白区域菜单含新建 / 导入 / 扫描 / 清理（与侧栏一致）
- [x] **ProjectPicker 侧栏「移动到回收站」**：与项目行右键相同，打开 `DeleteProjectConfirmDialog` 后移到系统回收站
- [x] **ProjectPicker 文案**：侧栏与菜单「导入项目 / 扫描项目 / 进入项目 / 收藏项目 / 清理项目」；「移到回收站…」→「移动到回收站」
- [x] **ProjectPicker 列表空白点击取消选中**：点击非项目行区域 `setSelectedId(null)`（`data-project-picker-item` + `closest` 判断）
- [x] **新建项目不进入编辑器**：`create_project` 仅创建文件夹 + `metadata.yssbi` + 注册列表，不 `ProjectLoaded`；前端创建成功后 toast 并留在选择页，进入需点「进入项目」或双击
- [x] **NewProjectModal 重构**：路径与项目名表单位置对调；移除前端路径校验与异步 `validateNewProjectPath`；创建成功/失败由创建按钮处理（成功关窗，失败 toast + 输入框红框）；浏览按钮 `h-9` 与 Input 对齐；错误不再内联展示
- [x] **DeleteProjectConfirmDialog 布局优化**：移除项目路径展示；标题 + 说明 + 底栏按钮（取消 / 移动到回收站），对齐 `NewProjectModal` 壳层；删除进行中禁止关闭；确认按钮文案与侧栏统一为「移动到回收站」
- [x] **跨平台路径显示适配**：Windows `canonicalize` 产生的 `\\?\` / `\\?\UNC\` 扩展前缀在展示与存储时剥离；后端 `path_format.rs` + `normalize_existing_path`；注册表 `fetch_by_path` 等价路径匹配并迁移旧格式；前端 `formatDisplayPath` / `pathsEqualForCompare` 用于项目选择器、新建项目、编辑器 `currentPath` 与底部栏
- [x] **连接 Pin 卡顿优化（前端）**：`connectPins` 乐观更新本地 store（`connectionOptimism.ts`）；`PinTypesInferred` 改为 `batchUpdatePinFields` 单次 set；`onPinPointerDown` 用 `pinFromStore` 替代整图 `deserializeGraph`；新建节点自动连线走 `executeCommand` 复用乐观路径
- [x] **Pin 连接根治与高亮修复（统一管线 + echo/batch，替代上一条的临时补丁）**：
  - 后端拓扑与副作用分离：`connect_topology` 仅改图结构，统一经 `finish_graph_effects(seeds)` 做「增量 schema 传播（`propagate_schemas_from` 下游闭包）+ 类型推断 + 受影响节点动态 pin 重建」；`connect` / `disconnect` / `disconnect_pin` 共用此入口
  - 批量粘贴：`batch_create_with_connections` 改为「每轮仅 `connect_topology` + 每轮一次 `finish_graph_effects`」，结束后单次发 `ConnectionsBatchCreated` + 合并 `NodePinsUpdated` + 一次 `PinTypesInferred`；删除批末重复 `infer_types`
  - 前端：`graphDataStore` 新增 `applyConnectionDraft` / `revertConnectionDraft` / `batchConnect`（单次 set）；`connectPins` 用 echo 抑制（`CONNECTION_ECHO_DOMAIN`）对齐 `MoveNodes` 模式；新增 `ConnectionsBatchCreatedHandler`；删除 `connectionOptimism.ts` / `pinFromStore.ts` / 粘贴 `setTimeout(50ms)`；拖拽链直接传 `Pin` 对象（`onPinPointerDown(pin, e)`）
  - 类型解析单一源：`dataTypeFromDisplayString` 对齐 Rust `DataType::from_str`（补 `Categorical` / `Date` / 裸 `DataSeries`），`pinCompatibility` 删除私有 `parseTypeDisplay` → 修复 OLS `DataSeries<Float64 | Categorical>` 拖拽不高亮
  - 执行 data 线高亮：执行器在每次 `NodeStart` 后调用 `emit_data_input_connections`，为该节点所有已连线 data input 发 `ConnectionActive`；`EdgesOverlay` 改读 `graphDataStore.connections` 作单一数据源
- [x] 批量节点复制粘贴「一个一个出现并连接」：粘贴改为单次 `ConnectionsBatchCreated` + 一次 `PinTypesInferred`，前端 `batchConnect` 单次 set，节点与连线同时出现（2026.06.28）
- [x] 连接 Pin 卡顿（后端）：schema 改为增量传播（`propagate_schemas_from` 下游闭包），连接/断开/批量统一经 `finish_graph_effects`，批量粘贴由 O(N) 次全图收尾降为每轮一次；类型推断暂保留全图以确保 unify 正确性（2026.06.28）
- [x] 连接 pin 的线在执行的时候执行动画只有一部分会亮（执行器在 `NodeStart` 后对节点全部已连线 data input 发 `ConnectionActive`，`EdgesOverlay` 读 store 连接，2026.06.28 根治）
- [x] **Sequence 执行顺序修复**：`TriggerOutput` / `TriggerSequence` 触发后立即完成的子帧改挂 `frame.parent_frame`（最近 waiting 祖先），不再挂到即将出栈的本帧；`TriggerAndContinue` 仍挂 `frame.id`；Loop 路径不变 → Then1 整条下游子树执行完毕后再执行 Then2，避免 Then 分支交错（`executor.rs`）
- [x] **Sequence 执行顺序回归测试**：`tests/common/mod.rs` 新增 `RecordingEmitter` 记录 `NodeStart` 顺序；`logic_test.rs` 新增 `test_sequence_runs_branch_fully_before_next`（Then1→A→A2、Then2→B→B2，断言 `[Seq, A, A2, B, B2]`）
- [x] **导入数据窗口与卡顿修复**：
  - 后端：`load_database` / `delete_database` / `list_sqlite_tables` / `list_sql_tables` / `list_excel_sheets` 改为 `spawn_blocking` 异步 command，重 I/O 不再阻塞主线程
  - 前端：导入/删除/读表列表包裹 `ProgressOverlay`（`dataOperationProgress.ts`），先绘制蒙层再 invoke；toast 与进度文案 i18n 化
  - UI：`ImportModal` 重构为左侧分类导航 + 右侧卡片选项，补 subtitle 与 `importModal.types.*` i18n
- [x] `.yssbi-event` 体积优化 Phase B：磁盘格式对齐 `GraphRebuildSnapshot`——`GraphInstance` 自定义 serde 落盘为扁平 `nodes[]`（pin 内联）+ 扁平 `connections[]`，去除 HashMap 键冗余与 `dataState` 包裹；静态 pin 由 registry 经 `set_registry` 重挂，动态/可重复 pin 自带完整定义 override；运行期缓存（`pinTypes` / `typeVarBindings` / `resolvedSchema`）不落盘
- [x] Dev/HMR 下 `[TAURI] Couldn't find callback id`：确认为 Vite HMR/整页重载期间长生命周期 `Channel`（`execute_project` 等）的残留回调所致——release 无 HMR 故天然干净；新增 `services/devHmrIpc.ts`（仅 `import.meta.hot` 守卫、生产 tree-shake）登记/拆除活跃 Channel，并仅过滤该条开发期噪声
- [x] 旧 `.yssbi-event` / `.yssbi-function` 迁移：移除早期读取分支（`node_instance` 完整 `definition` 回退、旧连接四表）；当前已删除过渡期旧格式回退读取
- [x] **创建节点卡顿根治（前后端双层）**：
  - 后端：`create_node_with_position` 与 `create_node_with_id` 去除孤立新建时的全图 `infer_types()`（新节点无连接，不影响既有 pin 类型，数据 pin 类型由定义 `data_type` / `variable_data_type` 覆盖确定）；`batch_create` / `connect` / `delete` 推断路径不变 → 单节点创建从 O(图) 降为 O(1)，且不再占用全局写锁
  - 前端乐观插入：`optimisticNodeDraft.ts` 依据注册表定义（对齐后端 `generate_initial_pins` + `from_pin_with_context`）在客户端生成 `nodeId`/`pinIds` 与初始 `NodeData`+`PinData`；`graphDataStore` 新增 `applyNodeDraft` / `revertNodeDraft` / `reconcileNode`；`createNodeCommand` 走 `create_node_with_id` + `trackPending(NODE_CREATE_ECHO_DOMAIN)`，失败回滚
  - 自回显对齐：`NodeCreatedHandler` 改用 `reconcileNode`——已乐观插入则按 id 覆盖权威字段（无重复、补齐 `defaultValue`/变量类型等），未插入则普通添加（redo/他端来源），时序无关
- [x] **从 pin 拖拽建节点：节点 + 连线一步即时出现**：新增合并命令 `CreateNodeWithConnection`（注册表 + `CommandType` + `STRUCTURAL_COMMANDS`），在任何 await 之前同步完成「乐观建节点 + `applyConnectionDraft` 乐观连线」，再后台按序 `create_node_with_id`→`connect_pins`；整段为单个撤销项（undo 删节点并恢复源端被自动断开的连接）；`handleNodePaletteSelect` 在存在 `pendingConnection` 时改派该命令，消除「先有节点、隔一拍才有连线」的两段式卡顿
- [x] **从 pin 拖拽建节点：自动对齐落点**：节点自动反向平移，使被连接的 pin 精确落在拖拽释放点（保持连线终点不动）；因节点宽高随内容动态变化（输出 pin 的 x 偏移=节点宽度），采用「测量后对齐」——`core/canvas/pinOffsetWaiter.ts` 的 `waitForPinOffset`/`resolvePinOffsetWaiters` 由 `useCanvasViewport` 测量布局后兑现，命令据测得偏移设最终位置（同步写入后端与撤销上下文，保证 redo 落点一致），超时回退不对齐
- [x] **画布渲染架构重构（逐节点订阅，根治创建/连接残留卡顿）**：原先每次 store 变更都触发整图级流水线——`useEditorGraphData` 对全图跑 `deserializeGraph`（O(节点×pin)），且 `useEditorGroup()` 被 Sidebar/Menubar/Detail/CanvasOverlays 多处调用导致重复执行；反序列化产生全新节点对象使 `Node` 的 `prev.node===next.node` memo 永远失效 → 所有可见节点重渲染；`useCanvasViewport` 测量副作用依赖 `nodes`，每次变更都对所有可见 pin 重测 `getBoundingClientRect`。改造分两阶段：
  - 阶段一·逐节点订阅渲染：`serialization.ts` 抽出共享纯函数 `resolveNodeViewMeta`（title/uiStyle/category/description 解析单一来源）；新增 `dataStore/useNodeView.ts`，仅订阅单节点切片（`nodes[id]` + 其 `pins[*]` + 各 `pinConnections[*]`，`useShallow` 比较），并由连接 id（`from->to`）直接派生 `links`（无需订阅整张连接表），返回引用稳定的 `UINode` 使 `Node` memo 真正生效；新增 `Nodes/CanvasNode.tsx`（memo 包装，`useNodeView(id)` → 渲染纯展示 `Node`）；`Canvas.tsx` 改为遍历稳定的 `graphNodes[graphId]` id 列表（`visibleNodeIds` 过滤）并下传稳定回调，不再遍历反序列化数组
  - 阶段二·停掉全图反序列化与全局重测量：`useEditorGraphData` 不再 `deserializeGraph`（`nodes` 从 `useEditorGroupWorkspace`/`useEditorState`/`useEditorGroup` 移除，`deserializeGraph` 仅保留给保存/按需）；`useCanvasViewport` 去除 `nodes` 入参，`nodePositionMap`/`pinNodeIdMap` 改由轻量 `useShallow` store 选择器派生，测量 `useLayoutEffect` 重新以 `visibleNodeIds + nodeResizeVersion`（而非位置）为依赖（pin 偏移相对节点原点，移动节点不再触发无关 pin 重测）
  - 净效果：对节点 X 的一次变更只重渲染 X（连接时加另一端）与连线层，告别「每次创建约 6 次 store 写入 × 全图反序列化/全节点重渲染/全 pin 重测量」的 O(N²)；阶段三（增量测量 + 逐边 memo）按「仅在仍卡顿时」前提暂缓
- [x] 目前创建节点还是非常的卡顿（见 2026.06.28「创建节点卡顿根治」「从 pin 拖拽建节点」三条）
- [x] 日志的更新会影响到其他组件卡顿，例如在这里我添加节点的时候，日志会发生更新，添加节点进而会卡顿，但是当我将日志隐藏起来之后，就不会卡顿了（见 2026.06.28「日志流解耦」）
- [x] **日志流解耦（根治日志更新拖累画布卡顿）**：根因——每条 `log-message` 事件都 `addLog` → zustand `set` → `LogPanelContent` 整体重渲染做 O(n) 工作（`[...logs, log]` 拷贝、render body 内 `getFilteredLogs()` 全量过滤、`autoScroll` 的 `useEffect([logs])` 读 `scrollHeight` 触发回流）；加一个节点会爆发 K 条日志 → K 次同步渲染抢占画布主线程，隐藏面板（卸载）才不卡。改造为「热数据流移出 React + 帧级合并」，对标 VSCode 节流 OutputChannel + 虚拟化：
  - 新增 `core/log/logBuffer.ts`（React-free 模块级环形缓冲，上限 `LOG_BUFFER_MAX=5000`）：`pushLive` 为 O(1)（超限丢最旧）并用 `requestAnimationFrame` 合并——一帧内涌入多少条都只重建一次快照、通知一次；`setInitial`/`prependOlder`/`clear`/`setLoading` 为人类频率操作，立即提交；`subscribe`/`getSnapshot` 供 `useSyncExternalStore`（快照引用稳定，仅 flush 时变化）；`getBackendCount()` 跟踪「已从后端加载的历史条数」作分页 offset
  - 新增 `core/log/useLiveLogs.ts`：`useSyncExternalStore(logBuffer.subscribe, logBuffer.getSnapshot)` → `{ entries, total, hasMore, loading }`
  - `logStore.ts` 瘦身为「冷」控制状态（`filter`/`selectedLog` + setters），移除 `logs`/`total`/`hasMore`/`loading`/`addLog`/`setLogs`/`appendLogs`/`prependLogs`/`setLogPageState`/`clearLogs`/`getFilteredLogs`；抽出纯函数 `applyLogFilter(logs, filter)` 供组件 `useMemo`
  - `useLogActions.ts` 改为经 `logBuffer` 写入（`setInitial`/`prependOlder`），`loadMoreLogs` 的 offset 改用 `getBackendCount()` 而非 `logs.length` → 顺带修复实时追加污染后端分页 offset 的潜伏 bug
  - `LogPanelContent.tsx` 改用 `useLiveLogs()` 取热数据、`useLogStore` 取冷状态；监听器 → `logBuffer.pushLive`；`filteredLogs = useMemo(() => applyLogFilter(logs, filter), [logs, filter])`；清空 → `logBuffer.clear()` + `setSelectedLog(null)`；autoScroll 因快照每帧至多变一次，回流风暴随之消失
  - 净效果：快照标识每帧至多变一次，过滤/自动滚动/虚拟化均每帧至多跑一次，与日志条数无关；内存由环形缓冲封顶，历史仍可经后端分页回看；`LogPanel` 与独立 `LogWindow` 同享此优化
- [x] **修复日志自动滚动失效（帧级合并的回归）**：原 autoScroll 用「内容追加后事后测量 `scrollHeight - scrollTop - clientHeight < 80`」判断是否贴底——逐条追加时每次高度跳变 < 阈值故持续跟随；改为帧级合并后一帧可新增大量行，高度跳变远超阈值被误判为「离底」而停止滚动。改为标准「贴底跟随」：新增 `pinnedToBottomRef`，由真实滚动事件（native onScroll + `handleScroll`）维护贴底状态，autoScroll effect 只要处于贴底就 `scrollTop = scrollHeight` 持续跟随（无关本帧新增行数），用户上滚离底即暂停、滚回底部即恢复；点亮「自动滚动」按钮时强制回到底部并贴底
- [x] **修复刷新后 Decompose DataFrame 连线全部消失（动态 pin 身份保持）**：根因——重开图时 `materialize_dynamic_pins` → `resolve_dynamic_pins_with_mode` 用「对顺序敏感」的 `old_names == new_names` 判断变化，一旦不等（哪怕仅顺序不同）就调用 `replace_node_pins` 整组替换：给所有动态 pin 生成全新 `PinId` 并 `disconnect_all`；而连接以 `PinId` 为键，故未变列的连线全被销毁。Decompose 输出为动态 pin（`with_dynamic(true)`）受此影响、其静态 `DataFrame` 输入幸存；触发条件是「重载列序（DuckDB 查询序）≠ 持久化 pin 序」及 Interactive 路径缺空 resolver 保护。
  - `graph_data_state.rs` 以 `reconcile_node_pins` 取代 `replace_node_pins`：按 `(PinDirection, name)`（列名即稳定身份，兼容 Decompose `Custom(name)` 与 prediction `Custom("pred_exog:{name}")`）对齐——存活列复用既有 `PinInstance`（保留 id + 连接，仅就地更新 `definition`/`order` → `updated_pins`），新增列建新 pin（`added_pins`），消失列删除并断连（`removed_pin_ids`/`removed_connections`）；`node.pin_ids` 重排为「静态 pin（原序）+ 动态 pin（目标序）」→ 纯重排不再改动任何 id；新增 `DynamicPinReconcile` 变更集
  - `graph_instance.rs` 的 `resolve_dynamic_pins_with_mode` 尾部改调 `reconcile_node_pins`，保留两处早退（名称同序跳过、Materialize 空 resolver 保护），由结果构建 `PinChangeSet`（填充 `updated_pins`），保留 `infer_types()`
  - 下游无需改动：`emit_pin_change_events` 从重排后的 `node.pin_ids` 派生 `pin_order`，前端 `NodePinsUpdatedHandler` 已处理 removed/added/updated/pinOrder
  - 测试：`graph_data_state.rs` 新增「重排保留 id+连接」「仅删缺失列」两例单测，连同原有 `round_trips_graph_with_dynamic_pins_values_and_connections` 全部通过

## 2026.06.29

- [x] 感觉 tooltip 太多了，去掉了菜单栏中最小化，最大化以及关闭的 tooltip
- [x] 类型推断估计存在较大的问题（推断精度本身，与粘贴卡顿无关，待单独排查）
- [x] **类型推荐/推断根治（前后端统一结构化 DataType，路线 B）**：解决「推荐对不上、刷新后类型停在旧值」三因——① 前端 Palette 变量/函数推荐走降级支路（扁平 `filterPin.type` + `dataTypeMatches` 纯字符串相等），与拖拽高亮 `isPinCompatible`（精确 `DataType` + `canAcceptDataType`）不一致，导致 `DataSeries<Float64>` 列漏推荐、`OneOf` pin 全推荐；② 后端 `infer_all` 逐边 `unify` 用 `?` 传播，一条脏边即整图推断失败，且调用处 `unwrap_or_default()` 静默吞错；③ 前端取结构化类型靠把 `typeDisplay` 字符串再 parse 回 `DataType`（`dataTypeFromDisplayString` 镜像 Rust `from_str`），「结构→字符串→再解析」往返是 2026.06.28 OLS 拖拽 bug 同源的持续漂移面：
  - 后端下发结构化 `DataType`：`DataType` 已是 `#[serde(tag="kind", content="inner")]`，序列化形态恰等于前端 domain `DataType`（`{kind, inner}`）可直通。`event_node.rs` `InferredPinType` 与 `schema/pin.rs` `PinInstanceDTO` 各增 `data_type: Option<DataType>`（camelCase `dataType`，`skip_serializing_if=None`），后者在 `from_pin_with_context` 用与 `type_display` 同源的 `dt` 填充；`command_connection.rs` `emit_inferred_types` 与 `command_variable/mod.rs` 内联构造处补 `data_type`
  - 前端统一到结构化单一来源：`PinTypesInferredPayload` / store `PinData` / domain `Pin` 增 `dataType`；`NodeEventHandler` 经 `dataTypeFromBackend` 归一写入 store；hydrate 路径（store spread + `useNodeView` spread）自然透传。`pinCompatibility.ts` `buildPinDataType` **优先读 `pin.dataType`**（无则回退 `typeDisplay`/`type`，兼容乐观 pin），新增导出 `pinAcceptsType(draggedPin, candidateType)`，`isPinCompatible` 复用之；`NodePalette.tsx` 变量 Get/Set 与函数子图分支改用 `pinAcceptsType`/`buildPinDataType` 精确判断；`optimisticNodeDraft.ts` 乐观 pin 写入 `dataType`；移除 `dataTypeMatches`（early-stage 直接删，无 shim）。`typeDisplay` 退化为纯展示/tooltip（Rust `Display` 唯一权威），不再参与兼容判断 → 根除 `from_str`↔`dataTypeFromDisplayString` 往返漂移
  - 后端推断健壮性（不引入排序）：`type_inference_session.rs` `infer_all` 改逐边 best-effort——单条 `infer_connection` `Err` 记 `log_sys::warn!`（含 from/to）后 `continue`，一条脏边不再毒化整图（并查集+绑定合并对边序无关，`commit` 仍严格）；`graph_instance.rs` 两处 `infer_types().unwrap_or_default()` 改为失败时 `warn!` 记录再回退，不再无迹可循
  - 测试：前端引入 vitest（`npm run test`）+ `pinCompatibility.test.ts` 12 例（DataSeries 列 Set/标量不误推、Float64 Get/String 不推、OneOf 仅兼容成员、函数 IO 精确筛选、`buildPinDataType` 回退回归）；后端新增 `PinInstanceDTO`/`InferredPinType` 序列化含 `dataType` 断言，及「含一条不兼容边的图 best-effort 仍推断其余 pin 为 Float64」单测；`cargo test --lib` 78 全绿（见 v1.0 待办「类型推断精度」项）

## 2026.06.30

- [x] 目前数据视图的数据库中老是出现：`_yssbi_rowid` 列 — 已改用 DuckDB `rowid` 伪列，ingest 不再写内部列；后续移除旧列清理分支
- [x] 节点 Detail 信息布局重设计：input / output pins 展示存在太多重复信息，方向已可用颜色/分组/位置区分时不再重复声明 “input/output”；保留最基本且高价值的信息（pin 名称、类型、必要的连接/默认值状态），整体布局更紧凑、易扫读
- [x] **Detail 面板 shadcn 化与共享组件抽离**：统一各类 Detail 的文本字号、字段行高和 Card/Form 视觉；新增共享 `DetailText` / `DetailBadge` / `DetailSectionHeader`、`DetailForm` / `DetailReadonlyField` / `DetailNameField`、`DetailColumnList`，将 Event / Function / Variable / Data / Worksheet / Log / Node 等 Detail 面板迁移到共享组件，减少散落样式和裸表格感
- [x] **移除 Detail 面板删除按钮**：去掉 Variable / Event / Function / Data / Worksheet Detail 中的删除入口，清理 `DetailDeleteButton` 及相关 `onDelete/onDeleted` 传参，Detail 只负责信息查看与轻量编辑
- [x] **Node Detail 头部信息精简**：移除节点 head 卡片中的内部 `type` 字段和 `graph` 字段，仅保留名称与必要分类信息，避免展示低价值实现细节
- [x] **Node Detail Pin Interface 重设计**：Pin Interface 抽成可复用 `DetailCollapsibleSection` 折叠卡片；Pin 接口默认收起，展开后用 shadcn Tabs 在 Inputs / Outputs 间切换；Documentation 也复用同一折叠组件并默认展开
- [x] **Node Detail pin item 紧凑化**：pin 类型不再直接占位显示，改为 hover 名称/左侧空白区域展示类型 tooltip；optional / repeatable / derived 等状态放到右侧 badge，长说明移到对应 badge tooltip；input/output pin item 统一样式，减少重复方向信息
- [x] **Detail sash 拖拽卡顿优化**：定位到拖拽时 `OverlayScrollbar` 的 `ResizeObserver` 随宽度变化持续 `setState` 导致 Detail React 重渲染；改为 sash 拖动期间跳过滚动条 thumb 更新，拖动结束通过 `layout-sash-drag-end` 补一次更新
- [x] **Worksheet preview 缓存第一阶段**：新增 `worksheetPreviewCache`，按 worksheet spec 缓存 `WorksheetPreviewPayload`，支持 LRU、并发请求去重、同步缓存命中读取；`WorksheetChartPreview` 切回 tab 时优先同步读缓存，命中时不再等待 300ms debounce；DataView 编辑成功后按 `databaseId` 失效相关 preview 缓存；补充缓存 key 稳定、命中、并发去重、按数据库失效等 vitest 用例
- [x] pin 拖动的时候，不亮的也能合并，需要修复；以及拖动的时候出现的节点好像有点儿匹配不上（已完成，见 2026.06.30「Pin 类型匹配 / TypeSystem 架构清理」）
- [x] **Pin 类型匹配 / TypeSystem 架构清理**：合并并执行 `pin-type-architecture` 与 `type-system-cleanup` 计划，确立 `DataType + TypeSystemSnapshot` 为 pin 类型判断事实来源；后端新增 Struct 类型系统快照并下发 schema，`DataType::can_accept()` 支持 `Struct<Model>` 接受 `Struct<OLSModel>`，前端 `dataType` DTO 修复 `Struct` inner/key 丢失，`pinCompatibility` 统一使用 TypeSystem 做 Palette 推荐、pin 高亮与自动连接匹配；通用 `Predict` 的 Model input 改为 `Struct<Model>`，pin 落点连接前增加类型匹配防御，修复 OLS `model` output 拖拽不推荐 Predict / 已有 Predict pin 不高亮 / 不亮仍尝试连接的问题；补充前端 DTO 与 pin 匹配 vitest、后端 Struct 族匹配单测
- [x] **Pin 类型架构后续收口**：完成第一轮类型系统收口——前端抽出统一 `canConnectPins(a, b)`，高亮、落点连接与旧 `connections.ts` 校验共用 `dataType + TypeSystem` 判断；`buildPinDataType()` 不再从 `typeDisplay`/`type`/`containerType` 回退推断，data pin 缺少结构化 `dataType` 直接视为 schema bug；后端将 `DataType::can_accept()` 收回为基础精确规则，Struct 族匹配迁移到显式 `TypeSystemSnapshot::can_accept(target, source)`；`NodeDefinition` 支持声明 Struct 类型元信息，`NodeRegistry` 聚合生成 TypeSystemSnapshot，`OLSModel -> Model` 由 OLS 节点注册声明并随 schema 下发；补充 `canConnectPins`、禁止 fallback、旧连接校验和后端 TypeSystem 单测
- [x] event等其他 detail 的 head 中有修改名字的 input 框的时候，有bug，其修改名称不是失去焦点后保存而是修改保存，需要切换为失去焦点后保存或者回车保存（已完成：抽出 `DetailCommitInput` 本地草稿输入，Detail 名称、变量值、Function pin 名称均改为 blur / Enter 提交，Escape 回滚）
- [x] 变量切换类型 dataview 无法获取
- [x] **变量 Detail 类型切换后端不变量**：变量类型 Select 保持即时响应，但类型变化规则收进后端 `ProjectState::update_variable`——只提交 `dataType` 且未显式传 `dataValue` 时，后端用 `DataType::default_value()` 重置变量值，不再保留旧类型历史值；`update_variable` command 返回更新后的完整变量，前端 `VariableService.updateVariable()` 用后端结果刷新 store，修复 Int 切 Boolean 后 UI/值不同步的问题；补充后端类型切换重置默认值与显式值优先单测
- [x] 断开连接后 pin 的状态有时还是连接状态（已完成：确认后端断开路径均更新 `ConnectionManager`；前端根因是 Node Detail 用非事实来源 `pin.links` 判断 connected，而连接/断开事实在 `pinConnections`。已抽出 `pinLinks` 派生工具，Canvas `useNodeView` 与 Node Detail 统一从 `pinConnections` 派生 runtime links，避免断开后残留连接状态）
- [x] **Pin links 历史残留收口**：`PinData` / 前端 `PinInstanceDTO` / 后端 `PinInstanceDTO` 不再携带 `links`，连接事实统一由 `connections` / `pinConnections` 维护；Store 写入入口新增 `toStoredPin()` 剥离旧运行时 links，`replaceGraphNodes` 不再从传入 Pin 对象读取 links；旧 `graphConverters` 删除对外暴露的 `applyConnectionsToPins` / `extractConnectionsFromPins`，视图层改为从 `pinConnections` 派生 `connected` / `linkCount` / `connectionIds`

## 2026.07.01

- [x] 去掉项目所有旧数据迁移 / 兼容读取逻辑；如 `_yssbi_rowid` 旧列清理分支
- [x] 已经修复 ~~右键 sidebar 中的列表 item 重命名的时候，延迟很重，同时如果 tab 如果有这个 item 的选项卡的时候呢，这个选项卡会消失，重新打开的时候发现 tabbar 中的名字没有更改；其次在 detail 重命名的时候 tabbar 上的名字更改了，但是当我关闭这个 tab 重新打开的时候，tab 上选项卡的名称变成更改之后的了，但是 detail 和 sidebar 中的字符串又是原来的；我再次执行关闭更改操作的时候，全部恢复为原来的了~~
- [x] **VSCode 式 Project / Resource / Tab 架构重做边界**：已新增 `ResourceStore` / `DocumentStateStore` / `ResourceRef` / `ResourceKey`，以 `resourceId + kind + uri` 作为资源稳定身份；graph / worksheet / database / variable 元信息统一进入资源索引，TabBar 标题从资源索引派生，旧 `LayoutTab.title` 仅作为 fallback。
- [x] **ProjectWatcher / 文件系统监听层**：Rust 侧新增 `ProjectWatcherState`（基于 `notify`），项目加载 / 另存后监听项目目录；外部 graph / worksheet 新增、删除、重命名、移动等变更经 debounce 后重扫 `ProjectIndex`，通过 `ResourceChanged` / `ResourceDeleted` 增量同步前端，并将已打开资源标记为 `stale` / `missing` / `conflict`。
- [x] **统一 ResourceActions 与后端原子资源命令**：新增 `resourceActions` 收口 Sidebar / Detail / ContextMenu 的资源创建、重命名、删除、folder 操作；后端新增 `command_resource.rs` 与 `rename_graph_resource`，graph 重命名会更新内存 graph、持久化 document name / 文件名，并广播 `ResourceChanged`，避免重开恢复旧名。
- [x] **Tab 模型改为资源引用模型**：TabBar 渲染时优先从 `ResourceStore` 读取资源名；新增统一 `updateOpenResourceLabels(resourceRef, name)` 同步仍保留的 legacy title fallback；graph / worksheet 等名称同步不再散落在各 UI 层。
- [x] **轻量资源索引刷新替代全量项目重载**：`ProjectIOStore` 新增 `refreshResourceIndex()`，普通资源 create / duplicate / folder rename / delete 后只刷新 graph meta、worksheet index 与 ResourceStore，不再调用破坏性的 `loadProject()`，避免关闭 tab、清 viewport/history/cache。
- [x] **Dirty / Loaded / Missing 状态机标准化**：graph / worksheet 的 loaded、dirty、save、close 接入 `DocumentStateStore`；ResourceStore 只保留 `hasDirtyDocument` / `hasStaleDocument` / `hasConflictDocument` 摘要；watcher 来源外部变化会让打开资源进入 `stale` / `missing` / `conflict`，TabBar 增加最小状态提示。
- [x] 在资源管理器中添加新的 yysbi-event 文件，项目的 event 并不会更新（已修复：watcher 发来的 `ResourceChanged` 对 graph meta 投影改为 upsert，新增 event/function 会进入 Explorer 列表；`read_project_index()` 跳过单个非法 graph 文件，避免坏 `.yssbi-event` 阻断整个资源索引刷新；资源管理器复制出的重复 graph id 文件会在索引扫描时自动规范化为新 graph id）
- [x] **Resource Snapshot Sync 架构切换**：外部文件系统变更不再走 watcher 增量 `ResourceChanged` / `ResourceDeleted`；Rust watcher debounce 后只广播 `ProjectIndexInvalidated { source, version }`，前端 `ProjectIndexInvalidatedHandler` 合并突发事件并通过唯一入口 `refreshResourceIndex()` 拉取完整 `ProjectIndex` 快照。
- [x] **ResourceStore 成为 Explorer 唯一事实来源**：Event / Function 列表、graph folder、graph order、auto-open-first-graph、Sidebar 与项目打开首图逻辑均改为从 `ResourceStore` selectors 派生；`GraphMetaStore` 不再驱动资源列表 / 顺序 / 文件夹。
- [x] **ProjectIndex 快照原子替换与打开文档 reconcile**：`refreshResourceIndex()` 会重建 `ResourceStore` 与 worksheet index；保留已加载资源的 loaded / dirty 摘要；快照缺失的已打开资源保留为 `missing`，快照元数据变化时 clean 文档标记 `stale`、dirty 文档标记 `conflict`，避免打开 tab 静默消失。
- [x] **Resource sync 测试与格式收口**：新增/更新 `ResourceEventHandler.test.ts`、`resourceSnapshotReconcile.test.ts` 与 watcher 路径测试，覆盖 invalidation coalescing、ResourceStore selector、snapshot reconcile；修复 `ResourceEventHandler.ts` / 测试文件多余空行格式问题。
- [x] **Document-owned variables + derived VariableIndex**：后端 `ProjectIndex.variables` 索引全局 `variables.yssbi-vars` 与各 graph 文档 `localVariables`（含 owner graph 元数据）；复制 graph 时保留并重写 local variable id/scope 与节点 `variableId` 引用。
- [x] **Runtime SymbolTable 作用域**：`GraphRuntime::get/set_variable_value` 仅允许全局变量与当前执行 graph 拥有的局部变量，拒绝其他 graph 的 hidden locals。
- [x] **前端 VariableStore catalog + ResourceStore projection**：`loadProject()` / `refreshResourceIndex()` 从 `ProjectIndex.variables` 灌入 `VariableStore`，经 `variableCatalog.ts` 统一投影到 `ResourceStore`；graph cache release 仍只清除该 graph 的局部变量。
- [x] **变量 Application Action 收口**：新增 `variableActions.ts`（create/update/delete/rename + `rebuildVariableResourceProjection`）；`useVariableManagement` 与 `resourceActions` 变量路径改为委托 action；`VariableEventHandler` 不再手动 patch 节点快照。
- [x] **后端变量 command 薄化**：`command_variable` 委托 `ProjectState::sync_variable_references` 处理节点引用与 pin type 同步；`get_variable` / `delete_variable` 返回明确 `Result` 错误而非 `unwrap()`。
- [x] **变量节点显示派生**：Get/Set Variable 节点标题固定为节点语义名称，变量名与类型从后端 `VariableIndex` / runtime symbol table 解析到 data pin；前端 pin label 从 `VariableStore` 响应式派生且保留原始大小写；移除 hook/handler/拖拽路径中冗余 `variableName`/`variableType` 快照与手动同步。
- [x] **DataFrame 节点显示派生与快照清理**：Get DataFrame 与 Get Variable 保持同一模式——节点标题固定为 `Get DataFrame`，节点参数只保存稳定 `dataframeId`，data pin 名称由 database catalog 动态解析；移除后端 `NodeInstanceParams::DataFrame.dataframeName` 与前端 DTO/store/serialization/node view 中的 `dataframeName` 快照字段，并补充序列化与 graph load pin 名解析回归测试。
- [x] **彻底移除 Pin.links / deserializeGraph 遗留**：连接事实仅保留 `GraphDataStore.connections` / `pinConnections`；Canvas 与 Node Detail 统一消费派生 `connected` / `linkCount` / `connectionIds`；删除 `deserializeGraph` / `serializeGraph` / `convertGraphFromDTO` 等旧 runtime 视图转换；新增 `derivePinConnectionView`、`findInternalNodeInGraph`、`buildRuntimeNodesFromStore` 等 store-native 查询；domain `Pin` 不再携带 `links` 字段。
- [x] **统一 DataView 组件体系**：新增 `features/core/dataView`（`UnifiedDataView` + source-only renderer resolver + `DataViewShell`）；Debug View 节点、Info summary 与 Plot 窗口统一注册为 `WindowDataStore` backend source；前端窗口先读取 source metadata，再通过 `get_window_source_value` / `get_window_source_page` 获取实际 JSON 或 DataFrame/DataSeries 分页数据；OLSResult struct 走 `OLSComponent` 结构化展示。
- [x] **变量测试补齐**：Rust 覆盖 VariableIndex、复制 graph local 修复、runtime 作用域、`update_variable` 类型切换默认值；Vitest 覆盖 `variableCatalog` 灌入与 resource projection。
- [x] **DataView source-only 收口**：所有窗口化结果（Debug View 节点、Info summary、Plot）统一注册为 `WindowDataStore` backend source，前端 DataView 只消费 source metadata；删除 inline/pageable renderer 分支和 legacy inline snapshot 新路径；旧 InfoWindow summary / Plot 窗口也改为 metadata + source value 读取。
- [x] **所有节点运行结果 Source 化**：所有节点 output pin 的运行结果统一注册为 backend `RuntimeResultSource`，前端 Detail / Debug / View 窗口都通过 source descriptor + typed read API 查看结果；View 节点只作为打开结果窗口的入口之一，不再是查看运行结果的唯一入口。需要设计执行缓存生命周期、pin result 索引、重跑覆盖、图切换清理与 Detail/Debug UI 入口。
- [x] **Unified Result Source 清理收口**：项目 load / save-as / new / 删除当前项目时同步清空 `ResultSourceStore`；删除后端旧 `get` / `get_source_value` 兼容 API 与前端 `DataViewService`、`DataViewPayload`、`WindowSourceMetadata` 等别名；View 节点改为直接构造 typed `ResultSourceRecord`，不再生成 legacy `viewType/dataType` JSON；`source_builder` 只保留普通 JSON 窗口与 plot payload 解析，DataFrame / Series / Struct 统一走 typed builder。
- [x] **DataFrame pin structured dataType 恢复**：修复 Get DataFrame 节点从项目文件加载后 data pin 名称已派生为数据集名（如 `iris`）但 `pinContract` 精简持久化导致 `dataType` 丢失的问题；`resolve_dataframe_nodes()` 在保留显示名的同时补回 `PinDataTypeDefinition::DataFrame` 与 `pin_types` 缓存，拖拽 data pin 时不再触发 `missing structured dataType` 报错，并补充加载回归测试。

## 2026.07.02

- [x] 如果在 tabbar 中打开了两张以上的选项卡，那么删除其中一个选项卡后，显示的第二张选项卡是空白的，如果删除了第二张选项卡，则会出现很多的 GRAPH [GraphDataStore] deleteNode: Node "9971537d-69e1-41ca-bb16-4049f81179e0" not found 错误；接着删除则会接着出现这个错误（已完成：关闭 active graph tab 后立即加载并同步新的 active graph；`clearGraph` / `NodeDeletedHandler` 改为幂等；tab 切换清理 selected nodes；补充 tab close、layout selection 与 graph clear 回归测试）
- [x] Event 只改名称；Function 除名称外还可能通过 PinEditor 改 inputs/outputs。因此 updateEvent/updateFunction 不能全部删除，但可以明确职责：名称走 renameResource，Function 结构性更新暂时保留为 graph document 更新，后续可单独抽到 graphDocumentActions。现在开始按这个边界改代码。 这一部分有更好的处理方式吗？（已完成：`useGraphManagement` 收口为 UI 编排层；Event/Function 创建、删除、重命名委托 `resourceActions`；Event 更新仅走资源重命名，Function 结构性更新暂留 graph document 路径）
- [x] **Graph resource identity 改为项目维护**：graph 文件不再持久化 / 读取自身 `graph.id`，新增 `graph_resource_index.rs` 维护 `resourcePath -> GraphId` manifest；`read_project_index()`、graph 加载、移动、删除、复制均通过显式资源表解析 graph id；资源管理器复制出的同内容 graph 文件会获得独立项目 graph id。
- [x] **移除 hash / remap 身份桥接残留**：删除 `GraphId` / `NodeId` / `PinId` / `VariableId` 的 `stable_uuid_from_key` / `from_stable_key`；删除旧 graph 文件扫描匹配、稳定 hash 与 entity remap helper；复制 graph 不再为了资源身份重写 node / pin / local variable id。
- [x] **Node / Pin / Local Variable graph-local 化**：后端复制 graph 时保留 node、pin、local variable 本地 id，只重绑 local variable scope 到新 graph；变量节点继续引用本 graph 内 local variable id；补充 graph-local node/pin/local variable 复制回归测试。
- [x] **GraphDataStore graph bucket 架构**：前端 `GraphDataStore` 新增 `graphEntities[graphId]` 作为 graph 数据权威 bucket；所有 node / pin / connection mutation API 收紧为必传 `graphId`；sync handlers、history commands、editor 操作、clipboard、project snapshot、Node Detail / Canvas node 读取路径改为 `graphId + localId`。
- [x] **关闭 tab 空白根因收口**：修复 duplicated graph local id 导致 `releaseGraphCacheIfClosed` 清理一个 graph 时误伤另一个 graph 的问题；`clearGraph` 只删除目标 graph bucket；graph-scoped selector 不再在已存在 bucket 时 fallback 到 flat mirror，避免错读同 local id 的其他 graph。
- [x] **EdgesOverlay graph-scoped 渲染**：Canvas 连线层不再读取全局 `connections` / `pins` flat mirror，改为读取当前 graph 的 connections 与 pins；补充两个 graph 拥有相同 local pin id 时只渲染当前 graph 连线的回归测试。
- [x] **GraphDataStore mirror 性能收口**：flat `nodes/pins/connections/nodePins/pinConnections` 降级为 legacy/read mirror；单 graph bucket mutation 改为增量 patch 受影响 mirror key，不再每次 mutation 全量 `flattenGraphBuckets`；全量 flatten 仅保留在整项目 hydrate 场景。
- [x] **Clean Graph Identity 验证**：新增/更新 `graphDataStore.test.ts`、`NodeEventHandler.test.ts`、`EdgesOverlay.test.tsx` 与后端 `project_io` identity 回归测试；通过 `npm run build`、`npm test -- --run`、`cargo fmt --check`、`cargo test --lib`；残留 hash / remap helper 搜索无匹配。
- [x] **Graph Document Actions 收口**：新增 `graphDocumentActions.updateFunctionSignature()` 与窄类型 `FunctionSignaturePatch` / `FunctionPinSpec`；Function inputs / outputs 结构更新改走独立 application action 与后端 `update_function_signature` command；名称更新继续走 `resourceActions.renameResource()`，不再混用 `updateFunction(Partial<Graph>)`。
- [x] **Detail / useGraphManagement 边界清理**：`FunctionDetailPanel` 将 `onUpdate(Record<string, unknown>)` 拆为 `onRename(name)` 与 `onSignatureChange(patch)`；`Detail` 中 Event / Function 名称直接委托 `renameResource`，Function pins 委托 `updateFunctionSignature`；`useGraphManagement` 删除 `updateEvent` / `updateFunction`，只保留创建后打开、sidebar 切换、toast/logger 等 UI 编排。
- [x] **Function signature 文档模型持久化**：后端 `GraphInstance` / `GraphInstanceDTO` 新增 `functionInputs` / `functionOutputs`，只允许 `GraphKind::Function` 通过 `ProjectState::update_function_signature()` 更新；Event 调用返回错误；DTO 始终返回 signature 数组，确保清空 signature 也能同步到前端。
- [x] **Function signature meta 同步收口**：新增 `syncFunctionSignatureMeta()` 作为前端唯一 signature meta 同步 helper；`projectIOStore.loadGraph()`、`graphDocumentActions`、`FunctionCreatedHandler`、`FunctionUpdatedHandler` 统一使用该 helper；helper 明确忽略非 Function graph，避免 Event graph 写入 function signature meta。
- [x] **Graph event handler 清理**：`FunctionUpdatedHandler` 不再在缺少 resource meta 且 payload 没有 name 时用 graph id 兜底创建 meta；`GraphUpdatedPayload` 显式补充 `functionInputs` / `functionOutputs` 字段；`GraphEventHandler` 抽出 `buildGraphUpdateData()`，去掉 inline `as unknown as import(...)` 双重 cast。
- [x] **Graph Document Actions 测试与验证**：新增 `graphDocumentActions.test.ts`、`graphDocumentMeta.test.ts`、`GraphEventHandler.test.ts`、`FunctionDetailPanel.test.tsx`，覆盖 signature action 同步、Event graph 忽略、FunctionCreated / FunctionUpdated 事件同步、缺少 resource/name 时不创建 meta、Detail rename/signature 回调分流；通过 `npm run build`、`npm test -- --run`、`cargo fmt --check`、`cargo test --lib`（后端验证在引入 Rust command / DTO 时完成）。
- [x] **Variables 侧栏从 Graphs 迁出**：Graphs 侧栏移除 Variable 区块，变量浏览与管理统一收口到 ActivityBar → Variables；Global / Local 均支持拖拽、重命名、删除；创建变量后自动切换到 Variables tab 并展开对应分组。
- [x] **Variables 侧栏 Local 上下文化**：Local 仅展示 tabbar 当前激活 Event/Function tab 的局部变量，不再按 graph 分组展示其他图/函数变量；移除 `localVariablesByGraph`、嵌套 graph 分组、`AddVariableTargetGraph` / `targetGraph` 与 `variablesLocal_${graphId}` 等逻辑；无激活 graph tab 时 Local 显示「无活动图」并禁用新建。
- [x] **Variables 侧栏顺序与默认展开**：Variables tab 顺序调整为 Local 在上、Global 在下；`sidebarStore` 分组顺序与默认展开状态同步为 Local 默认展开、Global 默认折叠。
- [x] **Sidebar Data 图标回归修复**：清理变量 promote/demote UI 时误删 `VscEye` import，恢复 Data 侧栏「在数据视图中查看」按钮渲染。
- [x] **Sidebar / Detail i18n 收口**：Sidebar 顶栏标题统一走 `activityBar.*`；折叠区块标题与空状态补全 `sidebar.sections.*`、`sidebar.noEvents` / `sidebar.noFunctions` 等翻译（Charts 仍用 `chartsSidebar.*`）。
- [x] **Workbench 顶栏高度统一**：新增 `workbenchPanelHeaderClass`，Sidebar head / TabBar / Detail head 统一为 `var(--titlebar-height)` + `border-b`；Detail 移除 header 下方多余 `Separator`，避免比 Sidebar / TabBar 高出 1px。
- [x] **Sidebar / Detail 顶栏字体统一**：新增 `workbenchPanelHeaderTitleClass`（`text-xs font-semibold uppercase tracking-wide`）；Sidebar head 与 Detail head 共用，`detailSectionTitleClass` 复用同一 class。
- [x] **Charts 工作表右键删除**：`buildSidebarContextMenuSections` 工作表项新增删除；`Sidebar.tsx` 接入 `deleteWorksheetWithConfirm`（确认后删文件、更新列表、关闭已开 tab）。
- [x] **Explorer Sidebar 折叠与右键菜单统一**：抽 `sidebarUi/SidebarChevron`、`SidebarRowActionButton`、`SidebarEmptyPlaceholder`；`SidebarCollapsibleSection` 统一折叠箭头（12px 旋转）并新增 `collapsible={false}`（Commands Undo/Redo）；空状态 token 统一为 `text-muted-foreground/70`；数据集行内打开图标与菜单对齐为 `VscChevronRight`；工作表区块空白右键「新建工作表」+ 工作表项「重命名」；移除 Graphs 区块冗余第三层 `onContextMenu`；删除未使用的重复目录 `Layout/sidebar/`。
- [x] **NodePalette 搜索栏与折叠/展开**：搜索框与左侧间距修正（`gap` + 容器 `px`）；搜索区与折叠按钮合并为圆角输入组（边框 + 轻阴影 + 竖线分隔 + focus ring）；纯图标按钮在「全部折叠 / 全部展开」间切换（`VscFold` / `VscExpandAll`，无 Tooltip）；`canvas.nodePalette.*` i18n（placeholder / collapseAll / expandAll / noMatches）。

## 2026.07.03

- [x] **共享右键菜单宽度收口**：`shared/ui/contextMenu/ContextMenu.tsx` 去掉 `min-w-[190px]`，改为 `w-max` 按内容自适应；`max-w-[13.5rem]` 限制最长项（如「在资源管理器中打开」）；菜单项 `px-2` / `gap-1.5` 略收紧。Sidebar、节点/Pin、DataView、ProjectPicker、画布变量菜单等共用组件一并生效。
- [x] **删除未使用的 shadcn ContextMenu**：移除 `components/ui/context-menu.tsx`（Radix 封装无任何引用）；应用内右键菜单统一使用 `shared/ui/contextMenu`（portal + 光标定位），避免与 Radix 受控 ContextMenu 定位能力冲突。
- [x] **去掉 Event/Function 文件夹逻辑并扁平化 Sidebar**：Event / Function 改为与 Data 相同的扁平列表（保留折叠区块）；删除 `renderGraphTree`、folder 右键菜单、`GraphFolderDropTarget` 与 DnD `GRAPH_FOLDER`；前端移除 `graphFolders` / graph `folderPath` 与 4 个 folder resource action；后端注销 `create/rename/delete_graph_folder`、`move_graph_to_folder`，`create_event` / `create_function` 去掉 `folder_path`；`events/`、`functions/` 仅一级扫描，保存固定根目录；打开项目时 `flatten_graph_layout` hoist 嵌套 graph 并 reconcile manifest。
- [x] **去掉 Worksheet 文件夹字段并统一磁盘布局**：删除 `WorksheetDocument` / 索引 / 前端 `folderPath`；`worksheets/` 仅一级扫描，保存固定根目录；`flatten_worksheet_layout` 与 graph 对称（`read_project_index` 顶层 flatten；load/save/delete/create 各 IO 入口 flatten 一次；`read_worksheet_index_entries` 纯扫描）；补充 `flatten_worksheet_layout_hoists_nested_files` 等后端测试；更新 `dnd-dropzone-contracts.mdc` 移除 graph-folder 约定。
- [x] **View 节点 source 统一走 source_id**：新增 `ensure_view_source_for_input`（upstream 复用 `runtime_*` pin source，否则 `insert_window_source`）；抽取 `build_source_from_resolved` / `ResolvedSourceValue`，与 `register_output_source` 共用；`view_nodes.rs` 简化为解析 source → `open_existing_source_window`，不再按 `DataValue` 重复分支构建。
- [x] **View renderer 收敛为 5 类**：`null` / `scalar` / `dataframe` / `dataseries` / `json`；删除 `struct_ols` / `struct_generic` 与 `OlsStructSourceView`；Struct（含 OLSResult）在 View 内统一 JSON 展示，计量 Summary 仍走 `/info` → InfoWindow。
- [x] **DataSeries 与 DataFrame 同构 tabular**：后端 `data_series_page` 与 descriptor 统一返回 `columns`（`#` + 列名）+ `rows`；前端 `DataSeriesSourceView` 与 `DataFrameSourceView` 共用 `TabularSourceView` + `ReadOnlyDataGrid`。
- [x] **Array / Object / Struct → json renderer**：`build_json_source_from_data_value` 将 Array、Object 从 scalar 拆出；`build_struct_source` 序列化 handle 为 JSON；前端 `JsonSourceView` + `JsonTreeView`（可折叠树，默认展开 2 层）。
- [x] **View 子窗口 layout 与滚动链**：`DataViewShell` 新增 `layout: window | embedded`；`DataViewWindow` source 模式 `flex min-h-0 flex-1`；tabular `ReadOnlyDataGrid` 支持 `fillHeight` 占满剩余高度；JSON / scalar 长内容 window 模式包 `OverlayScrollbar`；`UnifiedDataView` 透传 `layout`（子窗口 `window`，Canvas Runtime Results `embedded`）。
- [x] **Database 查看器 vs Runtime View 窗口命名拆分**：`/dataview` + `dataView` kind 专用于 DuckDB 表编辑；View 节点 / pin 预览走 `/view` + `runtimeView` kind；后端 `window_type` 为 `runtime_view`；新增 `RuntimeViewWindow`；`openPresentationWindow` + `presentationRouteForDescriptor` 统一打开逻辑；Pin inspect / View 节点执行不再混用 DataView 壳层。
- [x] **Runtime View 窗口标题与 View 节点对齐**：`register_output_source` 改走 `default_view_title`（`View: DataFrame` / `View: OLS Model` 等），不再用 `Output`；无 descriptor 时 fallback 为 **View** / **查看**，与 **数据查看器** 区分。
- [x] **变量类型禁止 Any**：Detail 类型下拉移除 Any；前后端 `create_variable` / `update_variable` 与 `variableActions` 校验拒绝 `DataType::Any`（无旧 Any 变量兼容层）。
- [x] **Series 数据结构重命名为 DataSeries（对齐 pandas / pin `dataseries`）**：`SourceKind` / `SourceRenderer` / View IPC 为 `dataseries`；`get_data_series` / `put_data_series` / `data_series_page`；`data_series_nodes.rs` / `data_series_compare_nodes.rs`；节点分类与 Pin 名 `DataSeries`；前端 `DataSeriesSourceView`；运行时 ID 前缀 `data_series_*`（Polars `Series` 类型名不变）。
- [x] **变量类型新增 DataSeries**：`VARIABLE_SELECTABLE_DATA_TYPE_KINDS` 增加 `DataSeries`（与 `DataFrame` 并列，引用型、Detail 无手填值）；前后端 `DataValue` DTO 补充 `DataSeries` 往返，默认 `Null`，实际值由 Set Variable / 图执行写入。
- [x] 处理完毕 ~~int32，int64，float32，float64 在运算中的处理？？~~
- [x] **类型收敛：运行时数值仅 Int64/Float64**（对应 `## 2026.03.20` 的「类型收敛，使用 Int64 和 float64 代替所有的 number」）：后端 `DataType` / `DataValue` 移除 `Int32` / `Float32`，`FromStr` 与 `polars_dtype_to_data_type` / `polars_type_string_to_data_type` 将所有整数宽度（Int8~UInt64）收敛到 `Int64`、所有浮点（Float32/Float64）收敛到 `Float64`；删除 `convert_to_int32/float32`、`register_int32/float32_constant` 等；`math` / `distribution` / `dataframe` 系列节点改用 `DataType::number()` / `number_series()`；前端 `DataType` / `DataValue` 联合类型、DTO、Pin input、变量创建默认类型同步收敛；`VARIABLE_SELECTABLE_DATA_TYPE_KINDS` 只留 `Boolean/Int64/Float64/String` + 容器类型，新增 `DATA_SERIES_ELEMENT_TYPE_KINDS` 供 DataSeries 元素类型选择。
- [x] **DataView 表头显示列类型**：`DataTable.tsx` 自定义 `drawHeader`，表头两行展示「列名（加粗）+ 类型（弱化）」，`headerHeight` 44；移除冗余 `dtypeToIcon`。
- [x] **时间类型语义修复：新增 DataType::Datetime / Time（方案 A）**：修复此前 DuckDB / Polars `TIMESTAMP` 与 `TIME` 在图运行时被统一压成 `DataType::Date` 的语义丢失（对时间序列计量至关重要）。后端 `DataType` 新增 `Datetime` / `Time` 独立变体并补全 `Display` / `FromStr` / `default_value` / `is_primitive` / `is_comparable` / `can_convert` 与 `DataValue::coerce_to`；`database_schema.rs` 映射修正为 `Datetime(_,_) → Datetime`、`Time → Time`、`Decimal(_,_) → Float64`（保真层原始串 `Datetime(...)` / `Time...` / `Decimal(...)` 同步）；`pin.rs` 补 `datetime` / `time` pin type。前端 `DataType` / `DataValue` 联合类型、`dataTypeFromDisplayString`（含带参原始串归一）、`isPrimitiveType` / `getDefaultValue` / `dataTypeFromPinType`、DTO（`Datetime` / `Time` → 后端 `String`）、`optimisticNodeDraft` 穷尽 switch、`DATA_SERIES_ELEMENT_TYPE_KINDS`、pin 颜色（`time` 复用 `datetime` 色）全部补齐；标量层暂仍以 `String` 承载时间值，真实 Polars 时间类型由 `DataSeries<Datetime>` / `DataSeries<Time>` 携带。`cargo check --tests` / `cargo test --lib value::` 通过，改动文件 `tsc` 无新增错误。
- [x] **Decimal / NUMERIC 收敛到 Float64（图运行时 + 计量节点）**：确认 `database_schema.rs`（`polars_dtype_to_data_type` / `polars_type_string_to_data_type`）、`duckdb_reader.rs::duckdb_type_to_raw_string`（`DECIMAL` / `NUMERIC` → `"Float64"`）、前端 `dataTypeFromDisplayString`（`Decimal(...)` → `Float64`）映射已对齐。修复 VAR / VEC 节点中 Polars 数值列白名单硬编码（`Float32/Int32/...` 且不含 `Decimal`）导致 Decimal 列被静默跳过的问题：`var_nodes.rs` 改为 `is_var_numeric()`（`dtype.is_primitive_numeric() || dtype.is_decimal()`）；`vec_coint_nodes.rs` 同步；两处在 cast 到 `Float64` 前统一纳入 Decimal 列。
- [x] **DataSeries / DataFrame 变量 JSON 列式格式统一**：变量字面量 JSON 与 DataFrame 一致，采用 `{ "col_name": [values] }` 列映射（非 `{columns, data}` / index）；前后端 `variableValueUtils` / `tabular/snapshot.rs` 对齐；DataSeries 单列 `{ "col_0": [...] }`。
- [x] **DataSeries 预览去掉 `#` 索引列**：`source_builder.rs` / `window_data_store.rs` 不再注入后端 row index 列，UI 仅展示数据列。
- [x] **Array / Object 类型切换默认初值**：前端 `DEFAULT_ARRAY_VALUE` / `DEFAULT_OBJECT_VALUE` + JSON 编辑器模板；后端 `DataType::default_value()` 与 `project_state_variable` 类型切换测试（`[1,2,3]`、`{ key_0: 1, key_1: 2 }`）。
- [x] **Tabular 模块与变量结构化存储（`src-tauri/src/tabular/`）**：新增 `snapshot`（`TabularSnapshot`、JSON 一次解析）、`ref`（稳定 handle `var:{uuid}`）、`catalog`（`TabularCatalog` / `VariableTabularCache`）、`variable`（`normalize_variable_tabular` / `sync_variable_cache` / `display_data_value`）；`VariableInstance.tabular` 持久化结构化快照，`data_value` 存 `var:{id}` handle；`ProjectStore.variable_tabular` 缓存 schema + `Arc<DataFrame>`。
- [x] **合并 schema / variable 双 Provider**：删除 `VariableProvider`、`set_variable_provider`、`refresh_variable_schema_in_graphs`；`build_schema_provider` 统一解析数据集 id 与 `var:{variable_id}`；Get Variable `output_schema_resolver` 改走 `schema_provider(variable_handle_str(id))`，Decompose 等下游可正确拿到列 schema。
- [x] **图编译入口 `compile_graph` / `compile_graph_from_seeds`**：变量变更走 `recompile_graphs_for_variable`（增量 schema 传播 + 动态 pin 解析）；`command_variable` 移除重复 refresh，依赖 `update_variable` → `finalize_variable` → recompile 链路。
- [x] **执行期走 tabular 缓存，移除 JSON materialize**：`get_variable_value` 直接返回 `var:{id}`；`get_dataframe` / `list_database_columns` / `load_database_data_series` 支持 `var:` 从 `ProjectStore.variable_tabular` 读取；删除 `var_lit_df_*` 运行时临时 ID。
- [x] **变量 DTO 展示层 tabular ↔ JSON**：`VariableInstanceDTO` 读取时 `display_data_value()` 将 tabular 转回列式 JSON 供前端编辑器；前端无需改动即可 round-trip。

## 2026.07.04

- [x] **View：runtime source 生命周期**：已实现，见 [`docs/runtime-source-lifecycle.md`](docs/runtime-source-lifecycle.md)（拓扑破坏按 pin 失效、Run 结束保留、窗口 unmount 释放 Window owner）。
- [x] **运行完毕后 backend source 缓存生命周期**：拓扑破坏（删 pin / 删节点）按 pin 失效；Run 结束保留；`markGraphDirty` 不再清 `pinResults`（Undo 恢复连线后结果保留）；窗口关闭释放 `SourceOwner::Window`。见 [`docs/runtime-source-lifecycle.md`](docs/runtime-source-lifecycle.md)。
- [x] **DeleteNodes / DisconnectPin / 粘贴结构性 undo**：事务化 undo；统一 `GraphUndoPatch` + `apply_graph_patch`；delete 不 resolve 邻居；capture 含闭包连线 + **`neighborNodes` 邻居 pin 冻结**；apply 先 patch 邻居 + Materialize 收尾；`remove_node_raw` 正确 disconnect。
- [x] 目前在执行完毕的动画状态下，第一次断开节点无法undo，修复
- [x] **Detail 点击驱动重构（取代推导优先级链）**：`editorStore.detailFocus` 统一承载 node / variable / data / event / function / worksheet / log 显式焦点；`resolveDetailTarget` 仅返回 `detailFocus`（log 需 `selectedLog`）；`detailFocusCommands.applyCanvasDetailFocus` 作为画布 Detail 唯一入口（blank-click / box-select / node-click）；删除 `syncDetailFocusToActiveGraph`、`syncDetailFocusToNode` 及旧推导 sync；TabBar / Sidebar / LogPanel / Worksheet / `closeGraphTab` 等各入口显式 `focusDetail`；Sidebar event/function 单击设 scope + Detail，double-click 开 Tab；补充 `detailFocusCommands` / `resolveDetailTarget` / `closeGraphTab` / `detailFocusScope` vitest。
- [x] **画布选区与 Detail 解耦**：`setSelectedNodeIds` 仅管选区；pointer up——空白单击 → graph Detail、框选有移动 → 仅单选时 node Detail、拖拽无移动 → node-click Detail；框选不自动切 graph Detail；多选拖拽 pointer down 不破坏已有选区（保留 multi-drag）。
- [x] **关闭 graph tab 后 Sidebar / Detail 布局不回缩**：`variablesGraphScopeId` 独立维护 Variables Local 作用域（tab 关闭后 Local 仍指向最近 graph）；`clearDetailFocusForClosedTab` 关 tab 时清匹配 Detail；`layoutStore.removeTab` 恢复 sidebar/detail/panel 固定尺寸；`SidebarCollapsibleSection` 无 `onAdd` 时保留 `size-6` 占位防 header 宽度跳动。
- [x] **Detail sash 拖动布局修复**：修复拖动时 Detail 被挤到左侧、右侧留空且 sash 只控制空白的问题；row 布局 `after` 面板用 `startSize - delta`；mouseup 仅清除被拖动面板的 inline flex（不再误清 center）；移除 `ChildWrapper` 与 sash 冲突的 `useLayoutEffect` 重复写 flex。
- [x] **Sash 架构收口（`sashResizeLogic.ts`）**：抽出 `resolveSashResizeTarget` / `computeSashSize` / `layoutNodeFlexStyle` / `attachSashDrag`；`Sash.tsx` 薄 UI 层；mousedown 一次性解析 resize 目标、松手 `computeSashSize` 提交 store；`LayoutNodeRenderer` 单一 `getChildRef` proxy、`ChildWrapper` 共用 flex 样式；补充 `sashResizeLogic.test.ts`。
- [x] **多节点拖拽性能（命令式预览，零每帧 React 重渲染）**：`dragPreview.ts` 从 gesture store 同步拖拽态；`useNodeDragPreview` 命令式更新节点 `transform`（rAF 循环防 React 覆盖）；`useEdgeDragPreview` 仅更新与拖拽节点相连的 SVG 边；`edgePath.ts` 共享贝塞尔路径；`Canvas` 移除 `dragDelta`/`dragNodeIds` 订阅；`getPinWorldPos` 回调内读 `getDragPreview()` 保持引用稳定；`Node`/`NodeContainer`/`CanvasNode` 移除 `dragDelta` prop；`useCanvasInteraction` 返回值 `useMemo` 避免 `useEditor` 每帧 bust cache。
- [x] **框选性能（selection session + 命中缓存 + 命令式 marquee）**：`selectionSession.ts` 承载框选 live 态（不再每帧 `setGesture`）；`selectionHitTargets.ts` 在 pointer down 一次性缓存节点 screen bounds + 纯矩形 hit-test（移除每帧 `getGraphById` / 全节点 `getBoundingClientRect`）；`useSelectionBoxPreview` 命令式更新选框 DOM；pointer down 延迟 `setSelectedNodeIds([])` 至 pointer up（避免框选开始即整图重渲染）；`CanvasOverlays` 移除 `SelectionRegion` gesture 订阅；补充 `selectionHitTargets.test.ts`。
- [x] **ResultSource Presentation 迁移**：后端 `SourceDescriptor.presentation`（`Inspector` / `Plot{chart}` / `Report{report}`）替代 `renderer` + 临时 `window_type` / `SourcePresentation`；节点 API 改为 `publish_plot` / `publish_report` / `open_registered_source`；`OpenSourceWindow` 事件携带 `presentation` + `windowTitle`；View 节点直接 `open_registered_source`；前端 `presentationRoute` / `plotTypeFromPresentation` 统一 Pin「查看」与执行自动开窗。
- [x] **窗口与模块命名整理**：`/database` + `databaseEditor`（DuckDB 编辑器 `DatabaseEditorWindow`）；`/inspect` + `sourceInspector`（`SourceInspectorWindow`）；`features/core/resultSource`（`UnifiedSourceView` / `SourceViewShell`）；`features/application/databaseEditor`；Info 内嵌 `SourcePreviewPanel`；WindowKind / i18n / `window_state.json` 键同步更新。
- [x] **执行可视化 cleanup**：移除 store 死字段（`currentNodeId` / `errorConnections` / `nodeDurations`）、`isExecuting` 死 UI 路径；`clearedVisualPatch()` 合并 reset 逻辑；`resolveTabId` 提取至 `canvasInteractionUtils.ts`；移除 `data-edge-from/to` 等遗留属性。
- [x] **C 节技术债清理完成**：删除 `useEditorInit`、`connections.ts`、`SettingsService`、`update_subgraph_io` stub、`syncFromBackend`（node registry）、`default_type_system_snapshot`；`useShemaStore`→`useSchemaStore`；`PinRuntimeState::from_instance` 保留 pin id；`window_data_store`→`result_source_store`；`ColumnInfo` 合并为 `ColumnInfoDTO`；移除 `executedNodes` 双写；`GroupContext`/`services` barrel 收口；`useGraphManagement` 统一 `uiStore.showToast`。
- [x] **B 节 Layout tab / 窗口 helper 收口**：`layoutTabQueries.ts`（`getLayoutTabById` / `locateLayoutTab` / `getActiveLayoutTab` / `getActiveLayoutTabAmongGroups` / `resolveEditorGroupId`）+ barrel `features/core/layout/index.ts`；close-tab / detail / menubar split / BottomBar / TabBar / layoutStore 迁移；`openDatabaseEditorWindow` / `openLogsWindow` + `windowLabels.ts`；Sidebar / menubar / LogPanel 去重。
- [x] **IPC 分层 + Toast 单通道**：`SourceService`→`services/resultSource`；`GraphService.renameGraphResource`；`pinViewTarget.ts`（core 纯逻辑）+ `pinViewActions.ts`（开窗/toast）；`openGraphInEditor` / `sidebarResourceActions` / `statsActions`；views 去直连 services 与 sonner。

## 2026.07.05

- [x] **删除未使用 hook `useEditorInit`**：`useEditorInit.ts` 导出但无消费者；init 已由 `appInitialization.hook.ts` 负责
- [x] **删除 node registry 冗余 sync**：`useNodeRegistryStore.ts` 的 `syncFromBackend` 从未调用；schema store 为唯一 loader
- [x] **删除未使用 utils**：`shared/utils/editor/connections.ts` 零引用
- [x] **删除 stub API `update_subgraph_io`**：`command_connection.rs` 注册为 no-op，应从 `lib.rs` invoke handler 移除或实现
- [x] **删除 dead export `default_type_system_snapshot`**：`graph/value/type_system.rs` 无引用
- [x] **清理注释死代码**：`type_inference_session.rs` 大段 commented `infer_incremental`
- [x] **文档/命名 cleanup**：更新过时 `editor/README.md`；重命名 `useShemaStore.ts` → `useSchemaStore.ts`；移除 `useGraphManagement.ts` 未使用的 `showToast` 参数
- [x] **后端类型/命名澄清**：`PinRuntimeState::from_instance` 可能错误生成新 PinId（`pin_runtime_state.rs`）；`window_data_store.rs` 重命名为 `result_source_store.rs`；`ColumnInfo` vs `ColumnInfoDTO` 合并
- [x] **executedNodes vs nodeStates 统一（低优）**：commit 后 `executedNodes` Set 与 `nodeStates.status === 'completed'` 双写；可统一为只读 `nodeStates`
- [x] **Settings 退出时重复 load/save**：删除从未被引用的 `SettingsService`（前端 settings 走 `settingsStore` + localStorage）；后端 `load_settings`/`save_settings` command 暂保留
- [x] **`GroupContext` / `services` barrel 收口**：`GroupContext` 从 application 与 core 双 barrel 导出；`services/index.ts` 仅 re-export 部分 service（缺 `projectService`、`worksheetService` 等），import 路径混用 deep path 与 barrel
- [x] **Layout tab 查找 helper**：重复 `findTab`（`closeGraphTab.ts`、`closeEditorTab.ts`）；重复 active tab 扫描（layoutStore、TabBar、BottomBar、detailFocusCommands 等）。提取 `getLayoutTabById` / `getActiveLayoutTab(groupId)` 至 `features/core/layout/`
- [x] **Viewport 持久化单入口**：`useCanvasInteraction.ts` 与 `useCanvasViewport.ts` 均调用 `ProjectService.updateCanvas`。提取 `persistGraphViewport(graphId)` 至 `features/core/viewport/`
- [x] **窗口打开 helper 收口**：Database（`sidebarUtils.ts` vs `useMenubar.ts`）；Logs（menubar vs `LogPanelContent.tsx`）。新增 `features/application/window/openDatabaseEditor.ts`、`openLogsWindow.ts` 等 typed helpers
- [x] **Canvas drop 逻辑合并**：`useCanvasDrop.ts` 与 `useCanvasOverlayHandlers.ts` 重复 VariableDropMenu / spawn 逻辑
- [x] **Graph 资源 CRUD 单入口**：`Sidebar.tsx` 直接调 `resourceActions`，`useGraphManagement.ts` 包装同一 API；Sidebar 应走统一入口
- [x] **后端 schema enrichment 去重**：`command_project.rs` L36–137 与 `application/database.rs` 重复 `database_display_name` / column DTO 映射
- [x] **后端 graph mutation 事件 helper**：`command_connection.rs` 中 `emit_pin_change_events → emit_inferred_types → emit_runtime_source_invalidation` 重复 ~5 次
- [x] **graph 事件 helper 迁出 command 层**：`emit_pin_change_events` 等定义在 `command_connection.rs` 并被 `command_node` / `command_pin` 等跨模块引用；迁至 `project/graph_events.rs` 或 `event/graph_sync.rs`，command 文件只注册 handler
- [x] **`load_graph` 跳过重复 `prepare_graph_runtime`**：`project_state.rs` 中图已在内存时仍 `insert_graph(existing)` 重跑 schema 传播与 runtime prepare；已有图且无 invalidation 标志时应直接返回
- [x] **connect gesture 统一 tab 解析**：`canvasPointerLoop.ts` connect 路径用 `activeTabIdRef.current`，pan/drag 已用 `resolveTabId`，可能 tab 不一致
- [x] **`useNodeManagement` 重复实例化**：`Canvas.tsx` / `CanvasOverlays.tsx` 与 `useEditorGroup` 重复
- [x] **variables 双源合并**：`useEditorGraphData.ts` 返回空 `{}`，真实数据在 `Variables`；Canvas 合并 `{ ...variables, ...Variables }`。删除 stub，统一从 collections 读取
- [x] **Presentation 子窗口架构统一（修复 OLS Summary 等 Info 窗「此窗口没有可用数据」）**：根因是 `useReleaseResultSourceOnUnmount` 在 React StrictMode remount 时过早 `release_result_source`，且 `InfoWindow` 按 `kind===json` 误判 inspectable、未走 `presentation.report` 加载链。改动：**Phase 1** `usePresentationWindowLifecycle`（Tauri `onCloseRequested` release，删 unmount release）；**Phase 2** `parseSourceIdFromLocation` + `loadPresentationWindow` + `usePresentationWindow`（按 `inspector`/`plot`/`report` 三分支 IPC）；**Phase 3–4** `PresentationWindowShell` + `SourceInspectorWindow`/`PlotWindow`/`InfoWindow` 薄壳化；**Phase 5** `reportViewResolver` + `ReportView`（`ReportKind → Component` 映射，删 InfoWindow heuristic 与 `SourcePreviewPanel`）；**Phase 6** 后端 `ReportKind` 细分 `binarySummary`/`iv2slsSummary`/`ivLimlSummary`/`praisSummary`，对应节点改 publish，`get_value` 对 `Presentation::Report` 返回完整 JSON

## 2026.07.06

- [x] **Editor 组合层瘦身**：`useEditor` / `useEditorGroup` 在同一窗口多次挂载（`EditorWindow.tsx`、`useProjectSync.ts`、Canvas 子树）；`useActiveEditorGroup` 在 state/actions/workspace 重复调用；`withCanvasInteraction: false` 仍是 band-aid。目标：引入 `EditorSessionProvider` 或拆分为 `useEditorTabs` / `useEditorCommands` / `useCanvasInteractionProvider`，仅 Canvas 挂载 pointer loop。涉及 `useEditor.ts`、`useEditorGroup.ts`、`useEditorState.ts`
- [x] **IPC / 分层边界统一**：`SourceService`（`features/core/resultSource/sourceService.ts`）直接 `invoke` 应迁至 `services/`；`resourceActions.ts` 的 `rename_graph_resource` 绕过 `GraphService`；Views 直连 services（`Sidebar.tsx`、`Workspace.tsx`、InfoView stats blocks）；`resolvePinViewTarget.ts` 在 core 层开窗 + toast，应上移到 application hook
- [x] **Toast 单通道**：`uiStore.showToast` → `Toast.tsx` → sonner 与 ~6 处直接 `import { toast } from 'sonner'` 并存（`useProjectPicker.ts`、`resolvePinViewTarget.ts` 等）。统一为 `uiStore.showToast` 或 `shared/ui/toast` 薄封装
- [x] **GraphDataStore flat mirror 退役**：`graphEntities[graphId]` 为权威，但 `nodes/pins/connections` flat mirror 仍被 ~15 处 `?? store.connections[cid]` fallback 使用（`graphDataStore.ts`、sync handlers、clipboard）。所有读写强制 graph-scoped API，删除 mirror 与 fallback
- [x] **后端 ProjectState 变异 API**：graph commands 直接锁 `project_data`（`command_node.rs` 等），与 `prepare_graph_runtime` / `compile_graph` 规则易漂移。引入 `ProjectState::with_graph_mut` + 统一 `GraphInstance::recompile(scope)` 入口（`project_state_graph_mut.rs`；`GraphRecompileScope` 覆盖 RuntimePrepare / Full / FromSeeds / TopologyEffects / InferOnly；graph commands 已全部迁移）
- [x] **后端 graph compile / rename 去重**：rename 收口至 `ProjectState::rename_graph` + 单一 command `rename_graph_resource`（含 unique name、持久化、`ResourceChanged`）；已删除 `rename_subgraph` 与 `ProjectService.renameSubgraph`；compile 已统一为 `GraphInstance::recompile`
- [x] **执行性能：`execute_project` 避免全量 clone**：`command_project.rs` / `project_execution.rs` 在 spawn 前 clone 整个 `ProjectData`
- [x] **InfoView 报告组件模板化**：13 个 `*Component.tsx` 重复 Suspense fallback、区块布局、IPC 编排（OLS/2SLS/LIML/Prais 等）。共享 `ReportLayout` + application hooks（如 `useStatsBlock`），组件只填 chart/table 插槽
- [x] **`graph_instance.rs` 拆分**：~1964 行 god module（CRUD / infer / schema / undo 混杂），是 command 层重复调用的根因之一
- [x] **`command_project.rs` 拆分**：~674 行混合 registry CRUD、项目 I/O、schema enrichment、execution、result-source commands；与 A5 ProjectState API 收口配合，按 domain 拆至 `command_project/`、`command_execution/` 等
- [x] **`command_hypothesis.rs` 业务下沉**：假设检验 parse → linearize → format H0/H1 → `yss_sci` dispatch 全在 command 层；应提取至 `hypothesis/` 或 `application/hypothesis.rs`，command 仅薄包装
- [x] **`canvasRef` / `viewportRef` 命名澄清**：canvas 栈中同名 ref 在不同层表示 DOM element vs `GraphPosition`（含 scale）；统一命名为 `canvasElementRef` / `viewportRef`，避免 gesture / pointer loop 误读
- [x] **框选 hit-target 与 viewport 变更不同步**：框选 pointer down 时缓存节点 screen bounds，缩放/平移过程中 marquee 命中可能偏移；需在 viewport 变更时 invalidate 命中缓存或框选期间锁定 viewport
- [x] **Summary 节点 Info / Inspect 双轨 source**：执行 `publish_report` 开 `/info` 报告窗（`window_{uuid}` + `Presentation::Report`）；Result output pin 保持 `emit_output` 注册的 runtime source（`Presentation::Inspector` + Struct JSON），Pin 查看走 `/inspect` + `JsonTreeView`。两套 source 不合并为同一 presentation
- [x] **`ReportSourceView` 报告渲染收口**：新增 `features/core/resultSource/components/ReportSourceView.tsx`（`ReportView` + 可选预加载 data）；`InfoWindow` 复用；`UnifiedSourceView` 增加 `info` renderer；`SourceInspectorWindow` 对误路由的 report payload 兜底展示
- [x] **`VarEigenvalueStabilityPanel` import 修复**：`VARStableChart` 从 `VARStableChart.tsx` default import，不再误从 `VarModelTable.tsx` 具名导入
- [x] **框选预览蓝点**：`useSelectionBoxPreview` mount 时先 `sync()` 再订阅，避免 0×0 marquee div 在 (0,0) 显示 accent 边框
- [x] **移除 Editor 自动打开首个 graph**：删除 `useAutoOpenFirstGraph` 与 import 后自动 open；用户从侧栏自行打开资源


## 2026.07.07

- [x] **View：Pin 结果搜索 + Inspector 窗口**：移除 Canvas Runtime Results embedded 浮层；Canvas 左上 Pin 搜索入口，筛选 node/pin 后打开 `SourceInspectorWindow`；统一 `openPinResultView` 出口；删除 `SourceViewLayout=embedded` 死代码；Detail 侧栏保留 pin 行「View」按钮，不做 inline 预览。
- [x] **View：Pin 搜索 UX 内联展开**：点击搜索图标 spring 展开为 input（同容器宽度动画）；再次点击图标 / Esc / 点击外部收起；input 与下拉列表合并为同一 bordered shell，避免错位；列表单行展示 `节点 · Pin`。
- [x] **View：Pin 搜索纳入 input pins**：`collectPinResultSearchEntries` 除 output runtime 结果外，枚举图中已连接且可解析上游结果的 input pin；列表 output 优先、input 其后；点击仍走 `openPinResultView` 打开上游 Inspector 数据。
- [x] **View：Pin 搜索列表滚轮修复**：Pin search 根节点加 `menu-container`，避免 Canvas 全局 wheel 拦截 `OverlayScrollbar` 原生滚动（后随 Canvas wheel 移除一并解决根因）。
- [x] **Canvas：移除全局滚轮平移/缩放**：删除 `viewportWheel.ts` / `attachViewportWheel`（`window` capture + `preventDefault`）；Canvas 不再响应滚轮 pan/zoom，保留中键/右键/Alt+拖拽平移。
- [x] **滚轮事件收口**：审计全项目无 `window`/`document` 级 wheel 监听；移除 Menubar / Sidebar / Detail 上仅为挡 Canvas wheel 而设的 `onWheel stopPropagation`；TabBar 拖拽时仅在容器 ref 上绑定非 passive wheel listener。
- [x] **Canvas：Ctrl/Cmd + 滚轮缩放**：新增 `canvasWheelZoom.ts` / `useCanvasWheelZoom`，监听绑定在画布根元素（非全局 `window`）；仅 `ctrlKey`/`metaKey` + wheel 以光标为中心缩放（0.1x~5x）；普通滚轮不平移；视口走 `viewportSession` 提交与 `persistGraphViewport` 持久化。
- [x] 给每一个节点都设置完整 Markdown 文档（含公式），点击节点时在 Detail 侧边栏展示（**builtin 139/139 已挂载、278 个 include_str 引用**）
- [x] **节点长文档（批次 1：OLS 家族 + WLS/GLS/Predict）**：`catalog/docs/en|zh/` 新增 Configure、Fixed Scale/Cluster/HAC/Newey、VCE NonRobust/HC0–HC3、WLS、WLS Summary、GLS Configure、GLS、GLS Summary、Predict；`docs/{ols,wls,gls,prediction}.rs` + 各节点 `with_documentation` 挂载；Detail `NodeDocumentationPanel` 渲染 KaTeX 公式。
- [x] **节点长文档（批次 2：Logit/Probit/IV/Prais/Panel）**：Logit & Probit（Configure/Summary/Predict）、IV:2SLS Configure & Summary、IV:LIML Summary、Prais 三节点、Panel Configure / VCE Cluster / Panel Summary / Panel DID (TWFE)；`docs/{logit,probit,iv,prais,panel}.rs` + 扩展 `prediction.rs`。
- [x] **节点长文档（批次 3：VAR/ADF/VEC + DataSeries/Align/Plot）**：VAR Summary & varsoc、DF & ADF & Summary、VEC & VECRANK；Get DataSeries / Int Range / Length / Sum / Mean 与 6 个 Compare；TS Align/Diff/Pct Change/Rolling Mean/Lag、XT Align/Diff；Histogram/KDE/Line/Scatter/ECDF/Correlogram/Correlation Plot；`docs/{var,adf,vec,data_series,align,plot}.rs`。
- [x] **节点长文档（批次 4：Distribution/Math/Logic/Value/DataFrame/Control/Debug）**：23 分布 + 10 数学 + 5 逻辑 + 22 Value（Convert/常量/变量/Call）+ 7 DataFrame（Get/Decompose/Combine/Filter/Standardize/Dummy）+ Branch/Sequence/Event Begin/Print/View；`catalog/docs/en|zh/*.md` 外部 Markdown + `docs/{distribution,math,logic,value,dataframe,control,event,debug}.rs` 以 `include_str!` 引用 + `apply_docs` 挂载（**不再使用内联 Markdown**）。
- [x] **节点长文档（批次 5：收尾与质量）**：① 批次 4 迁回 `catalog/docs/en|zh/*.md` ✅；② 批次 4 + 批次 1–3 薄文档扩写（Pin 表、公式、Convert 类型表等）✅；③ `nodeDocumentation.test.ts` 中英回退与 KaTeX 样本断言 ✅；④ **移除节点短描述层**（后端 `localized_description` / `with_localized_description()` / catalog `*_DESC_*`；前端 Detail 仅 `documentation` → 实例 `description`）✅；⑤ 删除 `scripts/audit_node_docs.py`、`.github/workflows/ci.yml` 与 `npm run audit:node-docs`（不做文档 CI 门禁）✅。
- [x] 节点短描述层已移除（原 `localized_description` + Detail fallback 几乎不可见，与 Markdown 长文档重复）
- [x] **ActivityBar 节点目录（Sidebar Nodes）**：ActivityBar **图与变量之间**新增「节点」Tab；Sidebar 按 category 展示全部 **builtin** 节点（`features/domain/nodeCatalog/buildBuiltinCatalogItems`）；**拖动**到画布走现有 `NODE_TEMPLATE` / `useCanvasDrop`；**单击** Detail 展示 `NodeDefinitionDetailPanel`（Pin 规格 + Markdown 文档，无需图中实例）；画布右键 `NodePalette` 保留 pin 过滤 + 变量/函数动态项（`buildContextualCatalogItems`）；共享 UI `NodeCatalogTreeView` + 虚拟列表，消除 `NodePalette` 与侧栏重复逻辑。
- [x] **节点目录 Sidebar UX**：搜索框置于侧栏**底部**（Popover palette 仍顶部）；分类行对齐 `SidebarCollapsibleSection`（chevron + 普通大小写）；节点行复用侧栏 hover/active token（`nodeCatalogLeafRowClass` / `sidebarItemIndent`），13px 字号与 `--sidebar-hover` 统一。
- [x] **Charts Worksheet 无法在主编辑器打开（Detail 仍有效）**：根因是从 Charts 侧栏点击时 `activeGroupId` 可能为固定 chrome（`sidebar`），`useOpenWorksheet` 误将 tab 挂到侧栏节点；新增 `resolveEditorTargetGroupId`（跳过 sidebar/detail/panel）+ 统一 `openEditorTab`（含从 chrome 节点 `moveTab` 回收）；`openGraphInEditor` 共用；Sidebar 去掉重复 `setDetailFocus`。

## 2026.07.08

- [x] **画布执行中断**：右上角运行按钮左侧新增「中断执行」；`ExecutionCancelRegistry` + `cancel_execution` command + `Executor` 协作式取消（帧间检查）；前端 `cancelGraphExecution` / `interruptExecution`；与回放 `stopReplay` 分离。
- [x] **控制流节点 Phase 1（Do / Merge / Sleep）**：`Do`（In→Out 透传 exec）；`Merge`（多路 exec 输入汇合为 Out）；`Sleep`（`Duration` 秒，上限 60s，同步 `thread::sleep`）；`catalog/control` + `docs/en|zh` + 单测。
- [x] **控制流节点 Phase 2（For Loop / Switch）**：`For Loop`（`Count` + Index + Body/Completed，`ExecutionEffect::Loop` + `loop_counters`）；`Switch`（`Selector: Int64` + Case* + Default，`ExecRole::Cases(usize)`）；文档与单测。
- [x] **控制流节点 Phase 3（While Loop）**：`While`（`Condition` + `MaxIterations` 默认 1000 + Body/Completed）；`On Error` 待错误传播模型定型后再做。
- [x] **执行器等待协议根治（join 作用域 + 子任务计数）**：用显式 `join_target` / `pending_children` / `WaitKind` 替换隐式 `parent_frame` + `has_active_children` 扫栈；`ExecutionStack` 拆为 `frames` + `ready`；执行器收敛为 `spawn(+1)` / `complete(-1)` / `resume` 三单点；删除 `Suspend`/`ResumeToken` 死代码及 `insert_at`/`trigger_output` 等冗余；修复 Sequence×For/While 嵌套时 Then1 未跑完就执行 Then2；`logic_test` 新增 `test_sequence_waits_for_for_loop_before_next` / `test_sequence_waits_for_while_loop_before_next`。
- [x] **View 快照与死代码清理**：View 改为 `ensure_view_source_for_input` 每次发布 `window_{uuid}`；删除 `open_registered_source` / `SourceAction::OpenExisting` / executor 对应分支；测试 `RecordingEmitter` + `WindowSourceEmitter` 合并为 `CapturingEmitter`。
- [x] **执行前清空运行期状态**：`GraphRuntime::reset_execution_state()` 清空 `pins_runtime_state`、`loop_counters`、`ExecutionDataStore`；`Executor::run` 每次执行前调用；删除从未读写的 `nodes_runtime_state` 及 `NodeRuntimeState`/`NodeState`；`logic_test` 新增 `test_rerun_clears_pins_runtime_state_and_reexecutes_data_nodes`。
- [x] **运行结果清除（Clear）**：重跑 = 替换（`reset_execution_state` + `clear_runtime_graph` + 前端 `startExecution` 清 artifacts）；工具栏新增 Clear（`VscClearAll`）手动清当前图 runtime pin sources + 前端 `pinResults`/回放/动效；`window_*` 快照保留；`clear_graph_execution_artifacts` command + `clearGraphRunArtifacts` store 单点；取消执行同步清 partial artifacts。

## v1.0 待办

- [ ] 点击更新会自动更新
- [ ] **多数据库 DataView 直接编辑行定位抽象**：当前项目内 DuckDB 持久化表用 DuckDB `rowid` 做分页/编辑定位；后续若支持 SQLite / MySQL 等外部数据库直接编辑，需要新增 `RowLocator` / `BackendRowKey` 类能力抽象，各 backend 明确自己的稳定行键策略（DuckDB `rowid`、SQLite `rowid` 或主键、MySQL 必须主键/唯一键）；无稳定行键的外部表默认只读或先导入项目 DuckDB，避免把 DuckDB `rowid` 语义错误泛化到所有数据库
- [ ] **Worksheet 图表切 tab 性能优化（坚持 ChartViewModel 路线）**：不要全局把所有 tab 内容 hidden 保活；继续沿用当前 preview/data 缓存方向，把昂贵工作从 React mount 生命周期中移出。后续将 `WorksheetPreviewPayload` 细化为更完整的 `ChartViewModel`（缓存数据列、聚合结果、domain、ticks、legend/tooltip 元信息等），组件重挂载时直接复用模型；绘制层避免 `svg.selectAll('*').remove()` 全量重建，尺寸变化只重算 scale/位置，大数据 scatter/line 考虑采样或 canvas 渲染；缓存使用 LRU，并在 DataView 编辑、数据版本变化或 worksheet spec 变化时精确失效
- [ ] **变量类型切换时的值迁移 / 智能转换（暂缓）**：当前策略——切换类型且未显式提交新值时，重置为 `DataType::default_value()`；Array / Object / DataFrame / DataSeries 已用 JSON 列式编辑 + `tabular/` 存储（见 ## 2026.07.03 已勾项）。**暂不实现**跨类型自动保留或 coerce（如 Int→String、DataFrame↔Array、Object 字段映射等）；后续可考虑接入 `DataValue::coerce_to`、切换前「将丢失当前值」提示。变量类型不可选 Any。
- [ ] **View：大 Array 分页 tabular（暂缓，没有合适的前端表现）**：一维、同质、较长 Array 走后端 `getPage`（2 列 `#` / `value`，与 DataSeries 同 API 形状）；短数组 / 嵌套 / 异构仍走 `json` + `JsonTreeView`；需 `ResultSource` 或虚拟 tabular 存储与 builder 分支。
- [ ] **Struct handle → View JSON 架构重设计（暂缓实现）**：当前 `ExecutionDataStore` 仅存 `Arc<dyn Any>`，`DataValue::Struct` 的 `typeKey` 与 handle 分离；View / `build_struct_source` 只能事后按 `typeKey` downcast，临时用 `execution/struct_json.rs` 中央 match 表（OLSModel、OLSResult 等逐个注册）——每增 Struct 要改表，且 `typeKey` 双写，不可持续。**待选方向（均未定稿，先不实现）**：① **入库 JSON 快照**：`put_struct(type_key, T: Serialize)` 写入 handle 时同步 `view_json`，View 只读快照、Predict 仍 downcast；② **`dyn ViewPayload` trait**：handle 自带 `view_json()` + `as_any()`；③ **TypeId 注册表 + macro**：注册点贴近类型定义，替代 central match；④ **View 永不碰 handle**：所有 output 注册 source 时必须带 JSON（仍要解决无 upstream source 时的首次序列化）。实施前需统一：handle 层 vs `ResultSourceStore` 谁为 JSON 真源、不可 `Serialize` 的类型（如 `StandardizeTransform1D`）策略、与现有 `source_id` 复用链如何衔接。完成后删除 `struct_json.rs` 式 per-type 注册。
- [ ] **View 节点展示（续）**：核心 renderer / source 统一 / 子窗口 layout 已完成，见 ## 2026.07.03 未完成项（Array 分页 tabular、子窗口 chrome、runtime source 生命周期、Struct handle JSON 架构重设计）。
- [ ] 函数图应该如何设计？？？设置不可删除节点，但是可以移动，如 event 中的 event begin，function 中的 inputs 节点 和 outputs 节点？？或许 function 中不应该使用节点形式？？在这里还需要对 event begin 屏蔽，同理 event 需要对 inputs 节点和 outputs 节点屏蔽
- [ ] **节点长文档（函数图 / 未来节点）**：Function 图 Inputs/Outputs（及 Event 与 Function 互斥壳层）设计落地后，补充对应 Markdown 文档（见 v1.0「函数图应该如何设计」）。
- [ ] 复制粘贴撤回逻辑的快捷键效果有问题
- [ ] 值类型处理
- [ ] 还有 7 个组件属于「壳统一了、内部还没拆干净」，优先级建议：VEC → Panel → DID → VARSoc → DFADFSummaryList → VecRank → DFADF。
- [ ] 优点：window_* 是「当时那一刻」的不可变快照，重跑不会误改已打开窗口里的内容。代价：不关窗时会累积（For 循环多次 View 会留下多个 window_*），直到关窗或 clear_all。文档里提过 Window LRU/TTL，尚未实现。
- [ ] **On Error / 错误传播（待设计）**：MaxIterations + loop_counters + 执行前清空已落地。错误模型仍停在「节点失败 → 记日志 + 发事件 + 整图 has_error」，没有可连线的错误传播；要做 On Error 需先定：错误是否中断下游、是否进专用 exec pin、与 Loop/Sequence 如何交互等，再扩 `ExecutionEffect` 和 executor。



# TODOLIST

绘图组件库需要重构

xt align 进行对齐，在这里是不是可以于 ts align 可以共用呢

感觉 faer 计算得很慢呢，是不是没有开启并行的原因，4500*17 维度需要耗时 1600ms 有点儿久

然后就是真的需要 polars-dtype 这个 crate 吗？我感觉 polar 应该包含了吧， ai 是不是弄错了

polars 的 csv 有意思，with_try_parse_dates 可以检索日期，没有检索 categories 的

现在节点分类特别混乱

gls 的 data input pin 中我认为可以设置为 matrix，这就意味着需要在值系统中添加并定义 matrix 类型，目前是 dataframe

wls 和 gls 的 predict 节点有问题，目前 wls 报错：Node 6b7c0693-8d92-4c76-a253-6d49333221ab failed: Predict: Model input is not connected or invalid


HAC 已实现（Bartlett、Parzen、Quadratic Spectral kernel，lag 参数）；hac-panel、hac-groupsum 尚未实现。

目前使用 fixed scale 并没有提供 scale pin ，同时在 ols configure vce 可接受的结构体中删除掉 hac-panel 和 hac-groupsum

对了 schema 中关于 vce 的要删除，暂时没有用到

OneOf 还是使用 Restriction + TypeVar 来做类型推断之类的东西？

oneof 类型连接其中一个类型的 pin 的时候，会变成该类型；（这个效果是好还是不好？很难知道）

我觉得 plot 可以学习 seaborn 来进行参数选择和绘制

catelog 中的 plot（尤其） 和 distribution 中的内容需要大量重构

同时 plot 可以使用数据节点，然后数据节点需要使用 plot 的 show 节点才可以展示，然后 show 节点可以结合多张图的数据节点，在新的窗口中配置如何结合的信息。

复制粘贴节点以及项目的保存不够完善

直方图有问题，其显示将 null 值当作了 0 在图中处理，正常应该忽略

打开 dataviewer 后，在 editorviewer 中导入数据，dataviewer 并没有更新；

类型推断系统不够完善

type infer 使用 dirty 的形式推测，不要每次都推测全量类型

function, macro 功能添加（感觉没必要区分 function 和 macro ）

ols 节点的 evaltor 逻辑有巨大的优化空间

不如纯 data 节点在连接的时候就进行计算就好了

执行动画有一点bug，节点在获取数据执行其他节点的时候本身状态并没有执行完毕但是ui上显示执行完毕了

node 的 tooltip 功能，可以查看节点的信息

为了解决 dummy 问题，例如个体效应和时间效应的哑变量问题，可能需要在 dataseries 中添加额外信息进而去提出多出来的哑变量：计划做一个 add dummy info 节点，下方有个下拉列表或者文本框选择需要设置 dummy 的信息，仅对 dataseries 为 string 的信息管用。

面板数据，可能需要对 dataframe 数据类型也加上 info，来表示个体信息和时间信息 dataseries

未来可视化需要加上地图，地图应该使用下载的方式下载到数据库中，下载完毕后才能使用；

settingview.tsx 中，要注意区分，首先 dataframe 是必须要设置形状和颜色的，然后 array, dataseries 等复合类型应该是控制形状，基础类型控制颜色，同时基础类型应该是相同的形状，any 和 受限制的 any 应该是颜色，应该区分 all any, base any, part any

工具变量法：2sls

dataframe 抽样方法，是在 ols 配置还是 dataframe 层面配置呢

参数网格 -》 数据变换（异常值检验，对数变换等等操作） -》 检验结果 -》 存储

ols_summary 打开 ols_result_viewer 并返回 ols_result, 这里面存储了一些统计模型信息可以使用节点进行提取；这些节点应该是使用类似于函数或者其他功能的注册的方式而不是定义，不然后续结构体太多了这里会爆炸

ols model 可以引申出一个新的节点 predict，这个节点可以使用 endog, exog 两个玩意获得拟合值，然后真实值 - 拟合值可以得到残差。这是基本操作不应该删除

deserializeGraph 这个玩意是干嘛的，好多地方都没必要用他，感觉好卡

下面这玩意在软件退出时保存了两次

[12:43:12.734][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.734][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.735][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.736][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.756][BE][DEBUG] [APP] Settings saved successfully via backend
[12:43:12.757][BE][DEBUG] [APP] Settings saved successfully via backend
[12:43:12.757][BE][DEBUG] [APP] Settings saved successfully via backend
[12:43:12.758][BE][DEBUG] [APP] Settings saved successfully via backend


### 二、Pin 右键类型收窄

**目标**：用户可以右键点击 `OneOf` 类型的 Pin，将其收窄为某个具体成员类型。

**结构**：

```
PinInstance 新增字段:
  type_narrowing: Option<DataType>
```

**需要改动的位置**：

| #   | 位置                                         | 改动内容                                                          |
| --- | -------------------------------------------- | ----------------------------------------------------------------- |
| 1   | `pin_instance.rs` — struct                   | 添加 `type_narrowing: Option<DataType>` 字段                      |
| 2   | `pin_instance.rs` — `from_definition`        | 初始化为 `None`                                                   |
| 3   | `type_inference_session.rs` — `register_all` | 注册 Pin 类型时，若 `type_narrowing` 有值，用它覆盖定义中的 OneOf |
| 4   | 后端 API                                     | 新增命令：`set_pin_type_narrowing(pin_id, Option<DataType>)`      |
| 5   | 前端 Pin 右键菜单                            | 检测 Pin 定义是否含 OneOf → 生成收窄选项菜单 + "重置"选项         |
| 6   | 前端 Pin 类型显示                            | 收窄后显示具体类型，未收窄显示 `Float64 \| String`                |
| 7   | 收窄后触发                                   | 设置 `type_narrowing` → 重跑类型推断 → 检查已有连线兼容性         |

**优先级链**：`type_narrowing` > 类型推断结果 > Pin 定义默认值

结构估计

# 在正式打包分发前，可以等渲染完毕再显示窗口
# 在目前的开发环境中，不要这样做，可以取消 debug
# 项目未发布 不做任何迁移处理

每次更新版本都需要

```
同步改三处版本（例如 0.1.1）：

src-tauri/tauri.conf.json
package.json
src-tauri/Cargo.toml

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
- [ ] 类型收敛，使用 Int64 和 float64 代替所有的 number

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

- [x] ingest / reopen 写入稳定行键 `_yssbi_rowid`；`read_table_meta` / schema 对用户隐藏内部列
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
- [x] InfoView legacy 深色类名迁移为 shadcn token（`bg-card` / `bg-muted` / `text-foreground` / `border-border` 等；新增 `shared/infoViewTheme.ts`）
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
- [x] 节点短描述 i18n：后端 `NodeMetaData.localized_description { zh, en }` + `with_localized_description()`；catalog 全部 `.with_description` 已迁移（Get DataFrame、Event Begin、控制/逻辑/数学/分布/转换/回归/面板/绘图等）；Detail 优先 `documentation` → `localizedDescription` → legacy `description`
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
- [x] 旧 `.yssbi-event` / `.yssbi-function` 迁移：移除 legacy 读取分支（`node_instance` 完整 `definition` 回退、`ConnectionManagerLegacy` 四表）；保存即升级为新格式，并保留过渡期旧格式回退读取（`read_legacy_graph_document` / `from_legacy_graph_json`，全部项目重存后可删）
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

## v1.0 待办

- [ ] 点击更新会自动更新
- [ ] 变量切换类型 dataview 无法获取
- [ ] 断开连接后 pin 的状态有时还是连接状态
- [ ] 给每一个节点都设置完整 Markdown 文档（含公式），点击节点时在 Detail 侧边栏展示（**短描述 i18n 已完成**：`localized_description` 全覆盖；**长文档待补充**：目前仅 OLS / OLS Summary 有 `catalog/docs/` Markdown，其余统计/计量节点待批量编写）
- [ ] 类型推荐估计存在较大的问题（推断精度本身，与粘贴卡顿无关，待单独排查）
- [ ] **Detail 状态推导式重构**（减少 `activeTabId` 与 `selectedItemId/Type` 双份维护）：Detail 按优先级推导显示目标——① 画布单选节点 → NodeDetail；② 否则若 `activeTab` 为 event/function/worksheet → 由 Tab 推导 Detail；③ 否则用 Sidebar 选中项（variable / data / …）；④ 否则空状态。Tab 型资源以 layout 为唯一事实来源，去掉 `syncDetailFromEditorTab` 等手动对齐；Sidebar / Log / Node 选择仍保留独立 Detail 目标
- [ ] 感觉 tooltip 太多了
- [ ] 我觉得在 tabs 中的所有窗口使用 hiden？ 进行隐藏？？ 不然每次打开都需要重新渲染？
- [x] 目前创建节点还是非常的卡顿（见 2026.06.28「创建节点卡顿根治」「从 pin 拖拽建节点」三条）
- [ ] 节点的 detail 信息布局很丑陋，input 和 ouput pins 需要重新设计调整

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

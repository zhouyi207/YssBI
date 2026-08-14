# 在正式打包分发前，可以等渲染完毕再显示窗口
# 在目前的开发环境中，不要这样做，可以取消 debug
# 项目未发布 不做任何迁移处理

每次更新版本都需要

由于历史代码重构原因，目前项目中存在许多的历史遗留代码，或多余或逻辑重复或实现低效；请检查整体项目，寻找出项目中的重复逻辑和未使用的逻辑，分析必要性，如果有更高效的更干净的架构请添加到 todo 的 v1.0 待办中，如果单纯的逻辑重复或者多余，也请添加到 v1.0 待办中

请分析这个问题有没有必要修复，如果有必要，则使用高效且干净的架构来执行这个逻辑，同时清除掉无效逻辑代码和重复逻辑代码

重复逻辑问题？无效逻辑问题？代码漂移问题？多事实源问题？代码冲突问题？无效函数问题？deprecated 兼容问题？

测试应该保护未来仍然成立的行为或架构约束，而不是永久证明某次历史重构确实做过。

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
- [x] Schema 与 Pin 解析架构原则（结构 / 信息 / 数据三层；connect 时链式传播 schema；schema 派生 pin 非 exec 动态）→ 见 [DESIGN_RULE.md §3.7](./docs/architecture/DESIGN_RULE.md#37-schema-与-pin-解析)
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


## 2026.07.14

- [x] 函数的 `FunctionSignaturePin` 结构化 `DataType`（与项目变量同构；见 Phase D + `functionSignaturePin.ts`）
- [x] 每次进入项目都会弹出多个 toast 当前编辑器图未能加载，请重新点击标签页或画布
- [x] detail 组件中的第一个气泡中列与列的间隙太大了
- [x] 日志 item 的 detail 信息中，我们需要将消息单独提取出来，做成类似于节点文档的那种形式

## 2026.07.15

- [x] **Executor 模块路径确认**：执行器已拆为 `execution/engine/executor/mod.rs` + `wire_events`/`data_inputs`；确认无残留 `executor.rs` 双份实现或 dead re-export，防止合并回潮。
- [x] 从 `4dd85f8` 起完成版本 0.2.6 → 0.2.7 的版本文件同步，并更新 Cargo lock、前端 package、Tauri 配置与应用链接。
- [x] 重构编辑器 Tab 与会话管理：引入独立 `editorTabStore`、编辑器会话命令/生命周期、图文档缓存与打开 Tab reconcile，统一关闭、激活、保存和拖放路径。
- [x] 完善侧边栏与 Tab 交互：扁平化 sidebar rows、分组展开状态、节点目录搜索/树视图、资源重命名、拖放预览和 Tab 插入预览，删除旧 Sidebar 虚拟列表与重复 UI 分支。
- [x] 增强编辑器与侧边栏拖放：统一 DnD contract、拖动预览、函数/图资源投放、编辑器分组拆分、跨窗口拖放策略和拖动到新窗口能力，并补充相关测试。
- [x] 将编辑器网格尺寸模型重构为 ratio weights，统一 split、maximize、sash resize、memento 和 layout persistence 的计算与恢复逻辑。
- [x] 增加 workbench layout reflow 与 maximize snapshot invalidation，处理面板可见性、Zen mode、编辑器分组恢复及 resize 后的布局重排。
- [x] 移除 Canvas viewport 中无效的 culling padding factor 和未使用 gesture 类型，简化视口更新与渲染参数传递。
- [x] 新增 OverlayScrollbar 拖动实现、指标计算、拖动状态管理与虚拟滚动测试，统一滚动条 thumb/viewport 同步逻辑。
- [x] 重构日志面板：使用虚拟列表、滚动/视口计算、日志类型 Tab、Popover 过滤器、详情行和面板工具栏，删除旧的单体日志列表与重复控制器逻辑。
- [x] 统一日志级别常量和类型安全，移除字符串字面量造成的 LogLevel/LogType 漂移。
- [x] 增强资源与图编辑器生命周期：后端资源事件精简、resource index coordinator、图资源拖放与加载同步、函数图和事件图状态恢复，并补充回归测试。
- [x] 动态 Pin 解析增加类型推断校验警告：后端返回 inference warnings，前端同步 GraphValidationWarning、Pin 类型状态和打开图时的警告展示。
- [x] 清理并重组项目文档：移除 Cursor 规则与旧 archive/ADR 入口，新增 `AGENTS.md`、文档分类目录、版本历史索引和 RAG 检索说明。
- [x] 更新默认深色主题为 Codex 风格中性深灰配色，统一主题预设循环、状态栏主题显示与主题颜色契约测试。
- [x] 更新项目 demo 图片资源，替换为改进后的视觉示例。
- [x] 主题切换记录上次浅色/深色主题，顶部与 bottom 入口共享 `appearance.colorTheme`，切换时恢复对应模式的最近主题并删除重复保存逻辑。
- [x] 修复 Tooltip 在 Tauri 窗口拖动期间残留：标题栏拖动前清除焦点，拖动期间关闭并禁止 Tooltip 重新打开，释放或窗口失焦后恢复。
- [x] **Command 层结构化 `AppError`**：`project/` 有 `ProjectError`，但绝大多数 `#[tauri::command]` 仍 `Result<_, String>`（graph/dataframe/hypothesis/worksheet 等）；统一 `{ code, message, details? }` 可序列化错误，与前端 `formatErrorMessage` / toast code 对齐，替代散落 `format!` 字符串。
- [x] **前后端 DTO 同步流水线**：Rust 侧 `DatabaseDecl`/`DatabaseEngine`、`GraphInstanceDTO`、`SerialTestsResponse` 已 typed，前端 `DatabaseRecord = Record<string, unknown>` 与手写 `types.ts` 易漂移；评估 `typeshare` / `ts-rs`

## 2026.07.16

- [x] 统一后端类型兼容规则：将连接校验、类型推断、OneOf 细化和 Pin 手工赋值校验收敛到 `TypeSystemSnapshot::can_accept`，支持容器递归、`OneOf` 与 Struct 继承，避免 `DataType::can_accept`、推断层和命令层规则漂移。
- [x] 移除无效运行时类型绑定接口：删除 `NodeExecutionContextTrait::get_bound_type` 及永远返回 `None` 的实现，保留推断层真实绑定查询能力。
- [x] 清理连接相关旧 IPC 与前端包装：删除 `get_connections`、`delete_connections_for_pin`、`delete_connections_for_node` 及对应 `ConnectionService` 方法，保留当前 `connect_pins`、`disconnect_pin`、`delete_connection` 路径。
- [x] 清理项目加载旧 API 与 DTO：删除 `get_project_data`、`get_project_graphs`、旧 `ProjectService` 包装、只服务旧路径的 DTO 和无调用的项目引用校验模块，保留当前分阶段加载与按需 `load_project_graph` 路径。
- [x] 删除 deprecated/兼容残留：移除 `ReportKind::from_legacy_key`、domain 层 re-export 前端 state 类型、重复/无效 DTO 导出，并更新相关 barrel export。
- [x] 修正注释漂移：更新 DataFrame schema 同步、ProjectLoaded 事件和 Layout Tab memento 规范化说明，明确 Layout 兼容逻辑只位于本地布局恢复边界。
- [x] 统一前端包管理器为 pnpm：删除 `package-lock.json`，生成 `pnpm-lock.yaml`，保持 `packageManager` 与锁文件一致。
- [x] 补充本地前端类型验证入口：在 `package.json` 增加 `typecheck` 脚本，使用 `tsc --noEmit` 做本地验证，不接入 CI。
- [x] 收敛 Graph Node command 结构：将 `batch_create_with_connections`、批量创建 DTO、pin remap 和连接恢复逻辑从 `command_node.rs` 拆到 `command_node_batch.rs`，保留 IPC 命令名不变。
- [x] 清理批量创建连线重复逻辑：抽出 pin 映射、user value 恢复和 `ConnectionRestoreState`，避免创建阶段与重编译后动态 pin 补映射逻辑重复。
- [x] 清理执行期 data input 重复逻辑：抽出可拉取 data node 判断和 ready 后 data flow 发射 helper，保持逐边取数 / 流动行为不变。
- [x] 本地验证通过：`pnpm run typecheck`、`pnpm test`、`cargo check --manifest-path src-tauri/Cargo.toml`、`cargo test --manifest-path src-tauri/Cargo.toml`、`git --no-pager diff --check`。
- [ ] 删除掉 `atomic_enum`、`crossbeam`、`dashmap` 这些 crates，因为目前没有什么作用

## 2026.08.06

- [x] 完成 `node-architecture.md` 迁徙遗漏审计；当前仍存在以下协议、权威边界和运行平台收口项，全部完成前不应继续把对应 Phase 标记为 100%。
- [x] 修复数据库导入 publication delta 的资源 revision 错误：database mutation 不再误用项目级 `authority_generation` 作为 `toRevision`，改为按资源自身的 `fromRevision.next()` 连续推进，避免 CSV 导入已在后端提交却被前端以 `resource deltas are malformed` 拒绝。
- [x] 补充数据库 revision 回归覆盖：验证项目已有多次 publication 后，新数据库仍从资源 revision `0 → 1`，同时保持项目级 publication revision 独立递增。
- [x] 修复后创建独立 WebView 的项目 lifecycle 初始化：新增只读 `get_current_project_activation` bootstrap command，返回当前 `projectInstanceId`、`activationRevision` 与项目路径；`initProjectSync` 在本地 identity 为空时先接受真实 activation receipt，再加载权威项目快照。
- [x] 统一子窗口项目同步入口：`useProjectSync` 不再于 lifecycle 建立前直接执行 `reconcileProjectPath`，数据库编辑器与 Bayes 等晚创建窗口可正常建立 publication baseline，避免 `ProjectLifecycleError: project lifecycle changed before publication settlement`。
- [x] 补充 late-created WebView 与 Rust activation receipt 回归测试；`typecheck`、IPC/初始化审计、focused database/activation tests、`rust:check`、`rust:fmt:check` 与 `git diff --check` 通过。完整前端套件唯一超时项单独重跑通过；Rust 串行 lib 测试 1080/1083，通过项外的 3 项因 Windows 当前用户缺少 reparse point 权限（错误 1314）失败。
- [x] **Phase 1–3：修复 `GraphDelta` project-event wire 身份断裂。** Rust `EventProject::GraphDelta` 只发送 `{ delta }`，前端 `GraphDeltaHandler` 却强制读取 `payload.projectInstanceId`，导致真实跨窗口图 delta 被静默忽略；统一 envelope，并增加 Rust→TS project-event golden contract，禁止测试继续构造后端不会发送的字段。
- [x] **Phase 2–3：为全部 active-project 节点命令补齐后端可验证的 lifecycle identity。** `mutate_graph_document`、`update_function_signature`、`hydrate_editor_graph`、History 和 `execute_graph_document` 等命令必须显式接收并校验调用方 `projectInstanceId`；当前部分 TS 已发送但 Rust 未消费，另一些调用两侧都缺失，旧 WebView 请求可能落到替换后的新项目。
- [x] **Phase 3：删除前端函数签名到 Pin 的业务投影。** `functionSignaturePins` 仍在 React 侧执行 `type_name → DataType`、参数/返回值到 Pin、未知类型回退 `Any` 和固定 `Result` 命名；改为直接消费 Rust 权威 function/editor projection，项目加载、publication 与 recovery 不再重建 resolved interface。
- [x] **Phase 1–2：删除仍进入生产类型图的 legacy node creation/identity DTO。** 移除已无 Rust IPC 对应物的 `batchCreateNode.ts`、`nodeInstanceParams.ts`、禁用但仍公开旧类型的 `useNodeManagement.createNodes`，并清理 clipboard、旧 graph DTO 和 store 中的 `NodeInstanceParamsDTO`/显示名身份残留。
- [x] **Phase 2：收紧 raw `GraphDocument` mutation API。** `create_node`、`delete_node`、`bind_port`、`connect`、`disconnect`、`set_literal` 当前仍是生产可见 public 方法；改为 descriptor/validated patch 唯一生产入口，raw helper 仅限 document 内部或 `cfg(test)`。
- [x] **Phase 3：删除前端直接改写 graph projection 的 legacy reference cascade。** `projectPublicationMovePlan.prepareReferences` 和 `cascadeSubGraphPathInLoadedGraphs` 仍直接修改 `NodeData.subGraphPath`；资源移动只能安装 Rust 返回的 revisioned projection replacement，前端仅迁移 tab、viewport 等临时 UI state。
- [x] **Phase 5：让 `CompilationBasis.resource_versions` 只记录本次分析实际读取的资源。** 当前 compile snapshot 预先纳入全部函数、变量和数据库版本，无关资源 mutation 也会使 analysis/plan 过期，不符合计划的精确读取集与“无关 mutation 不失效”要求。
- [x] **Phase 5：将所有可由用户修复的 lowerability 错误前移到 Analysis。** 当前先生成 `ValidatedSemanticGraph`，再由 function ABI/lowering 发现 blocking diagnostic 并清空 semantic/plan；应在 semantic validation 前完成可执行性检查，使 lowerer 仅因取消、资源耗尽或内部错误失败。
- [x] **Phase 6：真正落实 demand-driven result publication。** Scheduler 当前为保留 operation 的全部 outputs 创建 result source 并发送 `ValueReady`，普通运行仍保留中间 Pin；仅显式 requested output/default result 可发布，Pin 预览继续走独立 demand。
- [x] **Phase 6/运行平台：实现协议声明的 `CachePolicy` 和默认 per-run memoization，或删除无效声明。** 当前 Catalog 声明 `PerRun`，但 plan/runtime 不携带或消费 cache policy，现有 activation 重复执行保护不是结果 memoization。
- [x] **Phase 7：补齐 relational island 与 native kernel 边界的 materialization adapter。** 按 `InputConsumption`/`OutputProduction` 插入 collect、buffer、spill、replay/stream bridge；当前 relational 输出统一转换为 fully-materialized scalar，consumer contract 尚未进入 scheduler/runtime。
- [x] **Phase 8：对齐计划中的 bounded stream/backpressure 与调度契约。** 当前 TODO 将 stream transport、deadline 和并行调度列为后续能力，但 `node-architecture.md` Phase 8/运行平台完成标准包含 bounded stream、backpressure、workload-aware parallel scheduler、timeout/retry policy；需要实现，或明确修订计划和 Phase 完成度，不能同时标记 100%。
- [x] **Phase 9：恢复完整 Catalog 搜索字段。** 前端搜索目前只索引 title/aliases，且测试明确排除 `technicalTerms`、稳定 `nodeTypeId`、后端 `searchText` 和可选 pinyin；应与计划一致支持当前 locale 标题、别名、技术词、资源名及可选拼音命中同一稳定 ID。
- [x] **Phase 9：补齐可配对、可计时的性能 trace span。** `SpanEvent` 目前没有 span identity、parent/span pairing 或 timestamp/duration，operation span 也缺少稳定 operation/activation 字段，无法计算 snapshot、analysis、lowering、run、resource acquire、cleanup 各阶段耗时。
- [x] **横切边界：移除第十个 `node_system/parameter_types/` 顶层目录。** 计划固定九个顶层所有权边界；将其中协议类型、codec 和 validation 归并到 `protocol/` 或明确领域模块，避免 analysis/catalog/compiler 共同拥有该边界。
- [x] **横切清理：删除 localization compatibility 双接口。** 合并 `LocalizationLookup`/`LocalizationBundle`，移除标注为 `Compatibility boundary` 的 blanket bridge，保持 0.x 项目单一路径。
- [x] **Phase 4/横切清理：删除 History legacy 默认解码。** `ProjectHistoryTransaction.persistence` 等字段仍通过 `#[serde(default)]` 接受旧 wire，且存在专门的 `legacy_history_transaction_defaults_to_in_memory_until_save` 测试；项目未发布，不保留迁移 shim。
- [x] **协议契约：补齐 execution 与 project-event 的 Rust↔TS golden coverage。** 冻结全部 `ExecutionDemandDto`、`RunEventKindDto`、`ExecuteGraphResultDto`、`GraphDelta`/resource mutation event envelope，并在前端增加严格 wire parser，避免手写 TS union 与 Rust enum 漂移。
- [x] **已完成——graph 与 worksheet 已在正确的资源架构层面统一**: 工作表中的 worksheet 的存储形式是使用目前这种形式好还是使用 event, function 形式要好，请分析：在这里我可以要求 name 禁止使用特殊符号
- [x] 测试中我认为不应该有软件的版本号信息，因为软件版本号会更新，请分析

## 2026.08.11

- [x] 优化图打开流程和 Canvas 就绪状态
  - 在显示标签页前恢复已保存的视口，避免图在首次渲染后发生跳动。
  - 等待图文档和投影都准备完成后再挂载 Canvas，避免先渲染空
    Canvas、随后再完整渲染一次。
  - 跟踪图加载状态，使加载失败时能够退出加载界面，避免标签页永久
    空白。
  - 在布局阶段初始化连线 Canvas，忽略临时的零尺寸，并在尺寸未变化时
    避免重置 Canvas 缓冲区。
  - 当 Canvas、节点或引脚尺寸无效时，不发布引脚偏移量。
  - 先加载新图，再在后台串行卸载旧图并执行 LRU 缓存清理，避免阻塞
    打开流程。
  - 防止较早失败的图激活请求覆盖较新的焦点图会话。
  - 保留 publication recovery 期间新打开的图投影。
  - Rust 后端对已缓存图直接复用投影，不再重新插入图、推进权威代次、
    使编译产物失效或触发重复编译。
- [x] worksheet 中的图表中的数据比如轴标可以被复制，请去掉这里的复制样式；同时日志中的文本请加上复制样式，包括点击日志中的 item 中在 detail 组件中出现的 消息里面的字符也需要可以拖动鼠标复制文本，方便 debug
- [x] 修复 worksheet 需要两次 ctrl + s 才能保存的 bug
- [x] 在这里 activitybar 为 图时 sidebar 中的函数列表中的item，activitybar 为节点时的 sidebar 中的节点 item，还有 acitvitity 为变量的局部和全局 item 还有数据中的数据 item 应该都是可以拖动的，可以拖动到 graph 中并创建相关的节点；而且在这里拖动的鼠标样式不需要巴掌，只需要移动到 sidebar 中的折叠按钮的样式就好了



- [ ] 在更改 graph 的时候 tabbar 中的样式并没有其他变化，如果在更改后不保存关闭，那么下次打开打开的时候还是更改前的状态，这里明显是不符合逻辑的，除此之外还有其他的需要检查；同时磁盘上以及更新的符号和标签我感觉可以去掉，可以学习 vscode 的 tabbar 处理
- [ ] 在前端中的 graph 中的 data pin 的类别都是 unknown，导致节点没有颜色，同时在 pin 的时候不会筛选节点，更不会自动连接节点，这个是需要修复的，可能需要完整的从后端发送类型过来避免字符串解析？这样会更加完整？这里需要仔细考虑
- [ ] 在 sidebar 中创建 item 的时候首先会出现在最下方然后根据 name 移动位置，能不能直接根据 name 出现在某个位置，忽略出现在下方的过程，这样不美观
- [ ] 目前后端节点的定义好像也不太清晰明了，需要讨论怎么处理
- [ ] 在 graph 中的右键菜单我希望根据 section subsection 等等分类，包括 activitybar 为节点的 sidebar 中的节点也是一样，这样如果不记得名称找起来非常方便，需要讨论
- [ ] 后续我会加入 mcp 功能方便 llm 直接调用统计方法获得数值结果，同时我会加入智能分析功能利用 llm 分析数值报告获得分析结果，在这里我初步的构想是在 activitybar 中添加一个报告的 icon，其对应的 sidebar 中显示各种数值报告的 item，然后 llm 可以对这些报告进行执行分析输出得到结果编写论文；你怎么看，目前怎么预留接口，前端应该如何设计等等需要仔细讨论和实现（在这里 data 中每一列我希望可以添加一个描述统计，意味着我们可以在导入数据的时候对 data column 添加一个文本标注，方便模型知道 data column 并进行描述统计分析一些有意义的结果并输出一些合理的假设）
- [ ] 关于可视化层面，目前可视化的图表还不够，我希望更加的丰富；并将这些图表组件化放置在一起，哪里需要就调用避免重复实现，差异较大可以分为两个组件
- [ ] 将过去的操作尽可能实现后归纳到 v0_0.md 文档
- [ ] 思考是否有必要多窗口进行跨窗同步，这样就不需要什么多进程的 token 了吧
- [ ] snapshot 有必要吗？？？？
- [ ] graph 分为两种，一种是纯计算 graph，一种是目前这种；纯计算 graph 使用 notebook 这种形式，修改节点会污染依赖该节点的下游节点，递归污染；运行到此节点可以做到将上游阶段全部干净，




## v1.0 待办

### 窗口跨窗同步

- [ ] 我想将 @glideapps/glide-data-grid 切换为 shadcn 中的 data table，主要是因为风格和组件和目前的 shadcn 组件不搭，同时在构建的时候还有一些其他的错误，如下。需要考虑替换的可行性；或许使用 Handsontable 替代（商用收费）

```
"/*#__PURE__*/"

in "node_modules/.pnpm/@glideapps+glide-data-grid@6.0.3_lodash@4.18.1_marked@4.3.0_react-dom@19.2.7_react@19.2_c19a5bde3a2383671a6324b7c97614b7/node_modules/@glideapps/glide-data-grid/dist/esm/internal/data-editor-container/data-grid-container.js" contains an annotation that Rollup cannot interpret due to the position of the comment. The comment will be removed to avoid issues.
node_modules/.pnpm/@glideapps+glide-data-grid@6.0.3_lodash@4.18.1_marked@4.3.0_react-dom@19.2.7_react@19.2_c19a5bde3a2383671a6324b7c97614b7/node_modules/@glideapps/glide-data-grid/dist/esm/internal/data-grid-overlay-editor/private/markdown-overlay-editor-style.js (2:13): A comment
```
### 口语化表达

- [ ] 点击更新会自动更新
- [ ] **多数据库 DataView 直接编辑行定位抽象**：当前项目内 DuckDB 持久化表用 DuckDB `rowid` 做分页/编辑定位；后续若支持 SQLite / MySQL 等外部数据库直接编辑，需要新增 `RowLocator` / `BackendRowKey` 类能力抽象，各 backend 明确自己的稳定行键策略（DuckDB `rowid`、SQLite `rowid` 或主键、MySQL 必须主键/唯一键）；无稳定行键的外部表默认只读或先导入项目 DuckDB，避免把 DuckDB `rowid` 语义错误泛化到所有数据库
- [ ] **Worksheet 图表切 tab 性能优化（坚持 ChartViewModel 路线）**：不要全局把所有 tab 内容 hidden 保活；继续沿用当前 preview/data 缓存方向，把昂贵工作从 React mount 生命周期中移出。后续将 `WorksheetPreviewPayload` 细化为更完整的 `ChartViewModel`（缓存数据列、聚合结果、domain、ticks、legend/tooltip 元信息等），组件重挂载时直接复用模型；绘制层避免 `svg.selectAll('*').remove()` 全量重建，尺寸变化只重算 scale/位置，大数据 scatter/line 考虑采样或 canvas 渲染；缓存使用 LRU，并在 DataView 编辑、数据版本变化或 worksheet spec 变化时精确失效
- [ ] **变量类型切换时的值迁移 / 智能转换（暂缓）**：当前策略——切换类型且未显式提交新值时，重置为 `DataType::default_value()`；Array / Object / DataFrame / DataSeries 已用 JSON 列式编辑 + `tabular/` 存储（见 ## 2026.07.03 已勾项）。**暂不实现**跨类型自动保留或 coerce（如 Int→String、DataFrame↔Array、Object 字段映射等）；后续可考虑接入 `DataValue::coerce_to`、切换前「将丢失当前值」提示。变量类型不可选 Any。
- [ ] **View：大 Array 分页 tabular（暂缓，没有合适的前端表现）**：一维、同质、较长 Array 走后端 `getPage`（2 列 `#` / `value`，与 DataSeries 同 API 形状）；短数组 / 嵌套 / 异构仍走 `json` + `JsonTreeView`；需 `ResultSource` 或虚拟 tabular 存储与 builder 分支。
- [ ] **Struct handle → View JSON 架构重设计（暂缓实现）**：当前 `ExecutionDataStore` 仅存 `Arc<dyn Any>`，`DataValue::Struct` 的 `typeKey` 与 handle 分离；View / `build_struct_source` 只能事后按 `typeKey` downcast，临时用 `execution/struct_json.rs` 中央 match 表（OLSModel、OLSResult 等逐个注册）——每增 Struct 要改表，且 `typeKey` 双写，不可持续。**待选方向（均未定稿，先不实现）**：① **入库 JSON 快照**：`put_struct(type_key, T: Serialize)` 写入 handle 时同步 `view_json`，View 只读快照、Predict 仍 downcast；② **`dyn ViewPayload` trait**：handle 自带 `view_json()` + `as_any()`；③ **TypeId 注册表 + macro**：注册点贴近类型定义，替代 central match；④ **View 永不碰 handle**：所有 output 注册 source 时必须带 JSON（仍要解决无 upstream source 时的首次序列化）。实施前需统一：handle 层 vs `ResultSourceStore` 谁为 JSON 真源、不可 `Serialize` 的类型（如 `StandardizeTransform1D`）策略、与现有 `source_id` 复用链如何衔接。完成后删除 `struct_json.rs` 式 per-type 注册。
- [ ] **View 节点展示（续）**：核心 renderer / source 统一 / 子窗口 layout 已完成，见 ## 2026.07.03 未完成项（Array 分页 tabular、子窗口 chrome、runtime source 生命周期、Struct handle JSON 架构重设计）。
- [ ] 复制粘贴撤回逻辑的快捷键效果有问题
- [ ] 值类型处理
- [ ] 还有 7 个组件属于「壳统一了、内部还没拆干净」，优先级建议：VEC → Panel → DID → VARSoc → DFADFSummaryList → VecRank → DFADF。
- [ ] 优点：window_* 是「当时那一刻」的不可变快照，重跑不会误改已打开窗口里的内容。代价：不关窗时会累积（For 循环多次 View 会留下多个 window_*），直到关窗或 clear_all。文档里提过 Window LRU/TTL，尚未实现。
- [ ] **On Error / 错误传播（待设计）**：MaxIterations + loop_counters + 执行前清空已落地。错误模型仍停在「节点失败 → 记日志 + 发事件 + 整图 has_error」，没有可连线的错误传播；要做 On Error 需先定：错误是否中断下游、是否进专用 exec pin、与 Loop/Sequence 如何交互等，再扩 `ExecutionEffect` 和 executor。
- [ ] 节点样式问题
- [ ] **CI 扩展：`cargo clippy` + integration tests 矩阵**：在 `cargo test` 之外增加 `cargo clippy --all-targets`（先 `yss-sci` 修 error 再全 workspace）；与前端 `typecheck` 并列，形成全栈静态门禁。
- [ ] **CI 门禁 `tsc --noEmit`**：`package.json` 增加 `typecheck` script，CI 与 pre-push 跑 `pnpm typecheck`（`noUnusedLocals` 已开，需防止类型债再次累积）。
- [ ] **CI 门禁：`typecheck` + vitest + `cargo test` 并列**：`tsc` 无法捕获仅运行时才暴露的 API 形参错误（如 `batchCreateNodes` 三参数旧调用）；`package.json` scripts 与 CI workflow 至少跑 `tsc --noEmit`、核心 vitest 套件、Rust integration tests。
- [ ] **OLS 取数「逐边」vs「批量」语义文档化**：当前执行器按边 `emit_data_pull` → 求值 → `emit_data_flow`；确认是否故意取代旧 NodeStart 批量高亮，并在 `TODO`/执行器注释中写清 UX 预期，避免后续误改回批量形式。
- [ ] uistyle 可能需要根据节点类型来进行重构
- [ ] 在 editor group 多个的情况下，刷新后回到了单个 watermake 界面，但是同时会出现警告：当前编辑器图未能加载，请重新点击标签页或画布
- [ ] 函数图层中 **递归 Call 编辑器提示**：`CallDepthGuard`（64）仅 runtime 报错；编辑器内对自递归/深链 Call 做静态提示（非阻断），与超限单测（见 Rust 复盘）配套。
- [ ] sidebar 内容中的 scrollbar 以及日志及其他组件内容的拖动逻辑有问题
- [ ] 剩余唯一标记是 Rust 执行上下文中的 get_bound_type TODO。它依赖尚未提供类型绑定状态的 GraphRuntime，当前直接返回 None 是明确的未实现能力，不适合通过猜测补丁，否则可能引入错误类型推断。
- [ ] **ACF/PACF 命令与 Plot 节点 DTO 对齐**：`plot/correlogram.rs` 输出 `CorrelogramDatum { lag, value, q_stat, p_value }`；`command_sci::compute_acf_pacf` + InfoView `ACFPACFBlock` 仅 `Vec<f64>` + `n`——复用 `cumulative_ljung_box`，扩展 `AcfPacfResponse` 或共用 `CorrelogramPlotData`，避免 Summary 图 tooltip 缺 Q/p-value（前端 `CorrelogramChart` 已按可选字段防御）。
- [ ] **Julia 第二个迁移目标选择**：ACF/PACF 已经有 `src/sci` API、Julia worker 和 Rust/Julia golden fixture 测试；下一步不要直接上 VEC/RE MLE/DID。优先在「serial tests / Ljung-Box / DW」和「描述性统计」里选一个做第二个 PoC：输入输出简单、能复用 Arrow IPC、容易与 golden result 对齐。简化 OLS 可以排第三步，先只做 `y: Float64` + `x: Float64 matrix` + `hasIntercept`，暂不碰公式、分类变量、robust/cluster/HAC。
- [ ] tolerance 和 num_traits
- [ ] bayes 中的有很多的 errors.push(error("PREDICTOR_REQUIRED", "预测表达式尚未解析或绑定。", "boundPredictor")); 后期都是要修复的
- [ ] bayes 中的 ast 感觉可以和 src 下的 ast 放置在一起，在这里好像有 latex -> json ast，json -> julia ast，normal formula -> json ast 等等 ast
- [ ] bayes 长任务的通知最好是作为复用模块
- [ ] Failed to install Juliaup: 找不到与输入条件匹配的程序包。安装不了 julia
- [ ] 在这里似乎日志类的测试感觉没有必要，可以直接删掉


函数和事件保持一致性的 API 重复层面：不影响编辑一致性，但维护成本高：

useGraphManagement 里 addEvent / addFunction、deleteEvent / deleteFunction 几乎镜像，底层已是 createGraphResource(kind) / deleteGraphWithConfirm(kind)
GraphResourceKind vs GraphResourceType 两处 type alias（sidebar / editor）
快捷键 Ctrl+N 仅新建 Event（产品选择，非 bug）
Menubar / Watermark 仍分「新建 Event / 新建 Function」两项（入口文案差异，合理）
若要进一步收敛，可以把 Session 对外 API 收成 addGraph(kind) / deleteGraph(kind)，Sidebar/Menubar 只传 kind，不再暴露四套函数名。

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

## node_architecture 进度



| Phase | 完成度 | 当前状态 |
|---|---:|---|
| Phase 1：身份、协议、Registry | **100%** | Stable IDs、strict executable Registry、typed fail-fast built-in assembly、provider provenance、legacy Registry/runtime removal、one-way editor projection parsing、Rust↔TS golden contracts、fixture immutability、whole-slice reviews and fresh full verification complete |
| Phase 2：GraphDocument 和事务 | **100%** | GraphDocument structural validation now gates file load, insertion, graph-carrying resource patches, lazy hydration, and prepared activation before authoritative effects; typed causes, descriptor-only production mutation, test-only raw helpers, projected-member atomicity, exact persistence precedence/OrderKey round-trip, stable metadata, unknown-node loadability, and MoveGraph zero-effect rejection all passed focused verification and whole-slice review |
| Phase 3：编辑器权威投影 | **100%** | Exact descriptor、revisioned command snapshot、coherent database recovery 与唯一 Core lifecycle authority 已完成；identity cycle/shim 已移除，AST-resolved service boundary 与 stale command/event/graph/publication 行为通过最终审查 |
| Phase 4：Rust 权威 History | **100%** | Rust 权威 History 已完成：direct/lifecycle graph cache unload 保留完整项目 History；unloaded Graph/Function/local variable 在单一 coordinator lease 下按需 hydration，并与 loaded/global resources 原子 Undo/Redo；policy/head/session/revision/residency races、rollback/recovery、精确 delta 与 post-finalize publication 均通过最终审查。History 仍为 process-local，project activation/reload/replacement 会清空 |
| Phase 5：确定性语义分析 | **100%** | Type/Schema/dependency analysis, compile publication, immutable resource facts, scope/parameter validation, and authority races are complete; 105 canonical typed compiler diagnostics now provide complete generated localization, stable named facts, canonical stored ordering, locale-independent snapshots, alias-resistant authority audits, and deterministic non-empty invalid-graph analysis across forward/reverse/seeded insertion histories |
| Phase 6：无环数据执行计划 | **100%** | Demand-driven roots 已完成：Stable GraphOutputRef、full-analysis/specialized-plan 分离、bounded DemandKey variants、pure/Call/structured/relational/resource 剪枝、canonical selection correlation 与顶层 Event Pin preview 均通过最终审查；前端 1137 项及完整串行 `pnpm verify` 通过。CachePolicy memoization、deadline、并行调度与 Filter/Project lineage 仍属后续 Phase 7/8 能力 |
| Phase 7：Relational island | **100%** | Project/Filter strict authority、Catalog/UI、typed schema、exact lowering、safe lineage、stable demand 与 DataFrame-native runtime 全部完成；真实 built-in Registry/database final/preview production chain、磁盘参数回读、内部 order/dtype/nulls、UUID determinism、取消/资源清理及同节点 ParameterizedStatic UI route 通过 whole-slice 最终审查与完整 `pnpm verify`，legacy external-mask Filter/Decompose 和外部 RuntimeValue 合约保持不变 |
| Phase 8：结构化控制与副作用 | **100%** | Branch、Loop、Call、Effect、显式 effect dependencies、64 层递归边界、独立 frame、carried values、typed cancellation、RAII 资源清理、最终化竞态及加载期 drain 均完成生产验证；独立架构审计与 fresh structured/compiler/plan/runtime/RunRegistry 串行矩阵通过，stream transport、cache、deadline 与并行调度属于后续运行平台能力 |
| Phase 9：Catalog、搜索、可观测性 | **100%** | Static/resource Catalog、current-locale title+aliases search/docs、database recovery、canonical delta strict-wire 与 legacy inference audits 已完成；frontend 287、database integration 11 及 Rust focused matrix通过最终审查 |


需要你确认的执行语义

我建议选择：

### A. Demand-driven（推荐）

- 普通运行只计算终端结果、effect 和跨 island 依赖；
- 中间 Pin 不自动物化；
- 点击 Pin 预览时单独请求该输出；
- compiler 根据 requested outputs 决定 roots。

### B. 所有 Pin 每次都可立即查询

- 每次运行自动计算并保存所有节点输出；
- 行为接近旧架构；
- relational pushdown 和大图优化空间明显受限。

### C. 用户配置

- 默认 demand-driven；
- 可将特定 Pin 标记为“始终保留”；
- 灵活，但第一阶段复杂度更高。

请选择 **A、B 或 C**。我推荐 **A**。

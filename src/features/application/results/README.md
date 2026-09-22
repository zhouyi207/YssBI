# Results application

> Status: Current
> Scope: 结果查询、分页、保留租约、报告数据与展示协调
> Canonical owners: Results Application 与查询服务拥有读取流程；Rust Execution ResultStore 拥有数据
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## Results

结果数据与缓存由 [Execution ResultStore](../../../../src-tauri/crates/yss-graph-execution/README.md#resultstore-and-cache-validity) 拥有；这里维护查询、租约与消费者的交付契约。

图会话回执随编辑投影一起携带 `resultState`，包含逐输出、逐连线有效性和执行会话内单调递增的结果 `revision`。编辑、打开、撤销、保存和项目快照通过统一图 store 原子安装这些事实；普通编辑不再先清空整图结果，再异步查询恢复。
`get_graph_result_state` 按当前语义 hash 返回缺失、过期或有效状态，仅有效状态携带当前结果 ID；运行通知和显式结果读取继续使用这一查询。查询重验资源依赖，不交付执行计划身份。协调器按图会话、代次与请求身份拒绝迟到回执，图 store 再按结果 revision 拒绝同一执行会话和语义身份下的旧结果。查询返回没有匹配语义快照时，不用空状态覆盖当前显示的完整图快照。
项目身份由查询协调器在发布前校验，不在只读结果投影中另存无人读取的副本；图状态请求仅携带参与校验的会话、代次和语义身份。
图运行展示复用 `createReadProjection`：依据统一图快照和执行 owner 按图派生，并复用未变化的节点、端口和连线展示引用。`useGraphResultPresentation(graphPath, selector)` 仅订阅消费者所需字段；分页等无关 payload 更新不重新构造图展示，重复终态不再发布结果投影。该投影没有业务写入接口或独立运行事实。
图发布入口在安装后显式对账结果查询：撤销旧请求，移除新摘要已判为失效的当前 Pin 绑定，保留仍有效的绑定与已有报告租约。卸载和项目结束显式释放对应状态；不再通过两个图 Store 的订阅互相触发结果清空和重查。
项目发布完成后保留同批安装的结果摘要。卸载由统一图 store 一次移除图会话、实体和摘要，`resetGraphResultQueries` 仅清理查询、当前 Pin 绑定与运行状态，保留仍被消费者持有的结果 payload。
运行事件的输出所有权和等待摘要标记保存在同一个不可变查询投影中。RunStarted 明确声明失效的输出在新摘要到达前不显示旧结果为有效；终态只结束运行标记，不撤销这一等待状态。匹配当前请求的结果摘要确认后再移除等待标记，其他分支保留展示引用。
同一投影还返回当前连线是否已被目标节点实际消费：新绑定、保有旧结果的过期绑定和有效绑定分别表示。
仅有输入的“查看数据”节点也参与此投影：Execution 将成功运行的查看记录及其输入依据附在共享结果上，
重验时同时检查当前绑定与源结果有效性。该记录不复制数据，也不增加结果持有者；新连线不能仅凭源 Pin 有缓存显示已执行。
前端以这份投影组合实际 RunStarted demand 和诊断，驱动连线、Pin 与节点样式；尚无匹配结果时显示未运行，不以内部计划缓存代替执行结果，也不再持有执行录制、回放或逐节点动画队列。
节点按有效输出数量显示完整或部分结果，仅有输入的查看节点按实际消费的连线汇总；运行效果遵守减少动态效果设置。编辑使本地运行请求失效，迟到回执不能覆盖后续运行。
真实桌面人工验收仍在[组件化计划](../../../../docs/roadmap/COMPONENT_REFACTOR.md#p0确认范围清理执行遗留并建立图状态契约)中记录，不以接口与测试具备代表该验收已完成。

Run admission 按 demand/DAG 得到实际重算的 operation outputs，在准备资源和计算前解除这些输出的旧结果绑定。
一个 operation 的所有当前 outputs 同时失效；未参与本次 demand 的输出保留当前结果。已打开报告的租约保留旧快照，
但不把它重新绑定为当前输出。成功 finalization 在同一写锁内发布整批结果，并验证每个输出仍属于该 run；
被后续运行或图失效淘汰的 run 不能重新发布。失败、取消不恢复上次成功的当前输出。

Frontend 通过 `get_pin_result(graphPath, output)` 查询当前 descriptor 或 null，descriptor/value/page queries 使用完整结果引用。
`RunStarted { outputs }` 公告当前输出的失效范围；完成后重查当前输出。Frontend 只撤销这些 pin 查询和未被读取组件持有的缓存，
不清空已打开报告或中断其分页/分析。Pin 查看与搜索只索引当前输出，按引用读取保留快照不会重写当前 pin 绑定。

关系和数列页面由 Application 捕获当前 Result，交给句柄在固定快照上执行有界查询；I/O 在计算线程、锁外执行。
每页最多读取请求行数加一行，通过额外行判断 `hasMore`；不为了预览执行整表 COUNT。`totalCount` 可为 null，
在能确认末页总数时返回精确值；列名/精确类型与行值一起交付。页大小和字节预算由 Result query owner 限制。
读取完成后再次检查 Application session 和 ResultId；失效结果的迟到成功/失败均不重新发布。
Frontend 在总数未知时照常请求首页，按后端 `hasMore` 翻页，在表格中显示列名；超出 JavaScript 精确整数范围的值以十进制文本显示。
Application Results 的 `ResultStructure` 统一决定 descriptor 和分页的结构、已知总数与列信息，IPC 只编码此投影。
物化数列和惰性数列均使用 `sequence` 表格协议：每行是单元格数组，每页携带列定义，每行长度等于列数。
物化列表的每个元素占一行一列，嵌套列表和记录保留为结构化单元格，不根据第一个元素猜测多列表格。
列名和语义类型取自输出契约与语义标注；只有 Numeric 契约时展示 Numeric，不猜测 Float64。
关系结果的精确物理类型取自适配器 Schema。延迟 Schema 的 descriptor 暂不列出列定义，由受控分页解析，
已确定 Schema 的分页须与 descriptor 一致。空表及全 Null 数列仍返回列定义；前端也请求零行结果的首页。
前端仅渲染分页提供的行和列，不补列、不包装行，不再保留独立的 DataSeries JSON renderer。
分页只用于数据表格和数列（包括报告内的观测表）；报告概览、图形和分析结果本身不分页。

Run event 使用实际 ExecutionSessionId 与 RunId 标识运行，执行会话重建后的计数重置不会混入旧运行。前端结果投影集中在 Application results 模块，Execution UI store 只保存运行状态、请求身份和 Output。分页缓存按完整结果引用、part、offset 和 limit 隔离，不同消费者翻页互不覆盖；Sequence renderer 统一提供表格与数列分页入口。读取组件持有 payload consumer lease，最后一个消费者释放时才清除本地 value/page；project reset 后的旧 lease 不能释放新项目的数据。主窗口和独立窗口的 Inspector 均由挂载的 renderer 读取数据，不预读后再重复读取。descriptor 仅表示可用结果，不携带 pending/failed/cancelled 状态；运行状态通过 Run event 投影表达。provenance 包含 RunId、输出地址与创建时间。

显式打开的 Result panel 和独立展示窗口绑定打开时的结果引用；节点删除、图语义修改、重跑均不会更换已有报告的数据。
Application 在创建面板前通过 `retain_result` 原子取得租约和 descriptor。租约 token 由调用方预先生成，
便于丢失响应后清理；Rust 确认结果和窗口身份，同 token 的重复申请/释放不会重复计数。
打开面板先解析结果引用，再使用租约回执中的 descriptor；不在申请租约前另行预读取或复制 descriptor。项目切换或面板打开失败时释放本次租约，已失效的结果引用不报告为布局故障。
FlexLayout 的真实面板集合驱动窗口内租约对账，移动、隐藏和 React 重新挂载不代表面板关闭；打开失败会释放申请的租约。

独立窗口使用指定接收窗口的租约交接：父窗口先保留结果，新窗口通过 `claim_result_lease` 原子接管，
不存在创建窗口期间无人持有数据的间隙。窗口销毁时后端清理所属租约及尚未接管的交接，并拒绝该 owner 的迟到申请；
独立结果窗口使用唯一实例 label。前端卸载事件不是唯一回收来源。
原生窗口创建以 `tauri://created` / `tauri://error` 为成功或失败依据；构造 WebviewWindow 对象本身不代表创建成功，
异步创建失败进入租约释放流程。几何插件按种类分组不会合并实例 label 或结果租约身份。
所有权操作按窗口串行协调，查询与统计计算不进入该队列；每次对账只遍历该窗口的租约。

删除、重命名或卸载图解除其缓存绑定，有租约的结果继续可读。语义编辑使受影响缓存过期，移动节点等布局操作保留有效缓存。
Pin 查询与当前结果搜索重新验证数据库内容及函数依赖；报告仍按打开时的完整引用读取。
编辑及资源版本变化撤销旧运行的发布资格，撤销恢复相同语义也不会恢复旧运行的资格。
执行会话结束（包括项目关闭、切换或会话重建）撤销所属租约并释放 ResultStore，
前端清理 project-scoped 面板并通知独立报告窗口关闭。跨窗口通道只通知会话结束，不再把输出失效广播成结果销毁。
旧会话引用不能读取新会话中的同号结果。运行失败摘要与 Results 的生命周期独立，清除 Output 中的错误不清除 Results。

统计摘要区分 `LinearModelInfo` 与 `BinaryModelInfo`。Logit/Probit 使用 `pseudo_r2`、`adjusted_pseudo_r2`、`lr_chi2` 与 `prob_lr_chi2`，不生成 F/Wald 别名或线性 ANOVA 的平方和字段。通用回归 envelope 只复用系数、诊断与检验输入。

线性回归统一使用 `yssbi.statistics.linear.fit`、`linear.summary` 与 `linear.predict`。Fit 在配置中选择 OLS/WLS/GLS，输出 `model`、`fitted`、`residuals`；Summary 只接收 `model`，无拟合参数，也不重新估计；Predict 使用同一模型的系数与截距。

统计目录的其他 Summary 同样只接收对应方法的已拟合模型。ADF 不属于模型汇总：`adf.test` 接收序列及检验配置，声明 `result` 和 `report` 输出；Graph 仅将其 `report` 分类为 ADF 统计报告，`result` 保持普通结果。ADF 及非线性回归的相关目录定义尚未注册执行内核，不能据此视为已贯通；当前定义与接入边界见 [Node Catalog](../../../../src-tauri/crates/yss-node-catalog/README.md)。
WLS 通过一个按需添加的 `weights` 数列接收正精度权重；GLS 通过按顺序添加的 `sigma` 数列接收完整相对误差协方差矩阵的各列，验证有限、方阵、对称与正定。只允许所选方法需要的辅助输入。WLS 权重与训练列联合读取以验证共同样本；协方差矩阵的行列顺序由调用者对应训练样本。GLS 当前仅支持常规标准误；其他标准误配置被拒绝。
执行计划保留端口实例分组身份和模板名，内核只接收中立模板名来区分 predictors/weights/sigma，不解析图地址。

Fit 的 `model` 与 Summary 的 `result`、`report` 共享不可变的原生 `LinearRegressionResult`，仍由当前 `ResultStore` 拥有。报告类别统一为 `linearRegressionSummary`，标题为 Linear Regression Summary，模型概览保留实际 OLS/WLS/GLS 方法。WLS/GLS 的平方和与 R² 使用变换尺度，拟合值与残差保留原始尺度。
拟合值、残差、设计矩阵与参数协方差留在 Rust；报告 value 的 `presentation` 提供带格式标记的模型概览条目、ANOVA 列和行、条件数指标，由 [报告展示投影](../../../../src-tauri/crates/yss-application/src/graph/results/report/presentation.rs) 从统计结果构造，数字不会预先转换为展示字符串。报告还包含完整参数名目录 `paramNames`、
`{ executionSessionId, resultId }` 引用，以及 coefficients/observations 表引用与行数。
引用的 part 是固定枚举，不是任意 JSON 路径；观测表把拟合值和残差按拟合时的行序配对。
`get_result_table_page` 复用有界页面投影；`analyze_result` 读取已选的 ACF/PACF、序列相关与假设检验结果，仅需分析 kind。计算参数只由 Summary 节点配置，不在读取请求中重复传递。
残差图请求保留显示范围与点数，按拟合值范围筛选、跨整个匹配总体进行系统抽样，并标明总体/匹配数和抽样状态。
统计检验使用完整拟合数据。Application 捕获共享结果后在锁外读取/投影，返回前重验 session 与结果可用性，
结果回收或会话结束后的迟到成功和失败均被丢弃；仅当前输出失效不撤销有租约的快照读取。
数据仍由原 ResultStore 拥有，不另建报告存储或复制完整数据。

Application 的 `query_result_projection` 返回有界强类型投影；普通对象在克隆或编码前检查展开深度和空间预算，原生报告仅展开概览、参数目录与表引用。共享协议映射 `result_encoding` 供 IPC 与 Harness 调用，统一字段、宽整数文本与内联 JSON 字节上限；计数写入器不分配完整编码缓冲区。DataFrame、DataSeries 和顶层内存数列走现有分页入口；AI 不另维护统计字段白名单。AI 的 JSON 与表引用读取语义见 [Harness capability 契约](../../../../src-tauri/crates/yss-harness-core/README.md#4-registered-capabilities)。

线性回归报告的 `summary` 携带本次执行的内容选择和检验参数。Summary 执行阶段只计算选中的附加检验；报告查询返回已计算的检验，拒绝未选项和 Fit 模型。Fit 的普通 Inspector 保留模型概览，不伪造默认 Summary、分析能力或报告表引用。概览与报告共用 `query_result_projection`，不另设线性报告查询入口。系数读取只在选择系数表、系数图或方程时挂载；选择图形/观测表后仍在展开时读取有界投影。

`addLinearSummaryContents` 将 Report 的补选提交为来源节点的普通 `setParameters`，保留当前其他参数，再定向执行该 Summary 输出。上游模型有效时由既有缓存复用；模型输入变化时从当前图执行，最终以完整新结果替换报告。节点删除、项目切换、并发编辑或计算失败保留旧报告，不把新增分析装入旧结果。配置已提交但计算失败时仍可在节点修正或从报告重试；编辑不隐式保存图。
显式补选成功后，工作台通过 `replaceResult` 原子更新原面板的结果引用和租约，移动/重新挂载继续显示新结果；普通重跑仍不会替换已打开报告。独立窗口在当前视图内持有新快照租约。查询和租约继续由既有 Results owner 管理，失败和迟到结果释放临时持有关系。
前端复用 Result query coordinator，以执行会话、结果和完整查询语义隔离请求与缓存：分页包含 part/offset/limit，已计算的分析按 kind 区分，残差图还包含显示范围和点数。
相同不可变结果查询共用在途请求；不同查询参数独立发布。独立窗口及面板的 descriptor/value 同样经协调器读取与安装，并由挂载方持有 payload lease。Plot/report 仅接受完整 scalar payload；数列仍可在 Inspector 中分页，不能把第一页伪装成完整绘图数据。
展示加载器统一完成 Inspector、plot 和 report 分流。`UnifiedResultView` 只呈现已分流的 Inspector 标量或数列；报告组件必须接收加载器交付的数据，不保留报告回调、JSON 树回退或组件内的第二次结果读取。
消费者用自己的请求代次控制迟到回执和 loading；value/page/analysis 分别订阅数据和错误。最后一个 payload consumer 释放、结果回收或会话结束会清理这些投影。
分页 Hook 统一交付 `rows` 和 `pageSize`，不再并列暴露无消费者的原始 `values` 与 `limit`；底层分页请求和 wire 字段仍由查询协议拥有。
假设检验的参数提示读取 Rust 提供的完整 `paramNames`，与系数表当前页无关。独立窗口的图类型直接读取 descriptor，不通过 URL 传递副本。
展示窗口从 presentation kind 一次映射到 inspect/plot/info 窗口类型，并据此生成路由，不接受任意路由字符串或未知路由回退。
报告字段的结构不再随观测数增长，也不通过大 scalar 的分页回退搬运完整数值数组。

报告的字段结构由各 `parseCommon`、`parseRegression`、`parseVar`、`parseVec` 和 `parsePanel` owner 校验，复用 typed field reader，递归检查数组、矩阵和可选诊断块。`parseReportPayloadResult` 单次读取返回已校验的值或字段路径错误；OLS 的必需统计字段和标题要求在同一解析路径内表达。非法嵌套内容不能通过强制类型转换进入 renderer。

内联回归报告校验拟合值与残差、滞后残差对、设计矩阵与系数的维度；IV 过度识别检验按 `test_type` 要求对应统计量和 P 值。缺失的可选指标显示为空缺，绘图与检验不会用零补齐不完整数据。

报告组件与 JSON 布局见 [Results views](../../../modules/results/README.md)。

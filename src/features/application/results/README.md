# Results application

> Status: Current
> Scope: 结果查询、分页、保留租约、报告数据与展示协调
> Canonical owners: Results Application 与查询服务拥有读取流程；Rust Execution ResultStore 拥有数据
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## Results

结果数据与缓存由 [Execution ResultStore](../../../../src-tauri/crates/yss-graph-execution/README.md#resultstore-and-cache-validity) 拥有；这里维护查询、租约与消费者的交付契约。

`resultProjection` 持有唯一可写的 Zustand 查询缓存、读取接口和数据移除规则；`runtime` 编排查询发布、
运行通知及图状态对账；`graphPresentationRead` 从现有图与 Execution owner 派生稳定的图展示引用。
运行开始、图卸载、当前结果替换及图摘要对账在同一次 Immer 更新中发布本 owner 的相关字段，
不逐输出通知部分失效状态。请求取消仍归查询协调器；有消费者持有的旧数据与未变化分支继续共享原引用。
value/page/analysis 发布只更新对应条目与错误键，不为移除一个错误重建整个错误表。

当前 Pin 与结果搜索共用 `createReadProjection` 派生的可见绑定表，同时订阅查询缓存、图结果摘要和执行输出归属。
已加载图的绑定必须匹配当前语义身份、执行会话、有效输出及 ResultId；新图帧发布后即隐藏旧绑定，
不等待后续请求撤销和缓存清理。相同可见绑定继续共享表引用，摘要仅修订号变化不重复通知；
清理已隐藏绑定或更新无关 payload 时，当前 Pin 的 selector 保持原值。
命令式 Pin 读取直接重验当前源状态，保证同步图监听器不受只读投影监听顺序影响。
输出索引按已发布且不可变的 `outputs` 数组用 WeakMap 复用，接纳、读取与对账使用同一索引；
selector 不逐 Pin 扫描整个摘要。未加载图的当前输出查询继续由 Rust 回执提供事实。
按完整结果引用读取的 descriptor、数据和报告租约保持原生命周期，不因当前 Pin 被隐藏而删除。
该读取入口也按图索引有效 descriptor 并派生搜索条目；`usePinResultSearchEntries(graphPath)`
只读取对应数组，同图分屏共享投影，消费者只过滤自己的查询文本。标签直接按节点/端口 ID
读取现有实体表，不另存可写标签或订阅整个图桶。未变化的图、条目与数组保持引用，位置和
无关实体字段变化不重建搜索对象；没有有效绑定的图释放其搜索缓存。绑定过滤、搜索移除及
标签变化在同一只读发布入口交付，保留原语义身份、执行会话和输出归属检查。

图会话回执随编辑投影一起携带 `resultState`，包含逐输出、逐连线有效性和执行会话内单调递增的结果 `revision`。编辑、打开、撤销、保存和项目快照通过 ResourceStore 与资源标记一起原子安装这些事实；普通编辑不再先清空整图结果，再异步查询恢复。
`get_graph_result_state` 按当前语义 hash 返回缺失、过期或有效状态。有效状态携带当前结果 ID，过期状态携带仍保留的上次成功结果 ID，缺失状态没有 ID；运行通知和显式结果读取继续使用这一查询。查询重验资源依赖，不交付执行计划身份。协调器按图会话、代次与请求身份拒绝迟到回执，ResourceStore 再按结果 revision 拒绝同一执行会话和语义身份下的旧结果。查询返回没有匹配语义快照时，不用空状态覆盖当前显示的完整图快照。
项目身份由查询协调器在发布前校验，不在只读结果投影中另存无人读取的副本；图状态请求仅携带参与校验的会话、代次和语义身份。
项目结果重置在查询投影同步通知后重验原项目身份与执行会话；监听器建立后继项目或执行会话时，旧入口停止清理运行记录和发布旧事件，不覆盖后继会话。
图摘要发布和图帧对账在执行会话准备、资源摘要发布、运行清理及 Pin 失效通知后重验原项目和图帧身份。
对账还绑定原结果摘要与执行会话；监听器更换这些依据后，旧流程不再发起 Pin 查询或撤销后继请求。
保存锁变化不改变图帧身份，不会阻断仍有效的结果对账。
空 Pin 回执和运行失效通知之后的摘要刷新同样绑定通知前的项目、图帧、结果摘要与执行会话；
依据被监听器替换后，旧通知不再取消或重新排入后继摘要查询。
图运行展示复用 `createReadProjection`：依据统一图快照和执行 owner 按图派生，并复用未变化的节点、端口和连线展示引用。`useGraphResultPresentation(graphPath, selector)` 仅订阅消费者所需字段；分页等无关 payload 更新不重新构造图展示，重复终态不再发布结果投影。该投影没有业务写入接口或独立运行事实。
结果有效性汇总与运行外观分别派生：`outputs / connections` 只读数组或待确认失效范围变化时才重建节点、输入、输出及连线汇总；只改变结果 revision、运行节点或错误信息时复用这些表，再组合当前运行外观。图语义身份过滤仍在现有读取入口完成。宽的输入、输出标量索引作为独立只读发布根冻结，外层投影更新可直接跳过其遍历；缓存随原按图生命周期清理。分配与耗时可用 [Graph 发布基准](../../README.md) 的 `pnpm bench:graph:publication` 检查。
汇总按节点行与输出连线组做浅比较，在一次 Immer 更新中交付所有变化；输入、输出标量表由同一批次更新。
运行节点集合按成员浅比较，失败信息直接共享 Execution 的不可变引用，最后只对组合字段浅比较。
内部展示派生不使用通用递归合并；未变化行、索引及完整展示对象保持引用。
图发布入口在安装后显式对账结果查询：撤销旧请求，移除新摘要已判为失效的当前 Pin 绑定，保留仍有效的绑定与已有报告租约。卸载和项目结束显式释放对应状态；不再通过两个图 Store 的订阅互相触发结果清空和重查。
项目发布完成后保留同批安装的结果摘要。卸载由统一图 store 一次移除图会话、实体和摘要，`resetGraphResultQueries` 仅清理查询、当前 Pin 绑定与运行状态，保留仍被消费者持有的结果 payload。
图查询清理在投影通知后重验原项目、图会话、执行会话和输出运行表引用；卸载调用还提供原图生命周期检查。
只有这些依据仍属于原操作时才清运行记录，监听器建立的新图生命周期或新运行不被旧清理删除。
运行事件的逐输出归属、活动状态和后端 `resultRevision` 保存在 ExecutionStore 的对应图记录中，与图运行状态和失败一起用一次 Immer 发布。Results 不再保存另一份运行事实。RunStarted 明确声明失效的输出；只有相同语义身份和执行会话的摘要 revision 达到事件版本后，才能重新显示其有效性。终态更新捕获的结果版本，已覆盖该版本的摘要无需再次等待。等待状态由只读入口派生，不保存可写标记，也不在查询回执后逐输出确认。查询、编辑回执与项目快照安装摘要时均立即生效，其他分支保留展示引用。
运行输出的接纳范围由 Execution owner 提供；Application 先按该范围撤销查询、清理未持有缓存，再发布运行事实。最终写入重新检查输出归属，查询监听器之后也重验项目及语义身份。较旧运行可结束其仍拥有的输出，不覆盖最新图运行。当前 Pin 读取同时拒绝仍适用的较新执行所淘汰的旧结果；只改变错误或请求状态时，复用未变的输出记录，不重新遍历全部 Pin 绑定。
输出运行记录保留事件自带的完整运行身份，不从事件到达时的画布补造语义依据。已加载图的运行事件接纳、运行节点、等待摘要和失败展示共用 `core/graph/read` 的语义 hash 与执行会话匹配规则；新图帧立即隐藏旧依据的展示，不等待后续清理。未加载图仍可接收公共执行事实，按结果引用打开的报告不受当前画布语义筛选。
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
动态 Schema 连续执行沿用同一运行身份，以累计 `RunStarted.outputs` 公告各阶段新增输出；
ExecutionStore 只接纳尚未归属该运行的输出，已公告输出与请求身份保持原引用。终态一次结束整个运行。
`RunStarted { outputs }` 公告当前输出的失效范围；完成后重查当前输出。Frontend 只撤销这些 pin 查询和未被读取组件持有的缓存，
不清空已打开报告或中断其分页/分析。Pin 查看与搜索只索引当前输出，按引用读取保留快照不会重写当前 pin 绑定。

Pin 菜单另提供“查看上次成功结果”：`stalePinResultReferences` 按当前图摘要和连接索引取得
过期值的完整执行会话与结果身份，再复用普通显式引用的查询、租约和面板流程。该入口不运行节点，
也不把旧值绑定成当前 Pin；打开前已被替换且没有其他持有者的结果按现有 unavailable 流程处理。
descriptor 中的来源图和运行编号继续参与结果身份校验、查询和搜索；结果视图不单独展示来源信息栏。

ResultService 复用既有 DTO parser 在 IPC 入口解析回复，并将 descriptor 的完整结果引用、Pin 的图与端口、
claim 的租约 token 与原请求关联。非空 provenance 输出必须与自身的图路径和节点身份一致。
结果 ID 使用非零、无前导零且不超过 Rust u64 范围的十进制字符串；共享 Result 类型拥有该校验，
图结果、描述符、分页、运行事件的结果查看请求、报告引用和 工作台意图复用，不在各边界重新定义范围或转换为 JS number。
分页回复的 ResultId、requestedLimit 和 offset 必须对应原请求：已知总数允许偏移截到末尾，未知总数
保持请求偏移；普通数据页和报告表页共用这一规则。value 回复保持既有正文契约。
这些检查先于查询缓存发布；Application 继续负责项目与查询代次接纳、面板租约申请失败清理，以及交接
descriptor 与窗口目标结果的关联，不在组件中重复解析。
分页读镜像按原请求键返回已校验回执，不再用原始偏移拒绝服务允许的末尾截断。

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

Run event 使用实际 ExecutionSessionId 与 RunId 标识运行，执行会话重建后的计数重置不会混入旧运行。结果查询投影集中在 Application results 模块；Execution Store 拥有图及逐输出运行状态、请求身份和失败摘要。分页缓存按完整结果引用、part、offset 和 limit 隔离，不同消费者翻页互不覆盖；Sequence renderer 统一提供表格与数列分页入口。读取组件持有 payload consumer lease，最后一个消费者释放时才清除本地 value/page；project reset 后的旧 lease 不能释放新项目的数据。主窗口和独立窗口的 Inspector 均由挂载的 renderer 读取数据，不预读后再重复读取。descriptor 仅表示可用结果，不携带 pending/failed/cancelled 状态；运行状态通过 Run event 投影表达。provenance 包含 RunId、输出地址与创建时间。

显式打开的 Result panel 和独立展示窗口绑定打开时的结果引用；节点删除、图语义修改、重跑均不会更换已有报告的数据。
Application 在创建面板前通过 `retain_result` 原子取得租约和 descriptor。租约 token 由调用方预先生成，
便于丢失响应后清理；Rust 确认结果和窗口身份，同 token 的重复申请/释放不会重复计数。
打开面板先解析结果引用，再使用租约回执中的 descriptor；不在申请租约前另行预读取或复制 descriptor。项目切换或面板打开失败时释放本次租约，已失效的结果引用不报告为布局故障。
输入 Pin 依次尝试上游结果时，整次打开操作保留调用时的项目身份，项目换代后不继续尝试余下目标。
FlexLayout 的真实面板集合驱动窗口内租约对账，移动、隐藏和 React 重新挂载不代表面板关闭；打开失败会释放申请的租约。

独立窗口使用指定接收窗口的租约交接：父窗口先保留结果，新窗口通过 `claim_result_lease` 原子接管，
不存在创建窗口期间无人持有数据的间隙。窗口销毁时后端清理所属租约及尚未接管的交接，并拒绝该 owner 的迟到申请；
独立结果窗口使用唯一实例 label。前端卸载事件不是唯一回收来源。
申请交接租约前捕获前端项目生命周期，取得后重验；等待期间项目换代则停止创建窗口，并经原租约入口释放。
前端尚无项目身份时仍可凭后端验证的结果引用申请，但同样检查空身份对应的 epoch；已发起的原生创建不由该检查撤回。
原生窗口创建以 `tauri://created` / `tauri://error` 为成功或失败依据；构造 WebviewWindow 对象本身不代表创建成功，
异步创建失败进入租约释放流程。几何插件按种类分组不会合并实例 label 或结果租约身份。
所有权操作按窗口串行协调，查询与统计计算不进入该队列；每次对账只遍历该窗口的租约。
申请、释放和新的对账请求在该队列内撤销上次对账确认，只有成功对账才能恢复跳过同值集合的依据。
即使面板在下次对账前关闭、集合恢复原样，刚申请的租约也会回收；失败或丢失响应不能继续沿用旧确认。

独立展示窗口的加载流程依赖结果身份、会话有效性和稳定的窗口动作，不随最大化或窗口错误状态重启。
租约接管完成及设置标题之后均重验本次加载是否取消；过期流程不继续读取或显示窗口。
挂载方仍负责释放本次 payload consumer，原生窗口销毁仍由后端回收结果租约。

删除、重命名或卸载图解除其缓存绑定，有租约的结果继续可读。语义编辑使受影响缓存过期，移动节点等布局操作保留有效缓存。
Pin 查询与当前结果搜索重新验证数据库内容及函数依赖；报告仍按打开时的完整引用读取。
编辑及资源版本变化撤销旧运行的发布资格，撤销恢复相同语义也不会恢复旧运行的资格。
执行会话结束（包括项目关闭、切换或会话重建）撤销所属租约并释放 ResultStore，
前端清理 project-scoped 面板并通知独立报告窗口关闭。跨窗口通道只通知会话结束，不再把输出失效广播成结果销毁。
旧会话引用不能读取新会话中的同号结果。运行失败摘要与 Results 的生命周期独立，清除 Output 中的错误不清除 Results。

Rust 的线性与二元统计摘要各自保留对应模型字段。Logit/Probit 使用 `pseudo_r2`、`adjusted_pseudo_r2`、`lr_chi2` 与 `prob_lr_chi2`，不生成 F/Wald 别名或线性 ANOVA 的平方和字段。通用回归 envelope 只复用系数、诊断与检验输入。

线性回归统一使用 `yssbi.statistics.linear.fit`、`linear.summary` 与 `linear.predict`。Fit 在配置中选择 OLS/WLS/GLS，输出 `model`、`fitted`、`residuals`；Summary 只接收 `model`，无拟合参数，也不重新估计；Predict 使用同一模型的系数与截距。

统计目录的其他 Summary 同样只接收对应方法的已拟合模型。Summary、ADF、独立检验和诊断只输出结构化 `result`；数值与报告是同一引用的两种展示，不是独立输出。普通结构化结果由通用组件直接呈现，ADF 及已实现方法的接入范围见 [Node Catalog](../../../../src-tauri/crates/yss-node-catalog/README.md)。
WLS 通过一个按需添加的 `weights` 数列接收正精度权重；GLS 通过按顺序添加的 `sigma` 数列接收完整相对误差协方差矩阵的各列，验证有限、方阵、对称与正定。只允许所选方法需要的辅助输入。WLS 权重与训练列联合读取以验证共同样本；协方差矩阵的行列顺序由调用者对应训练样本。GLS 当前仅支持常规标准误；其他标准误配置被拒绝。
执行计划保留端口实例分组身份和模板名，内核只接收中立模板名来区分 predictors/weights/sigma，不解析图地址。

Fit 的 `model` 与 Summary 的唯一 `result` 共享不可变的原生 `LinearRegressionResult`，仍由当前 `ResultStore` 拥有。报告类别统一为 `linearRegressionSummary`，标题为 Linear Regression Summary，模型概览保留实际 OLS/WLS/GLS 方法。WLS/GLS 的平方和与 R² 使用变换尺度，拟合值与残差保留原始尺度。
拟合值、残差、设计矩阵与参数协方差留在 Rust；报告 value 的 `presentation` 提供带格式标记的模型概览条目、ANOVA 列和行、条件数指标，由 [报告展示投影](../../../../src-tauri/crates/yss-application/src/graph/results/report/presentation.rs) 从统计结果构造，数字不会预先转换为展示字符串。报告还包含完整参数名目录 `paramNames`、
`{ executionSessionId, resultId }` 引用，以及 coefficients/observations 表引用与行数。
原生线性报告引用的 part 是固定枚举，不是任意 JSON 路径；观测表把拟合值和残差按拟合时的行序配对。
`get_result_table_page` 复用有界页面投影；`analyze_result` 读取已选的 ACF/PACF、序列相关与假设检验结果，仅需分析 kind。计算参数只由 Summary 节点配置，不在读取请求中重复传递。
残差图请求保留显示范围与点数，按拟合值范围筛选、跨整个匹配总体进行系统抽样，并标明总体/匹配数和抽样状态。
统计检验使用完整拟合数据。Application 捕获共享结果后在锁外读取/投影，返回前重验 session 与结果可用性，
结果回收或会话结束后的迟到成功和失败均被丢弃；仅当前输出失效不撤销有租约的快照读取。
数据仍由原 ResultStore 拥有，不另建报告存储或复制完整数据。

Application 的 `query_result_projection` 返回有界强类型投影；普通对象在克隆或编码前检查展开深度和空间预算，原生报告仅展开概览、参数目录与表引用。共享协议映射 `result_encoding` 供 IPC 与 Harness 调用，统一字段、宽整数文本与内联 JSON 字节上限；计数写入器不分配完整编码缓冲区。DataFrame、DataSeries 和顶层内存数列走现有分页入口；AI 不另维护统计字段白名单。AI 的 JSON 与表引用读取语义见 [Harness capability 契约](../../../../src-tauri/crates/yss-harness-core/README.md#4-registered-capabilities)。

结构化统计报告也默认只交付标量概览和数据引用。所有数组（含小数组和空数组）统一投影为
`{ kind: "tableRef", part, rowCount }`，不按大小选择是否内联；原数组仍由同一个不可变
ResultStore 结果持有。`structured:` part 由后端生成，使用有长度/深度边界且转义字段名的
内部数组地址；客户端把完整 part 原样传回 `get_result_table_page` / `inspect_result`，
不能据此查询其他结果或外部资源。分页会重新验证结果身份、类型、可用性和页大小，
嵌套数组继续返回引用。原生线性报告仍使用 `coefficients` / `observations` 两个固定 part。
GUI 默认只显示引用的行数，用户展开“查看数据”后才挂载分页读取；AI 通过显式
`inspect_result` 的 part 读取所需数据页。报告打开本身不预取这些数组。

线性回归报告的 `summary` 携带本次执行的内容选择和检验参数。Summary 执行阶段只计算选中的附加检验；报告查询返回已计算的检验，拒绝未选项和 Fit 模型。Fit 的普通 Inspector 保留模型概览，不伪造默认 Summary、分析能力或报告表引用。概览与报告共用 `query_result_projection`，不另设线性报告查询入口。系数读取只在选择系数表、系数图或方程时挂载；选择图形/观测表后仍在展开时读取有界投影。

`addLinearSummaryContents` 将 Report 的补选提交为来源节点的普通 `setParameters`，保留当前其他参数，再定向执行该 Summary 输出。上游模型有效时由既有缓存复用；模型输入变化时从当前图执行，最终以完整新结果替换报告。节点删除、项目切换、并发编辑或计算失败保留旧报告，不把新增分析装入旧结果。配置已提交但计算失败时仍可在节点修正或从报告重试；编辑不隐式保存图。
取得新结果租约后再次检查消费者、项目和图版本；失效时直接释放租约，不再持有本地 payload 或启动数据查询。
显式补选成功后，工作台通过 `replaceResult` 原子更新原面板的结果引用和租约，移动/重新挂载继续显示新结果；普通重跑仍不会替换已打开报告。独立窗口在当前视图内持有新快照租约。查询和租约继续由既有 Results owner 管理，失败和迟到结果释放临时持有关系。
前端复用 Result query coordinator，以执行会话、结果和完整查询语义隔离请求与缓存：分页包含 part/offset/limit，已计算的分析按 kind 区分，残差图还包含显示范围和点数。
相同不可变结果查询共用在途请求；不同查询参数独立发布。独立窗口及面板的 descriptor/value 同样经协调器读取与安装，并由挂载方持有 payload lease。Plot/report 仅接受完整 scalar payload；数列仍可在 Inspector 中分页，不能把第一页伪装成完整绘图数据。
展示加载器统一完成 Inspector、plot 和 report 分流，并复用原有 parser 在交付时校验完整 scalar。
Plot 交付稳定的 `ParsedPlotPayload` 或无效图形状态；报告交付带类别的数据和校验诊断，
数值查看仍使用同一原始 scalar。面板和独立窗口直接消费这份结果，不在重绘时重新解析。
`UnifiedResultView` 只呈现已分流的 Inspector 标量或数列；报告组件不保留第二次结果读取。
消费者用自己的请求代次控制迟到回执和 loading；value/page/analysis 分别订阅数据和错误。最后一个 payload consumer 释放、结果回收或会话结束会清理这些投影。
分页 Hook 统一交付 `rows` 和 `pageSize`，不再并列暴露无消费者的原始 `values` 与 `limit`；底层分页请求和 wire 字段仍由查询协议拥有。
页码属于消费者局部状态，按执行会话、结果、part、页大小和调用方总数绑定；身份变化在订阅和派发读取前重置，
不会先向新查询提交旧页偏移。同一完整身份的对象重建保留页码，未知总数仍由后端 `hasMore` 决定翻页。
只读网格直接接收页面的只读列和行，在 AG Grid 适配入口按行数组引用物化外层数组，保留行对象。
`ReadOnlyDataGrid` 的紧凑变体供 Details 内的数据表复用：省略行号、类型标记及补空列，按行数提供有界高度，保留列宽调整与表内双向滚动。普通结果页沿用现有网格外观与分页；紧凑变体不增加查询或数据状态。
假设检验的参数提示读取 Rust 提供的完整 `paramNames`，与系数表当前页无关。独立窗口的图类型直接读取 descriptor，不通过 URL 传递副本。
展示窗口从 presentation kind 一次映射到 inspect/plot 窗口类型；报告与数值检查共用 inspect，并据此生成路由，不接受任意路由字符串或未知路由回退。
报告字段的结构不再随观测数增长，也不通过大 scalar 的分页回退搬运完整数值数组。

报告校验直接使用 descriptor 的 `ResultReportKind`，只区分 `structured` 与 `linearRegressionSummary`。`parseReportPayloadResult` 验证通用结构化对象，或由 `parseLinearRegression` 校验原生线性报告的内容选择、展示字段、表引用及标题；`reportValidation` 同时核对线性报告的结果引用。其他统计方法保留 Rust 输出的结构，不再经过专用报告 DTO 或旧方法解析器。
结构化报告的解析结果同时持有已校验的展示章节及其方程文本或表引用，保留原数据引用；视图不再重复解析元数据或路径。
`useStructuredReportTable` 复用现有分页 hook，在页面或章节变化时通过 `structuredReportDisplay` 的原守卫校验声明列和稳定性根，
向视图交付有类别的只读行。非对象、缺列和无效根显式失败，不过滤坏行、补零或增加结果存储。

线性系数页与已选分析分别通过 `linearCoefficientField` 和 `parseResultAnalysis` 校验。缺失的可选指标显示为空缺，绘图与检验不会用零补齐不完整数据。
ACF/PACF 分析携带 SCI 已计算的 `ciHalfWidth`，IPC 原样映射，前端 parser 要求有限正数。
报告和相关图共用 SCI 的 95% 白噪声参考带，视图不依据样本数重新计算，也不为缺失字段提供推断或兼容回退。

报告组件与结果呈现见 [Results views](../../../modules/results/README.md)。

可视化结果仍使用 scalar Plot 查询与租约，不通过数列分页取第一页作为整张图。
Graph 的 20 类 `plot.data` 输出按 descriptor 的 chart kind 校验和绘制；KDE 也归 Plot。
`nomogram` 经同一 payload parser、模型映射和共享 renderer 展示 Rust 计算的轴与刻度；
校准和决策曲线复用折线及参考线，不在前端拟合或计算生存概率。
chart kind 使用实际图形名称，散点图为 `scatter`；`plot` 仅表示 presentation kind。
完整样本计算和展示抽样在 SCI 完成，前端保留观测数、AUC、分组或区间信息。

数据描述节点的 Details 通过 `useCurrentPinResult` 只订阅对应输出的有效 descriptor，复用
`useResultValue` 读取按原始列名组织的 `columns` 对象，各列摘要直接包含在结果 JSON 中。
`useDescriptionResult` 在载荷变化时按 `semantic` 验证各类型的专属字段及连续的 `position`，拒绝混入不适用字段；分类列的 `categories` 编号对象投影为有序明细，保留类别原值、标签、频数、占比以及 Null、空字符串与宽整数文本。
组件只格式化 Rust 已计算的统计量。当前图持有有效结果，Details 不固定历史快照或另建结果存储；
上游编辑、重新执行和项目切换沿用 Results 的有效性判定，清除过期内容，卸载后释放 payload consumer。

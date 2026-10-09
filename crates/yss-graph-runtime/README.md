# Graph runtime

> Status: Current
> Scope: 图解析 facade 与可丢弃的语义缓存
> Canonical owners: 本 crate 的解析入口、mutations、binding_cleanup 与 semantic_cache 模块
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## Semantic cache

`node_creation_form` 从冻结 Registry 读取协议，复用 Editor 的参数合并、Protocol 的初始端口数量校验和
Analysis 的参数投影，再使用既有本地化与 Editor 映射生成创建表单。查询返回显式值、有效参数组、
端口数量和配置范围；不构造 GraphDocument、不载入图，也不改写语义缓存、历史或项目版本。

`registry()` 返回本运行时使用的冻结注册表借用。Application 的函数正文捕获和 Project 引用事务
复用该声明身份，不重新组装内置注册表，也不新增可写节点配置。

`materialize_open_candidate` 验证 Project 已捕获的不可变 `Arc<GraphDocument>`，直接共享这份
快照供打开回执使用。打开物化阶段不再深拷贝文档；Project 的编辑、历史和文档身份仍是权威，
Application 在交付前继续重验项目与会话。验证失败和物化阶段故障不会交付投影。

连线候选与编辑规划先借用原文档。只有需要认领/恢复派生绑定或应用已生成的补丁时才创建临时副本，
方向、类型等预检拒绝不提前复制整图；已有非 orphan 绑定也按借用读取。
候选判断继续使用提交所用的同一规划器，补丁原子校验及无引用派生绑定清理仍由现有入口完成。
`mutations` 复用 Document Edit 的 `GraphDocumentPatchPreview`，在首次准备补丁时分叉原文一次，
并复用原文的结构校验依据。`GraphDocumentRead` 将该依据与不可变文档借用一起交给 Editor，
连接变更和认领补丁使用同一结构规则，只核对受影响字段；删除节点/绑定与非法原文修复仍完整校验。
认领与实际编辑使用嵌套作用域，成功或失败返回时均恢复原内容。每个目标仍从同一原文规划，
返回补丁包含无引用派生绑定清理。候选只供本次规划读取，提交仍重验当前文档。
`binding_cleanup` 在同一规划器内按需统计原文档的实例端口引用一次，再叠加本次已验证补丁中的
连线与输入状态引用变化。引用按数量统计，任一连线或输入状态仍引用时就保留绑定；各候选只叠加
自己的变化，用户创建绑定始终保留。此索引随规划器丢弃，不写入项目、语义缓存或图文档。

Graph Runtime 为最近使用的图保留一个不含本地化文本的完整 `GraphAnalysis` 缓存，容量由 [semantic_cache.rs](src/semantic_cache.rs) 定义。复用前校验解析文档指纹、registry/kernel 指纹和之前实际读取的资源，包括传递函数正文和 absent lookup。解析缓存的身份与执行身份分开：常量名称、函数参数名称、诊断所用 connection ID 和 orphan metadata 会影响解析快照，不能仅凭 `semanticInputHash` 复用。无关资源变化不使该缓存失效。

`ResourceCatalogSnapshot` 只保存捕获的函数和数据库 Schema 事实；缓存与执行身份使用实际依赖的读取记录，不计算或保存未被消费者使用的全目录指纹。项目会话和资源提交身份仍由 Application 在捕获与交付边界重验。

Runtime 将实际读取记录直接交给 `GraphSemanticSnapshot.dependencies()`，包括资源指纹和 absent lookup。
[分析环境](../yss-graph-analysis-contract/README.md) 只携带 Registry/Kernel 指纹；Runtime、Analysis
与 Editor 投影不再转换或复制另一份资源版本/观测表。

有运行列观测时，Runtime 的同一解析缓存也核对观测指纹。`definition_input_hash` 标识文档、协议、内核和
资源定义；`semanticInputHash` 再包含解析实际采用的输出观测版本及字段。该区分让 Execution 识别纯列反馈，
保留输入未变的在途生产者，同时继续拒绝过时的图定义和已被替换的观测。函数定义解析不继承调用图的结果观测。

解析入口接收不可变的 `Arc<GraphDocument>`。缓存只记录该正文的 `Weak` 身份和已验证的解析指纹，
同一正文再次解析时复用指纹，不重复编码整图；新正文仍计算指纹，内容相同时可复用语义并更新弱身份。
`Arc` 的写时复制会在写入前分离缓存记录的身份；弱引用不保留已释放的正文，也不能授权修改或提交。
无论正文指纹是否复用，Registry、Kernel、实际资源依赖与运行列观测仍按当前捕获值校验。

位置和节点 user label 从当前文档投影，不进入解析缓存身份；资源显示名与语言按当前请求本地化。
缓存命中跳过 Schema、类型和函数语义重算，仍执行观测及依赖指纹计算、本地化、投影生成以及
Application 的资源/session 重验。缓存只是可丢弃的派生结果，没有文档或历史写权限；解析、哈希、
本地化和大对象释放均在缓存锁外完成。

资源节点的实例标题按协议 `instance_display` 指定的参数读取已有语义资源事实，遵循解析后的默认值和显隐规则，不扫描普通字符串参数。常量名称继续来自语义快照，用户自定义标签仍由文档单独交付。

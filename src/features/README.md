# Features

- `application/` 编排跨领域用户用例，调用 services 并协调投影发布。
- `core/` 保存 Rust 投影、显式未保存草稿和共享交互状态。
- `domain/` 提供不依赖 React、Tauri 或 services 的领域规则。

Graph、Resource、Execution、Settings、Database 和界面读取能力复用 `core/state/readProjection.ts`。各 owner 必须不可变更新；提交投影在发布时通过 `freezePublishedValue` 冻结，读取端共享原始引用，不复制完整数据。React 使用统一 selector 订阅，未变化投影字段不触发消费者重渲染。`freezeProjectionSnapshot` 仅用于需要隔离外部输入的复制边界；不能重新放进高频读取 hook。Map/Set 同样遵守不可变替换约定，Readonly 类型禁止消费者调用修改方法。
只读投影的快照与通知由内部 Zustand store 管理，外部仍只获得 `getSnapshot / subscribe`；订阅遵循 Zustand 的生命周期，
发布时不再复制完整监听器数组。候选先进行顶层浅比较，实际变化时冻结并整体替换一次，保留原始分支引用。
内部 store 从空状态创建，再同步安装首次快照后才开放读取，避免 Zustand 的初始状态长期保留首份图或资源数据。
它只替换原有读取缓存的实现，不接受业务写入；源 owner 和现有订阅适配接口保持不变。
深冻结按自有可枚举字段遍历，避免为每个对象分配临时值数组。弱引用遍历缓存只记录需要递归的容器、非空数组和显式发布根，纯标量叶对象与嵌套空数组仍正常冻结，减少首次加载的缓存表分配。可复用的验证凭据只在完整冻结成功后对外可见；浅冻结不能作为深冻结凭据。

纯读取模块导出 selector hook 和使用中的命令式读取入口，不再并行维护重复的 `*ReadCapability` 包装对象。状态栏等调用方直接复用通用 `useReadProjection`，不另包工作台专用订阅 hook。

图表名称、资源路径和修订由 ResourceStore 统一发布与读取；ChartDocumentStore 只保存已加载文档和本地草稿，不再保存另一份图表资源索引。数据库元数据更新使用 DatabaseStore 的现有写入方法，完整快照由项目发布入口统一安装。

资源成员关系和排序通过 `ResourceStore.setSnapshot` 一次发布；文档加载、dirty 和 stale 标记由 `documentStateActions` 更新。`GraphMetaStore` 按路径索引类型与函数签名投影，项目加载和发布直接安装完整映射，资源名称从 ResourceStore 读取。

主题变化统一调用 `settingsUi.updateAppearance`。侧栏拖拽的呈现通过 `useSidebarDragUi` 订阅，动作通过 `sidebarDragUi` 更新；画布投放处理器由 Application 直接使用 `canvasDropHandlerStore` 注册和查询。

节点 Details 按当前节点、端口和诊断显示文本选择投影，引用数组使用浅比较；参数编辑器消费只读协议值，不在 selector 或渲染中深拷贝。连接候选在打开选择器时查询 Rust，前端只映射候选标签、标出替换行为并保留当前已连接项用于显示；重复连接由 Rust 判定，Details 不另设候选排除规则。编辑草稿在用户修改或提交边界产生新值。
节点参数统一消费 Rust 的 `parameterGroups`，组内参数通过 `editor.kind` 选择控件，普通参数和依赖 Schema 的参数共用这一结构。Details 保留组顺序与说明，每个参数独立展开；画布从各组读取 `inlineAndDetail` 字段，折叠状态属于局部 UI。`setNodeParameters` 只提交用户修改的字段，null 表示清除显式值，合并、默认值、条件显隐和原子校验由 Rust 拥有。
数值参数编辑遵循当前 `Scalar/Numeric` 语义，允许小数；客户端保留必填、有限值和安全整数检查，不保留旧物理整数类型对应的“必须为整数”错误分支。

连接提示由 `application/graphEditing/useConnectionCandidates` 查询 Rust 的连接决策投影，普通连接和迁移共用实际 mutation planner。前端按后端的 append、replace、invalid 结果显示高亮、替换范围和候选列表，不推导类型兼容性或迁移容量。查询只保留当前起点及操作的一份结果，按项目、图编辑版本、语义身份和资源目录发布版本失效；迟到响应不能覆盖新手势，鼠标移动不触发 IPC。Pin 创建目录同样在这些身份变化时重新查询 Rust 的兼容目录；单纯结果状态更新不会重新查询。

`core/dataStore/graphProjectionStore` 是图会话、实体和结果有效性摘要的统一只读发布入口。打开、编辑、撤销、保存以及项目快照都先验证完整回执，再一次安装 `sessions / graphEntities / resultStates`；原图编辑 Store 已移除。完整快照和增量响应都按实体 ID 复用未变化的内容，发布时冻结对象。编辑版本和结果摘要版本独立接纳，迟到的较旧结果不能覆盖新运行状态。结果查询和持有租约仍由 Results Application 管理。
单图安装和多图快照共用 `prepareGraphSessions`：先准备每张图的会话、实体桶与结果条目，再一次冻结完整批次。三个项目级表各自最多复制一次，删除过滤已经生成的新表直接参与候选构建；拒绝旧版本或内容未变化时保留原引用，完全无变化的批次返回原状态。中途校验失败或路径重复不会发布部分结果，项目快照仍在原提交边界一次安装。
实体安装直接更新现有 `GraphEntityBucket`，不再构造另一份临时规范化模型。首次加载在发布前填充新表；后续安装用一次 Immer `produce` 更新变化分支，按前后只读投影的引用跳过未变化节点及其端口。节点顺序和成员关系变化时仍按 ID 复用实体；仅移动节点不重建端口表、连线表或邻接表。连线变化只重建受影响端口的邻接数组，并按投影顺序扫描连线以保留重排语义；删除节点、端口和连线时同步清理索引。`blockedConnectionIds` 只随规范诊断变化重算，连线订阅按 ID 查询，不再逐条扫描诊断。
`primaryPortDiagnostics` 是同一实体桶内按端口 ID 查询的扁平稀疏索引，仅在对应节点的
诊断引用变化时替换其条目。一次遍历选出每个端口的首个阻断诊断，无阻断时保留首个诊断，
并共享原始诊断对象；不额外分配每个节点的索引表。节点或诊断移除时在同一次发布中清理，位置和
无关字段变化复用原索引。Pin selector 直接按 ID 读取，不重复扫描列表、构造地址键或过滤数组；
完整问题列表及诊断语义仍来自 Rust 投影。
同步入口先冻结属于该响应的候选数据，再由现有 parser/guard 完成校验，成功后才更新基线并交付 Store。已成功校验且由 `deepReadonly` 完成深冻结的对象，可通过弱引用缓存复用节点结构、文档、结果摘要和节点局部语义检查；可变对象及仅浅冻结的输入不进入缓存。节点语义检查复用 shape guard 已验证的参数元数据与键唯一性，避免首次加载重复分配参数校验集合；直接传入的未验证节点仍执行这些检查。缓存不持有独立会话或延长对象生命周期，变化分支仍重新校验。每个变化的投影都检查图路径、重复 ID、端点存在性与方向及阻断诊断；同一不可变投影在同步和安装之间复用完整校验结果，结果语义哈希、会话及响应绑定继续在接纳前核对。`shareProjection` 保留已经完成结构共享的增量对象身份，只在需要替换子引用时分配容器；完整快照继续复用与旧投影相等的分支。
`pnpm bench:graph` 对完整快照和增量分别报告同步解析、实体安装及两者合计的耗时；安装样本在计时外准备已验证的输入，快照样本每次清空基线和该图 Store，增量样本先建立基线。样本之间清理 mock 调用历史，避免历史响应干扰对象回收。可用 `--testNamePattern='5000 nodes: snapshot'` 或 `--testNamePattern='5000 nodes: delta'` 独立测量，减少上一阶段回收对下一阶段的干扰。该基准使用模拟 IPC，不测量 Rust 处理、真实传输或浏览器布局与绘制，桌面交互仍需人工验收。
`pnpm bench:graph:publication` 单独测量多图快照准备和执行状态的结果展示更新。批量样本消费已冻结的会话，每次从空项目表准备；展示样本安装稳定结果后，通过真实 Execution Store 与 Results 读取投影交替设置、清除错误。两者均不包含 IPC、React 组件渲染或浏览器绘制。
节点视图按输入、输出分组并共享原始 Pin 引用；连接数量直接读取 Rust 的 `connections.current`，不再派生第二套 Pin 连接状态。连线记录保留必需的结构化端点和顺序字段，查看结果与诊断直接消费这些字段。

Application 可以依赖 Core、Domain 和 Services；依赖不能从 Core 或 Domain 反向指向 Application 或界面。完整边界由[当前架构](../../docs/architecture/ARCHITECTURE.md#layer-and-dependency-direction)和[架构门禁](../../docs/development/ARCHITECTURE_GATES.md)维护。

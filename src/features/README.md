# Features

- `application/` 编排跨领域用户用例，调用 services 并协调投影发布。
- `core/` 保存 Rust 投影、显式未保存草稿和共享交互状态。
- `domain/` 提供不依赖 React、Tauri 或 services 的领域规则。

Graph、Resource、Execution、Settings、Database 和界面读取能力复用 `core/state/readProjection.ts`。各 owner 必须不可变更新；提交投影在发布时通过 `freezePublishedValue` 冻结，读取端共享原始引用，不复制完整数据。React 使用统一 selector 订阅，未变化投影字段不触发消费者重渲染。`freezeProjectionSnapshot` 仅用于需要隔离外部输入的复制边界；不能重新放进高频读取 hook。Map/Set 同样遵守不可变替换约定，Readonly 类型禁止消费者调用修改方法。

纯读取模块导出 selector hook 和使用中的命令式读取入口，不再并行维护重复的 `*ReadCapability` 包装对象。状态栏等调用方直接复用通用 `useReadProjection`，不另包工作台专用订阅 hook。

图表名称、资源路径和修订由 ResourceStore 统一发布与读取；ChartDocumentStore 只保存已加载文档和本地草稿，不再保存另一份图表资源索引。数据库元数据更新使用 DatabaseStore 的现有写入方法，完整快照由项目发布入口统一安装。

资源成员关系和排序通过 `ResourceStore.setSnapshot` 一次发布；文档加载、dirty 和 stale 标记由 `documentStateActions` 更新。`GraphMetaStore` 按路径索引类型与函数签名投影，项目加载和发布直接安装完整映射，资源名称从 ResourceStore 读取。

主题变化统一调用 `settingsUi.updateAppearance`。侧栏拖拽的呈现通过 `useSidebarDragUi` 订阅，动作通过 `sidebarDragUi` 更新；画布投放处理器由 Application 直接使用 `canvasDropHandlerStore` 注册和查询。

节点 Details 按当前节点、端口和诊断显示文本选择投影，引用数组使用浅比较；参数编辑器消费只读协议值，不在 selector 或渲染中深拷贝。连接候选在打开选择器时查询 Rust，前端只映射候选标签、标出替换行为并保留当前已连接项用于显示；重复连接由 Rust 判定，Details 不另设候选排除规则。编辑草稿在用户修改或提交边界产生新值。
节点参数统一消费 Rust 的 `parameterGroups`，组内参数通过 `editor.kind` 选择控件，普通参数和依赖 Schema 的参数共用这一结构。Details 保留组顺序与说明，每个参数独立展开；画布从各组读取 `inlineAndDetail` 字段，折叠状态属于局部 UI。`setNodeParameters` 只提交用户修改的字段，null 表示清除显式值，合并、默认值、条件显隐和原子校验由 Rust 拥有。
数值参数编辑遵循当前 `Scalar/Numeric` 语义，允许小数；客户端保留必填、有限值和安全整数检查，不保留旧物理整数类型对应的“必须为整数”错误分支。

连接提示由 `application/graphEditing/useConnectionCandidates` 查询 Rust 的连接决策投影，普通连接和迁移共用实际 mutation planner。前端按后端的 append、replace、invalid 结果显示高亮、替换范围和候选列表，不推导类型兼容性或迁移容量。查询只保留当前起点及操作的一份结果，按项目、图编辑版本、语义身份和资源目录发布版本失效；迟到响应不能覆盖新手势，鼠标移动不触发 IPC。Pin 创建目录同样在这些身份变化时重新查询 Rust 的兼容目录；单纯结果状态更新不会重新查询。

`core/dataStore/graphProjectionStore` 是图会话、实体和结果有效性摘要的统一只读发布入口。打开、编辑、撤销、保存以及项目快照都先验证完整回执，再一次安装 `sessions / graphEntities / resultStates`；原图编辑 Store 已移除。完整快照和增量响应都按实体 ID 复用未变化的内容，发布时冻结对象。编辑版本和结果摘要版本独立接纳，迟到的较旧结果不能覆盖新运行状态。结果查询和持有租约仍由 Results Application 管理。
节点视图按输入、输出分组并共享原始 Pin 引用；连接数量直接读取 Rust 的 `connections.current`，不再派生第二套 Pin 连接状态。连线记录保留必需的结构化端点和顺序字段，查看结果与诊断直接消费这些字段。

Application 可以依赖 Core、Domain 和 Services；依赖不能从 Core 或 Domain 反向指向 Application 或界面。完整边界由[当前架构](../../docs/architecture/ARCHITECTURE.md#layer-and-dependency-direction)和[架构门禁](../../docs/development/ARCHITECTURE_GATES.md)维护。

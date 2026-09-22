# Features

- `application/` 编排跨领域用户用例，调用 services 并协调投影发布。
- `core/` 保存 Rust 投影、显式未保存草稿和共享交互状态。
- `domain/` 提供不依赖 React、Tauri 或 services 的领域规则。

Graph、Resource、Execution、Settings、Database 和界面读取能力复用 `core/state/readProjection.ts`。各 owner 必须不可变更新；提交投影在发布时通过 `freezePublishedValue` 冻结，读取端共享原始引用，不复制完整数据。React 使用统一 selector 订阅，未变化投影字段不触发消费者重渲染。`freezeProjectionSnapshot` 仅用于需要隔离外部输入的复制边界；不能重新放进高频读取 hook。Map/Set 同样遵守不可变替换约定，Readonly 类型禁止消费者调用修改方法。

纯读取模块导出 selector hook 和使用中的命令式读取入口，不再并行维护重复的 `*ReadCapability` 包装对象。状态栏等调用方直接复用通用 `useReadProjection`，不另包工作台专用订阅 hook。

图表名称、资源路径和修订由 ResourceStore 统一发布与读取；ChartDocumentStore 只保存已加载文档和本地草稿，不再保存另一份图表资源索引。数据库元数据更新使用 DatabaseStore 的现有写入方法，完整快照由项目发布入口统一安装。

节点 Details 按当前节点、端口和诊断显示文本选择投影，引用数组使用浅比较；参数编辑器消费只读协议值，不在 selector 或渲染中深拷贝。连接候选只依赖端口、连接和节点标题，编辑草稿在用户修改或提交边界产生新值。
节点参数统一消费 Rust 的 `parameterGroups`。Details 按组显示可独立展开的折叠面板，画布从各组读取 `inlineAndDetail` 字段；组状态属于局部 UI。`setNodeParameters` 只提交用户修改的字段，null 表示清除显式值，合并、默认值、条件显隐和原子校验由 Rust 拥有。
数值参数编辑遵循当前 `Scalar/Numeric` 语义，允许小数；客户端保留必填、有限值和安全整数检查，不保留旧物理整数类型对应的“必须为整数”错误分支。

连接提示消费后端已解析的类型域；结构类型按名义身份精确匹配，前端不维护继承关系表。Pin 创建目录在图编辑版本、语义身份或资源目录发布版本变化时重新查询 Rust 的兼容目录；单纯结果状态更新不会重新查询。

`core/dataStore/graphProjectionStore` 是图会话、实体和结果有效性摘要的统一只读发布入口。打开、编辑、撤销、保存以及项目快照都先验证完整回执，再一次安装 `sessions / graphEntities / resultStates`；原图编辑 Store 已移除。完整快照和增量响应都按实体 ID 复用未变化的内容，发布时冻结对象。编辑版本和结果摘要版本独立接纳，迟到的较旧结果不能覆盖新运行状态。结果查询和持有租约仍由 Results Application 管理。
节点视图按输入、输出分组并共享原始 Pin 引用；连接数量直接读取 Rust 的 `connections.current`，不再派生第二套 Pin 连接状态。连线记录保留必需的结构化端点和顺序字段，查看结果与诊断直接消费这些字段。

Application 可以依赖 Core、Domain 和 Services；依赖不能从 Core 或 Domain 反向指向 Application 或界面。完整边界由[当前架构](../../docs/architecture/ARCHITECTURE.md#layer-and-dependency-direction)和[架构门禁](../../docs/development/ARCHITECTURE_GATES.md)维护。

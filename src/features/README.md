# Features

- `application/` 编排跨领域用户用例，调用 services 并协调投影发布。
- `core/` 保存 Rust 投影、显式未保存草稿和共享交互状态。
- `domain/` 提供不依赖 React、Tauri 或 services 的领域规则。

Graph、Resource、Execution、Settings、Database 和界面读取能力复用 `core/state/readProjection.ts`。各 owner 必须不可变更新；提交投影在发布时通过 `freezePublishedValue` 冻结，读取端共享原始引用，不复制完整数据。React 使用统一 selector 订阅，未变化投影字段不触发消费者重渲染。`freezeProjectionSnapshot` 仅用于需要隔离外部输入的复制边界；不能重新放进高频读取 hook。Map/Set 同样遵守不可变替换约定，Readonly 类型禁止消费者调用修改方法。

纯读取模块导出 selector hook 和使用中的命令式读取入口，不再并行维护重复的 `*ReadCapability` 包装对象。状态栏等调用方直接复用通用 `useReadProjection`，不另包工作台专用订阅 hook。

节点 Details 按当前节点、端口和诊断显示文本选择投影，引用数组使用浅比较；参数编辑器消费只读协议值，不在 selector 或渲染中深拷贝。连接候选只依赖端口、连接和节点标题，编辑草稿在用户修改或提交边界产生新值。

连接提示消费后端已解析的类型域；结构类型按名义身份精确匹配，前端不维护继承关系表。Pin 创建目录在当前图编辑版本变化时重新查询 Rust 的兼容目录。

Application 可以依赖 Core、Domain 和 Services；依赖不能从 Core 或 Domain 反向指向 Application 或界面。完整边界由[当前架构](../../docs/architecture/ARCHITECTURE.md#layer-and-dependency-direction)和[架构门禁](../../docs/development/ARCHITECTURE_GATES.md)维护。

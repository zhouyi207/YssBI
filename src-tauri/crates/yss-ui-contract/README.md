# 工作台界面意图

> Status: Current
> Scope: 工作台操作请求、回执与会话交付
> Canonical owners: `yss-ui-contract` 拥有共享协议；Application presentation 验证目标并管理意图；本文维护跨端契约
> Update when: 意图类型、会话边界、交付或回执改变时

Workbench 拓扑、标签顺序、尺寸、选中面板和折叠状态由 FlexLayout Model 拥有。Application 接收打开资源、定位图节点、打开结果和显示登记面板的请求，前端调用已有工作台入口并确认执行结果。

协议见 [UI contract](src/lib.rs)，用例见 [presentation](../yss-application/src/presentation.rs)。报告组件直接消费 [Results](../../../src/modules/results/README.md) 的已解析结果，不经过本模块。

## 请求与交付

`request_ui_intent` 提交 `{ clientKey, intent }` 并返回 `UiIntentReceipt`。`inspect_ui_intent` 通过 `{ id }` 查询同一回执。Harness Schema 从 Rust 类型生成；前端 Service 验证收到的意图、结果引用和回执状态。

`subscribe_ui_intents` 仅允许 `main` 工作台订阅；会话 Channel 交付 `intent`、`resync` 和 `sessionChanged`。前端工作台 hook 拥有该订阅，卸载调用 `unsubscribe_ui_intents`，迟到回复与事件按原项目生命周期隔离。没有页面监听者分流或订阅权限切换。

后端 Channel 使用 128 条有界广播缓冲；慢消费产生 `resync`，前端恢复待处理意图。Application 发布中立 observer 通知，Tokio 缓冲与任务归 IPC。取消订阅在注册表锁内移除 observer，锁外释放回调，允许资源析构重入订阅管理。会话替换后旧流发出 `sessionChanged` 并结束，前端重新订阅。

## 意图与回执

Harness `request_ui_intent` 通过 Application 请求界面操作。`openResource` 使用
`yss-project-identity::ProjectResourceRef { kind, id }` 打开已有事件图、函数图、图表、思维导图、
文档或数据；可选 `nodeId` 只用于事件图和函数图的节点定位。其他意图为打开保留结果、显示白名单面板。
前端复用文件/数据库打开入口；新资源的索引通知尚未到达时先刷新索引，并检查原生活动面板的真实身份。
后端验证项目、资源成员关系、结果会话和请求大小；只有 `main` 工作台订阅能认领意图，其他窗口不能执行工作台操作。

请求携带 `clientKey`，在调用者与 Application session 内去重。相同 key 和内容返回原回执，不同内容拒绝；回执缓存有界，跨会话或淘汰后不承诺重放。
回执状态是 pending → claimed → applied / failed。前端先原子认领，再串行调用现有入口，最后确认实际结果；重复消息不能重复认领。
关闭面板不要求结束 Harness 对话，界面操作也不保存图文档。

未认领或未确认的请求超过 30 秒转为 expired；它表示没有及时得到完成证据，不能宣称已执行或撤销。没有活动工作台时请求直接失败。
重连恢复只投递 pending 意图，不重新执行 claimed 意图。Harness 可通过 `inspect_ui_intent` 查询回执，pending 不是成功证据。
前端 `uiIntentDelivery` 拥有当前工作台绑定的临时认领队列与恢复请求，React hook 只负责订阅及生命周期。
pending 读取期间再次收到缺口会合并为一次后续读取；旧请求成功或失败后均重新查询，项目或绑定失效则丢弃后续恢复和旧回执。
前端交付队列满载时记录一次恢复需求，原队列排空后重新读取 pending；后台已过期并淘汰的旧回执不能让新意图永久漏投递。恢复不扩张队列，也不重放已经认领的操作。
项目替换后不执行旧队列，结果打开仍经既有租约取得与失败释放流程。

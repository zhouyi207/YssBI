# 工作台界面意图

> Status: Current
> Scope: 工作台操作请求、回执与会话交付
> Canonical owners: `yss-ui-contract` 拥有共享协议；Application presentation 验证目标并管理意图；本文维护跨端契约
> Update when: 意图类型、会话边界、交付或回执改变时

Workbench 拓扑、标签顺序、尺寸、选中面板和折叠状态由 GPUI 的根 DockArea 拥有。Application 接收打开资源、定位图节点、打开结果和显示登记面板的请求，原生宿主调用已有工作台入口并确认执行结果。

协议见 [UI contract](src/lib.rs)，用例见 [presentation](../yss-application/src/presentation.rs)，原生交付见 [workbench intents](../yss-desktop-gpui/src/workbench/intents.rs)。报告组件直接消费 [Application Results](../yss-application/src/graph/README.md) 的已解析结果，不经过本模块。

## 请求与交付

原生宿主通过 `ApplicationState::attach_workbench` 获取 `WorkbenchBinding`，直接订阅同一 `UiEvent`，
使用 binding 的 `pending` 恢复待处理请求、`settle` 认领和结算。Binding 持有原应用会话，
读取及结算重验该会话；Drop 释放工作台登记及 observer。宿主不另维护业务回执或另一套工作台可用性计数。原生宿主未实现的目标结算为 failed，不能回传 applied。

GUI 与 Harness 通过原 Application 请求、检查和结算同一回执；Harness 工具只包含业务
`intent`，幂等 key 由 Core 生成。结果引用和模型 schema 继续由 Rust 当前契约导出。
原生宿主在自己拥有的有界事件队列中串行消费，缺口通过 binding 的 `pending` 恢复；
事件和迟到回复按项目与宿主 lifecycle 隔离。项目替换后的旧请求不能作用于后继工作台。

## 意图与回执

Harness `request_ui_intent` 通过 Application 请求界面操作。`openResource` 使用
`yss-project-identity::ProjectResourceRef { kind, id }` 打开已有事件图、函数图、图表、思维导图、
文档或数据；可选 `nodeId` 只用于事件图和函数图的节点定位。其他意图为打开保留结果、显示白名单面板。
原生宿主复用文件/数据库打开入口，并检查原生活动面板的真实身份。
后端验证项目、资源成员关系、结果会话和请求大小；当前工作台通过自己的 binding 认领意图，并按登记的 claimant 结算。

请求携带 `clientKey`，在调用者与 Application session 内去重。相同 key 和内容返回原回执，不同内容拒绝；回执缓存有界，跨会话或淘汰后不承诺重放。
回执状态是 pending → claimed → applied / failed。宿主先原子认领，再串行调用现有入口，最后确认实际结果；重复消息不能重复认领。
关闭面板不要求结束 Harness 对话，界面操作也不保存图文档。

未认领或未确认的请求超过 30 秒转为 expired；它表示没有及时得到完成证据，不能宣称已执行或撤销。没有活动工作台时请求直接失败。
重连恢复只投递 pending 意图，不重新执行 claimed 意图。Harness 可通过 `inspect_ui_intent` 查询回执，pending 不是成功证据。
GPUI Workbench 持有当前 binding 的有界临时队列和恢复标记。队列满载时记录恢复需求，
排空后从同一 binding 重读 pending；恢复不扩张队列，也不重放已经认领的操作。
事件接纳检查项目身份和宿主的交付代次，项目替换时丢弃旧队列。
结果打开按原结果面板的完成通知结算，继续使用既有租约取得与失败释放流程。

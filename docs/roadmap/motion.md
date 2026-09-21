# motion：React / Harness 共用入口与投影同步

> Status: Planned
> Scope: 各领域共用 Application、后端投影、界面意图与剩余验收
> Canonical owners: 本文维护覆盖矩阵与进度；当前 Graph、Presentation、Harness、Project 和 IPC 文档拥有稳定契约
> Update when: 操作覆盖、同步协议、界面意图或验收状态改变时

React 和 Harness 都是 Application 能力的客户端。业务状态继续归既有领域；本轮选择让 Rust 管理页面 JSON、页面动作及打开/聚焦意图，FlexLayout 继续作为工作台物理布局的唯一权威。当前页面实现见 [Presentation](../../src-tauri/crates/yss-ui-contract/README.md)。

## 当前覆盖

| 领域      | GUI                              | Harness                                          | owner / 交付                                                  |
| --------- | -------------------------------- | ------------------------------------------------ | ------------------------------------------------------------- |
| Graph     | 编辑、历史、显式保存、执行       | inspect/edit/validate/execute/save               | Project 当前文档；共用 Application；Graph 快照/增量与活动通知 |
| Project   | 创建、打开、切换、另存等生命周期 | inspect_project 查询资源索引                     | Project；项目发布协调器安装索引/Activity 增量                 |
| Data      | 导入、管理、编辑、保存、分页查询 | schema/profile 检查                              | Database / Project；既有查询和项目发布，无 Harness 数据写能力 |
| Chart     | 资源管理、配置草稿、保存         | 尚无 Chart 写能力                                | Project 持久化，现有前端资源草稿；项目发布安装                |
| Results   | 保留、分页、分析、报告、打开视图 | inspect_result、list_graph_results、打开结果意图 | Execution ResultStore；共用读取与租约入口                     |
| Plugin    | 安装、启停、视图、任务管理       | 尚无插件管理能力                                 | Plugin Manager；现有插件协议与投影                            |
| JSON 页面 | 显隐、排序、替换、重置、按钮     | inspect_ui / update_ui                           | Application presentation；修订快照与直接元素增量              |
| 界面意图  | 页面按钮                         | request_ui_intent / inspect_ui 回执              | Application 验证和排队；工作台认领、执行并确认                |

Graph 的 GUI 显式 Save 与 Harness 原子保存编辑批次继续保持既定差异。Graph 已有的活动通知后刷新适合当前业务投影边界；页面变更规模有明确上限，直接推送已提交增量。没有把所有领域拼成一个可任意 patch 的全局 JSON Store。

## 已接入

- [x] 记录各领域 GUI / Harness 的现有覆盖、唯一状态 owner 和交付方式；未开放给 Harness 的业务写操作仍单独跟踪。
- [x] JSON 页面 GUI / Harness 共用 Application；提交后返回并推送相同修订增量，基线错误读取快照恢复，不重放写操作。
- [x] 受控界面意图支持打开已有图、定位节点、打开保留结果和显示登记面板；复用既有面板、编辑器和结果租约入口。
- [x] 意图按调用者/clientKey 去重，目标为 main 工作台；前端先认领再串行执行，回执区分 pending/claimed/applied/failed/expired，Harness 可以查询。
- [x] 项目、结果和 Application session 校验；旧会话结束订阅，工作台关闭、队列容量、请求过期与失效目标明确失败。
- [x] 后端回归覆盖原子批次、修订冲突、重复认领与 GUI/Harness 读取同一页面；不以自动检查替代桌面操作验收。

## 剩余工作

- [ ] 人工验收 GUI / Harness 交错编辑、错误批次、重复意图、打开与定位、面板关闭重开、会话替换、迟到投影和恢复；记录实际结果。
- [ ] 对 Graph 的通知后刷新与直接增量推送做端到端测量，再决定是否调整既有协议；页面推送不能证明 Graph 的请求数或绘制成本已改善。
- [ ] 按产品需求补齐 Project、Data、Chart、Plugin 等 Harness 写能力；复用各自 Application 用例，沿用批准、幂等、提交与恢复要求。
- [ ] 在有需求时增加界面意图取消、更多资源打开方式或多工作台目标；超时只表示没有完成证据，不宣称已取消正在执行的界面操作。
- [ ] CLI / MCP 后续客户端按 [Harness 路线图](STATISTICAL_HARNESS.md) 接入，不从当前内部工具存在推导生产 MCP 已完成。

业务协议、UI 页面增量、Graph 编辑和 Graph 投影保持独立。跨结果模板和通用组件扩展见 [JSON Driver](jsonDriver.md)。

[返回专项计划](README.md)

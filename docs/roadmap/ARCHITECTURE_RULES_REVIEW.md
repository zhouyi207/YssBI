# 架构规则循环复核

> Status: Planned
> Scope: 纯 Rust / GPUI 架构复核、历史结论的适用范围与原生剩余验收
> Canonical owners: 本文拥有审查范围和验收记录；源码、Cargo manifests 和各模块 README 拥有当前契约
> Update when: 具名架构问题、适用证据或人工验收状态改变时

## 当前审查基线

根 Cargo workspace 以 `crates/yss-desktop-gpui` 为默认入口。原生宿主调用
`ApplicationServices` 与类型化用例，在 worker 执行阻塞业务，再接纳类型化事件、投影和回执。
`react/` 仅为不参与原生构建的参考源码，不是当前状态 owner 或原生验收证据。

| 范围 | 当前 owner | 审查重点 |
| --- | --- | --- |
| 入口、控件、窗口与工作台 | [GPUI host](../../crates/yss-desktop-gpui/README.md) | 根 DockArea 唯一拥有拓扑；控件和异步任务具有明确生命周期 |
| 服务组装、项目切换与跨领域提交 | [Application](../../crates/yss-application/README.md) | 原子替换完整会话，拒绝旧会话回执；不在 UI 线程执行阻塞用例 |
| 当前图、历史与保存 | [Graph application](../../crates/yss-application/src/graph/README.md)、[Project](../../crates/yss-project/README.md) | Project 唯一拥有当前文档、undo/redo 和保存身份；视图不建立平行图草稿 |
| 语义、就绪与编辑投影 | [Graph Analysis](../../crates/yss-graph-analysis/README.md) | GraphSemanticSnapshot 是类型、Schema、血缘与诊断的唯一权威 |
| 运行、计划与结果 | [Graph Execution](../../crates/yss-graph-execution/README.md) | 计划在 Execute 内准备；结果适用性、租约与运行失败分别表达 |
| 节点与科学计算 | [Node Kernel](../../crates/yss-node-kernel/README.md)、[SCI](../../crates/yss-sci/README.md)、[SCI Runtime](../../crates/yss-sci-runtime/README.md) | 冻结注册表、中立调用、分配前预算；算法和展示不混合 |
| 数据查询与图表 | [Database application](../../crates/yss-application/src/database/README.md)、[Chart application](../../crates/yss-application/src/chart/README.md) | 查询绑定资源版本，迟到读取不覆盖新草稿或已删除资源 |
| 会话、工具与任务 | [Harness Core](../../crates/yss-harness-core/README.md) | 持久事件与账本是事实；模型和视图不伪造提交成功或执行授权 |
| 日志与插件 | [Logging](../../crates/yss-logging/README.md)、[Plugin Runtime](../../crates/yss-plugin-runtime/README.md) | 日志独立于 Problems/Results；插件安装、原生视图和任务各自验收 |

## 核对要求

按当前 manifests 和[模块索引](../reference/MODULE_MAP.md)确定受影响范围；不把历史文件数量、
一次全文阅读或编译通过当作全项目合规证明。每次修改在既有 owner 中闭合以下问题：

1. 结构和职责清晰，依赖朝领域与应用逻辑流动；领域不依赖 GPUI。
2. 公开接口最小，类型化输入与错误保留业务语义，不开放内部缓存或可变容器。
3. 每项已提交事实只有一个 owner；临时输入、不可变投影和结果租约不冒充第二状态权威。
4. 复用已有规则、查询基线和计算，删除确认无消费者的实现；不要为清理引入框架。
5. 函数与目录表达实际职责，不按文件长度、crate 数量或零警告机械拆分。
6. 更新全部调用方、测试和 owner README；不保留内部兼容门面或重复入口。
7. 会话、资源、语义、运行、结果、面板身份分别核验，失效回调不能操作后继实体。
8. 锁内只完成必要准入/发布，I/O、耗时计算、通知与资源释放遵循既有锁外边界。
9. 有界读取和分配前预算不等于进程硬内存限制；减少复制不等于实测帧率提升。
10. 人工交互、业务回归与性能测量分别留证，不用其中一项替代另一项。

## 已有结论及证据限制

- 2026-10-03 的原架构审查已按当时用户确认范围收尾。项目管理七项基本流程，以及画布/Mind、
  工作台与设置、资源编辑、数据与结果、Logs、Assistant、插件交互七组功能获用户确认通过。
  这些是当时界面的历史回执，不改写为 GPUI 已验收，也不要求重测已关闭的历史任务。
- 当时已定位的 IV、Panel、White/IM、HAC、ADF、项目回执等具名修复，以及真实 Julia 采样中途取消，
  已有当时的聚焦证据。详细实施过程由 Git 保留，当前算法契约以各 Rust owner README 为准。
  一般统计精度探索、未枚举算法组合和全文本地化校对不自动扩成无界架构任务。
- 真实采样取消曾在 Windows 的特定 Julia 包与 fixture 上观察到约 259 ms 的端到端终态延迟，
  包含 worker 终止和轮询成本；不是所有平台的延迟保证，更不是当前原生视图验收。
- 原四处参考实现 JSON 共享策略的局部测量只解释当时实现，见[基准资料](../benchmark/README.md)。
  不将其推广为 Rust 分配策略、GPUI 状态模型或原生渲染的规则例外。
- 用户明确要求跳过当轮画布 Performance 录制及量化分析：状态为**未测，按用户要求跳过**。
  该历史范围调整不代表当前 GPUI 已达性能目标，也不自动重新开启同一历史任务。

## 原生开放工作

当前详细逐项清单由 [GPUI 组件审查](GPUI_COMPONENT_AUDIT.md)与[迁移计划](GPUI_MIGRATION.md)
继续维护。此处保留跨 owner 的审查与验收责任，不用历史通过记录关闭以下事项：

- [ ] 随原生切换核对受影响模块及完整消费者，检查职责混合、重复规则、平行状态、无效抽象与残留调用；
  具名问题在所属 owner 中处理，不将旧全模块阅读次数计为本次完成证据。
- [ ] 拟保留的当前规则偏离必须有同语义行为与性能依据；参考实现的历史例外不自动适用。
- [ ] 画布：单/多节点拖动、连线反馈、缩放、Escape、隐藏/保存中断、松键一次提交、分屏视口与恢复；
  节点参数、常量精确值和 JSON 草稿在无关刷新时保持，切换目标时正确隔离。
- [ ] 资源与工作台：确认期间切换项目，同路径图关闭重开，图/Mind/Doc/Chart 保存与放弃，
  Details 目标回退、目录拖入、菜单快捷键、原生窗口焦点和未保存关闭保护。
- [ ] 数据与结果：导入嵌套流程及取消、连接失败保留输入、版本变化与迟到元数据、空 Schema/未知计数、
  分页/拖选/复制、图表迟到预览、报告追加、独立窗口与结果租约；外链及相关文件操作按实际原生入口验收。
- [ ] Logs：持续追加、筛选、Details、跟随滚动、主/独立窗口及恢复/终止提示；不得从日志重建运行或结果状态。
- [ ] Assistant：流式状态、工具与任务卡、来源引用、会话切换/关闭重开、失败恢复、进度和订阅释放，
  真实模型与 Worker 交付按 [Harness 计划](STATISTICAL_HARNESS.md)验收。
- [ ] 插件：当前原生视图的打开/关闭/重开与授权释放、任务取消及后继状态、公式及中英文参数含义；
  真实计算、安装维护和各平台进程行为分别验收，历史计算链证据不能替代新视图协议验收。
- [ ] 原生目标平台的物理键鼠、输入法、剪贴板、拖放、无障碍、缩放及打包；大图性能另记录输入延迟、
  布局/绘制与完整帧时间，不以 CPU 提交片段推断 120Hz 达标。
- [ ] 保留 Windows Application 测试曾出现的 `0xc0000409 / STATUS_STACK_BUFFER_OVERRUN` 排查：
  后续通过和功能确认不证明根因已修复；需要可定位事件或转储，不能通过串行通过宣称默认并发已通过。
- [ ] 每批最终核销消费者、文档与差异，并记录实际检查范围；局部结果不等于完整工作区或全部桌面通过。

## 工作入口

从仓库根目录以 `cargo run` 打开原生应用。具体包、测试目标、平台前置条件与示例命令由
[根 README](../../README.md)及 owner README 维护。验收使用项目副本与隔离应用数据。
遵循[根规则](../../.rules)和[架构复核流程](../development/ARCHITECTURE_GATES.md)，UI 采用人工验收。
本次文档整理未运行构建、测试、格式或人工交互检查，不产生新的通过记录。

[返回路线图](README.md)

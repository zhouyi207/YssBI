# GPUI 迁移

> Status: Planned
> Scope: Tauri/React 界面向 GPUI 原生界面的分阶段迁移与验收
> Canonical owners: 原生实现由 yss-desktop-gpui/README.md 拥有；现有业务契约仍由各 Rust 模块拥有
> Update when: 迁移阶段、未完成能力或验收状态变化时

迁移在 `gpui` 分支进行。原生宿主复用现有 Rust 业务和项目契约；不迁移旧格式，不另建 Graph
文档、历史、解析或执行权威。现有 Tauri 入口在原生功能达到替代条件前保留。
原生视觉与端口布局已完成本轮实现；继续沿用该布局补齐业务功能，人工验收条目保持开放。

- [x] 首阶段原生宿主实现与 Fedora 构建：独立启动入口、Project 只读快照、节点和 GPU 连线。
- [x] Fedora Event 图人工验收：用户确认节点与连线显示、右键平移、滚轮缩放及重置视图正常。
- [x] 原生视觉实现：统一主题与 SVG 图标、自绘标题栏、固定画布工具栏、分节属性和侧栏资源高亮；单侧端口固定贴对应边缘。
- [x] 端口位置与连线的 Fedora 人工验收：用户确认单侧与双侧端口分别贴 input 左、output 右，连线对齐正常。
- [ ] 其余视觉及属性交互的 Fedora 人工验收；图诊断中文模板与严重程度样式已接入，仍需确认实际显示。
- [ ] 首阶段补充验收：Function 图、空图、中键平移和关闭最后窗口退出。
- [x] Application 平台中立服务组装及 optional Tauri host；初始化与工作台 binding 的 9 个后端回归通过，原生入口没有 Tauri/WebView 依赖。
- [ ] 补齐完整项目生命周期及其人工验收。
- [ ] 迁移节点端口和布局、选择、框选、多选拖动、Escape 取消及节点配置输入。
- [ ] 复用现有图命令、历史、显式保存和发布回执；节点位置只在松键时提交。
- [ ] 迁移节点目录、连接兼容性反馈、连接替换、Reroute 和 Details。
- [x] Details 类型化参数及端口操作实现：开关、选项、常量引用、数列和列顺序、筛选、编码映射、默认值恢复、字面量与端口增删/重排；复用原投影和图事务。
- [ ] 类型化属性的 Fedora 人工验收；继续补齐创建前配置。
- [x] 原生新建 Event/Function 图、函数签名、图常量与引用拖动实现；复用现有 Project 事务、图历史及资源发布。
- [ ] 图属性的 Fedora 人工验收：函数参数及调用方、签名与正文保存关系、精确常量值、引用拖动及撤销/保存；继续完善复杂值的图形编辑器。
- [ ] 接入运行、Results 和 Output；Problems 已接入编辑器投影，仍需完整人工验收。
- [x] 原生运行、取消、快照恢复与 Output 接入现有 Application；结果目录、租约、标量/结构概览和每页 100 行虚拟表格完成实现与构建检查。
- [ ] 运行、失败定位、结果分页与租约关闭的 Fedora 人工验收；补齐完整报告、分析、图形与独立结果窗口。
- [ ] 中立日志运行时及原生 Logs：原生构建、两个持久化回归和宿主编译通过，仍需人工验收。
- [ ] 迁移项目浏览器、工作台布局、窗口和焦点；原生工作台建立单一拓扑 owner。
- [ ] 迁移表格、图表、Mind、Markdown、Assistant 和插件界面。
- [ ] Fedora/Windows/macOS 验收输入法、剪贴板、拖放、文件对话框、缩放、无障碍与打包。
- [ ] 用同一真实图记录原生输入延迟和呈现表现，确认目标平台性能收益。
- [ ] 达到替代条件后切换默认入口，移除不再使用的 Tauri/React 代码和依赖。

当前实现能力与限制见 [GPUI host](../../src-tauri/crates/yss-desktop-gpui/README.md)。
首阶段预览不意味着编辑器、应用服务或所有平台迁移已经完成。

## 当前变更验证

- `pnpm build:gpui`：原生宿主构建通过。
- `pnpm test:rs:package -p yss-application --no-default-features --lib runtime::tests::`：4 个初始化回归通过。
- `pnpm test:rs:package -p yss-application --no-default-features --lib presentation::tests::`：5 个绑定、认领与释放回归通过。
- `pnpm test:rs:package -p tauri-plugin-tracing --no-default-features --lib persistence_tests::`：2 个日志提交/恢复/失败回归通过。
- `pnpm test:rs:package -p yss-application --no-default-features --lib graph::run::tests::`：4 个执行、准入、取消与会话隔离回归通过。
- `pnpm test:rs:package -p yss-application --no-default-features --lib graph::results::`：17 个结果、分页、依赖失效与报告读取回归通过。
- `pnpm test:rs:package -p yss-graph-editor --lib parameter` 与 `--lib port`：2 个参数规则/重置/撤销及 4 个端口/成员/投影回归通过。
- `pnpm test:rs:package -p yss-project --lib project_state::function_mutation::tests::`：3 个签名版本准入、并发发布及失败原子性回归通过。
- `pnpm test:rs:package -p yss-graph-editor --lib tests::constant_edits_preserve_reference_identity_and_reject_duplicate_names_atomically`：1 个常量引用身份、重复名称拒绝及原子性回归通过。
- `pnpm check:rs:package -p yssbi --lib`：现有 Tauri 调用方编译通过。
- `pnpm lint:rs:package -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 与
  `pnpm lint:rs:package -p tauri-plugin-tracing --no-default-features --lib --no-deps -- -D warnings`：改动宿主与日志 crate 的严格 lint 通过。
- `pnpm format:rs:package -p yss-desktop-gpui -p yss-application -p tauri-plugin-tracing -- --check`：改动 Rust 包格式检查通过。
- `pnpm test:ts src/tests/documentationContract.test.ts`：6 个文档契约通过；
  `pnpm docs:crate-dependencies:check` 与 `git diff --check` 通过。

原生依赖树验证没有 Tauri runtime、Wry、WebKitGTK、GTK 或 Tauri plugin build dependency。
包含依赖的严格 lint 首次被未改动 `yss-sci` 的 4 个既有警告阻塞，改动 crate 按上述范围单独通过；
未运行全量 CI。此前编辑窗口已关闭，当前执行验收副本为 `/tmp/yssbi-gpui-run-rf665bh6/project`，
应用数据也单独隔离。此前执行窗口已关闭；新的图属性与视觉预览已使用该副本启动，运行与编辑功能的完整验收仍未完成。
原项目未用于编辑验收。

# YssBI Open Backlog

> Status: Planned
> Scope: 尚未完成且未归入专项 roadmap 的跨领域工作
> Canonical owners: 本文件只拥有开放任务；代码和 Current docs 定义当前行为
> Update when: 添加、完成、取消、澄清或迁移开放任务时

只在这里记录仍需完成的工作。完成记录由 Git 和 `docs/version/` 保存；release/subsystem 计划放在 `docs/roadmap/`；实现细节和验证输出不追加到本文件。

## Active tasks

- [ ] 补齐目录中 DataFrame 选列、求和节点的执行实现及选列类型推断；当前 `yssbi.dataframe.series.select` 输出泛型无法解析，且这两个节点均未注册到执行器。
- [ ] 按 [Tolerance 分析](docs/reviews/2026-09-07-tolerance-analysis.md) 处理数值策略：优先修复判秩失败回退和 Prais 迭代上限被当作成功，再统一模型级秩不足策略、模型参数与报告，最后评估 SVD 重复计算。
- [ ] 清理 Rust Clippy 基线：统计代码的既有诊断分布在 `yss-sci` 和迁出的 `yss-sci-runtime::data`，`yss-ipc-command` 也有既有诊断。数值循环/模型参数重构需结合 SCI golden tests；传输参数和 wire 枚举须保持 IPC 契约，不能为消除 lint 随意改协议。
- [ ] 评估前端既有的 14 条 Oxlint 警告；涉及遍历集合副本和测试 observer 的条目应先确认快照/回调语义，再决定简化或注明必要原因。
- [ ] 按需核对 [legacy TODO snapshot](docs/version/legacy-todo-2026-09-04.md)，只把经当前代码验证仍有效的条目迁回本文件或对应 roadmap；不要把历史 change summary 重新标为开放任务。

## Routed roadmaps

- [Statistical Harness](docs/roadmap/STATISTICAL_HARNESS.md)
- [v0.3](docs/roadmap/v0_3.md)
- [v1.0](docs/roadmap/v1_0.md)


数据驱动：Immer 库处理 json


1. json-render：最贴近你的目标

Vercel Labs 的 json-render 本身就是：

JSON Spec → Component Registry → React Components

而且它明确支持“AI 只能使用你注册过的组件”，非常适合你以后 YssBI 让 AI 生成 UI。


2. Zod：强烈建议一起使用

你的 JSON Renderer 最大风险不是渲染，而是：

JSON 是不是合法？

尤其以后 AI 生成：


3. Zustand + Immer：负责“JSON 局部变化”

这个还是保留。

整个架构建议是：


4. JSON Patch：我也很推荐

如果后端或者 AI 会不断修改页面：

不要：.图表格式从 schema 3 升到 4






这三段是在 向 Tauri 注册三个应用级共享状态。后续 IPC 命令通过 State<T> 或 app.state::<T>() 获取同一个实例。

  1. ProjectWatcherState：管理当前项目的文件监听

  ProjectWatcherState::new(
      Arc::new(NotifyProjectFileWatcher::new()),
  )

  这里分成两个角色：

  - ProjectWatcherState 管理监听的启动、切换、停止，以及屏蔽旧项目的延迟事件。
  - NotifyProjectFileWatcher 使用 notify 库实际监听操作系统文件变化。

  这行只是组装并注册。项目加载等流程调用 watch_project() 后才开始监听；收到变化后，通过 Application 更新项目索引，再通知
  前端。

  实现见 ProjectWatcherState (src-tauri/crates/yss-project-watcher/src/lib.rs:217)。

  2. ProjectTaskCancellationRegistry：管理项目选择器任务的取消

  它记录当前正在执行的扫描或清理任务：

  - begin()：创建取消标记，同时取消前一个任务。
  - cancel_active()：请求取消当前任务。
  - end()：任务结束后清除对应记录。

  例如，用户扫描目录寻找项目时点击“取消”，IPC 命令会通过这个共享状态发出取消请求。任务自己检查标记并退出，不会强制杀死线
  程。

  实现见 取消管理器 (src-tauri/crates/yss-project-progress/src/lib.rs:60)。

  3. ActivityPanelSyncState：缓存侧栏面板的增量同步基线

  它服务于项目、节点、命令、插件等侧栏面板的数据传输：

  - 前端首次请求：返回完整 Snapshot。
  - 前端携带上次的 cursor 请求：与缓存基线比较，返回 Patch。
  - 没有匹配基线：重新返回完整快照。
  - 数据没变化：返回空补丁。

  缓存区分窗口、语言、面板和项目，并有数量与内存上限。它保存的是用于传输的面板投影，不负责保存项目文件或管理 Dockview 布
  局。

  实现见 ActivityPanelSyncState (src-tauri/crates/yss-ipc-command/src/activity_panel_sync.rs:16)，调用入口是
  get_activity_panel_document (src-tauri/crates/yss-ipc-command/src/commands/command_activity_panel.rs:26)。



     Crate                                   当前具体职责                             主要依赖
  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   yss-sci-contract (src-tauri/crates/     共享统计输入、缺失值策略、OLS 配置与     serde、thiserror
   yss-sci-contract/src/lib.rs)            报告类型、错误、取消与截止时间，以及
                                           ScientificBackend 接口
  ──────────────────────────────────────  ───────────────────────────────────────  ─────────────────────────────────────
   yss-sci (src-tauri/crates/yss-sci/      数值模型和算法：OLS/WLS/GLS、工具变量    sci-contract、linalg、faer、statrs
   src/lib.rs)                             回归、Logit/Probit、面板模型、ACF/
                                           PACF、序列检验、ADF、VAR/VEC 等
  ──────────────────────────────────────  ───────────────────────────────────────  ─────────────────────────────────────
   yss-sci-runtime (src-tauri/crates/      将调用方输入整理为模型输入，调用算       sci、sci-contract、linalg、Arrow 等
   yss-sci-runtime/src/lib.rs)             法、转换错误、生成报告；提供
                                           SciRuntimeBackend；还包含 Arrow 数据
                                           准备和部分算法
  ──────────────────────────────────────  ───────────────────────────────────────  ─────────────────────────────────────
   yss-sci-linalg (src-tauri/crates/        线性代数公共约定：带检查的矩阵分解、     faer 等
   yss-sci-linalg/src/lib.rs)               求解、秩判定和统一数值错误

  yss-sci-linalg 并没有包办全部矩阵运算。SCI 也直接使用 faer，只有需要统一分解检查、秩和错误语义的地方才使用 yss-sci-linalg。



  1. SCI 通过 linalg 使用线性代数能力，合理，但要明确隔离程度。

     职责可以划分为：
      - linalg：矩阵、向量、乘法、分解、求解、秩和条件数。
      - sci：模型拟合、协方差估计、统计检验、时间序列算法。
      - faer：linalg 内部使用的具体实现。

     需要注意，当前 SCI 大量使用 faer::Mat、Col 和借用视图。让 linalg 重新导出这些类型，可以统一导入入口，但 SCI 仍然依
     赖 faer 的类型和方法语义。如果希望真正隐藏 faer，就需要由 linalg 提供项目自己的矩阵接口，封装项目实际使用的操作，并
     避免引入重复数据存储。

     这项改动的价值取决于目标：集中管理数值依赖可以先采用轻量封装；需要替换底层库时，才需要更完整的类型隔离。

  2. runtime 移除直接的 linalg 调用，应该做，同时也要处理它对 faer 的直接依赖。

     当前 runtime 自己计算部分模型的预测值、残差和条件数。相关代码 (src-tauri/crates/yss-sci-runtime/src/regression/
     mod.rs:428)

     这些应由 SCI 的模型计算完成，并随拟合结果返回。runtime 负责：
      - 将 Arrow、普通向量等输入整理为科学计算请求。
      - 管理计算调用的预算和错误转换。
      - 将 SCI 结果映射为报告和应用需要的结果。

     这里还涉及公共接口调整。例如 runtime 的假设检验输入直接暴露了 faer::Mat 和 Col，Application 因此也需要认识 faer。当
     前接口 (src-tauri/crates/yss-sci-runtime/src/hypothesis.rs:6)

     要贯彻你的分层，runtime 对外应提供中性输入，矩阵构造和计算归入 SCI。

  3. 插件调用宿主的 Rust 计算能力统一经过 runtime，合理；只需要契约的插件模块应允许直接依赖 Contract。

     我建议允许：

     插件需要宿主计算能力 → sci-runtime → sci → linalg → faer
     插件只需要共享类型   → sci-contract

     插件不直接引用宿主的 sci、linalg，这样可以避免绕过统一的计算入口。

     但当前 Julia 插件中的 worker 契约只需要 StatisticalInput、取消和截止时间等类型。强制它改为依赖 sci-runtime，会把
     Rust 算法和矩阵库加入这些契约模块的构建依赖。现有依赖 (plugins/julia/native/crates/yss-bayes-worker/Cargo.toml)

     Julia 自己的计算仍应保留：

     BayesRuntime → BayesWorkerPort → JuliaAdapter → Julia进程

     它只有在需要复用宿主 Rust 算法时，才需要依赖 sci-runtime。

  这套规则可以落实为：底层数值实现集中在 linalg，统计事实由 sci 产生，宿主科学计算入口由 sci-runtime 提供，中性契约独立
  共享。 实施时需要同步搬移计算职责和调整公共类型，不能只修改 Cargo 依赖声明。



  收紧这条依赖链是合理的。前两项我赞成；插件限制建议调整为：调用宿主科学计算使用 sci-runtime，共享类型仍允许使用 sci-
  contract。

  1. SCI 通过 linalg 使用线性代数能力，合理，但要明确隔离程度。

     职责可以划分为：
      - linalg：矩阵、向量、乘法、分解、求解、秩和条件数。
      - sci：模型拟合、协方差估计、统计检验、时间序列算法。
      - faer：linalg 内部使用的具体实现。

     需要注意，当前 SCI 大量使用 faer::Mat、Col 和借用视图。让 linalg 重新导出这些类型，可以统一导入入口，但 SCI 仍然依
     赖 faer 的类型和方法语义。如果希望真正隐藏 faer，就需要由 linalg 提供项目自己的矩阵接口，封装项目实际使用的操作，并
     避免引入重复数据存储。

     这项改动的价值取决于目标：集中管理数值依赖可以先采用轻量封装；需要替换底层库时，才需要更完整的类型隔离。

  2. runtime 移除直接的 linalg 调用，应该做，同时也要处理它对 faer 的直接依赖。

     当前 runtime 自己计算部分模型的预测值、残差和条件数。相关代码 (src-tauri/crates/yss-sci-runtime/src/regression/
     mod.rs:428)

     这些应由 SCI 的模型计算完成，并随拟合结果返回。runtime 负责：
      - 将 Arrow、普通向量等输入整理为科学计算请求。
      - 管理计算调用的预算和错误转换。
      - 将 SCI 结果映射为报告和应用需要的结果。

     这里还涉及公共接口调整。例如 runtime 的假设检验输入直接暴露了 faer::Mat 和 Col，Application 因此也需要认识 faer。当
     前接口 (src-tauri/crates/yss-sci-runtime/src/hypothesis.rs:6)

     要贯彻你的分层，runtime 对外应提供中性输入，矩阵构造和计算归入 SCI。

  3. 插件调用宿主的 Rust 计算能力统一经过 runtime，合理；只需要契约的插件模块应允许直接依赖 Contract。

     我建议允许：

     插件需要宿主计算能力 → sci-runtime → sci → linalg → faer
     插件只需要共享类型   → sci-contract

     插件不直接引用宿主的 sci、linalg，这样可以避免绕过统一的计算入口。

     但当前 Julia 插件中的 worker 契约只需要 StatisticalInput、取消和截止时间等类型。强制它改为依赖 sci-runtime，会把
     Rust 算法和矩阵库加入这些契约模块的构建依赖。现有依赖 (plugins/julia/native/crates/yss-bayes-worker/Cargo.toml)

     Julia 自己的计算仍应保留：

     BayesRuntime → BayesWorkerPort → JuliaAdapter → Julia进程

     它只有在需要复用宿主 Rust 算法时，才需要依赖 sci-runtime。

  这套规则可以落实为：底层数值实现集中在 linalg，统计事实由 sci 产生，宿主科学计算入口由 sci-runtime 提供，中性契约独立
  共享。 实施时需要同步搬移计算职责和调整公共类型，不能只修改 Cargo 依赖声明。

# 功能开发检查清单

本清单用于新增功能或改变现有行为时的设计、自审和交付。它不替代
[架构说明](../architecture/ARCHITECTURE.md)、
[诊断、错误与输出契约](../architecture/DIAGNOSTICS_ERRORS_AND_OUTPUT.md)或
[本地开发工作流](LOCAL_WORKFLOW.md)。与功能无关的条目标记为“不适用”并说明原因，
不要为了勾选清单引入额外抽象。

## 1. 问题与范围

- [ ] 记录真实问题及证据：用户路径、缺陷复现、性能数据或明确需求。
- [ ] 用一句话写清功能：谁在什么场景执行什么操作，并得到什么可观察结果。
- [ ] 写出可验证的验收标准，以及本次明确不处理的事项。
- [ ] 检查现有能力和依赖，优先复用已有路径；不要建立第二事实源或临时兼容路线。
- [ ] 判断这是行为变更、行为保持重构，还是纯配置/文档变更，并据此确定测试范围。

## 2. 权威状态与模块归属

- [ ] 指定每项状态的唯一 owner。项目、持久化、图、执行、数据库、结果和科学计算
      的权威状态属于 Rust；React 只保存交互状态和后端投影。
- [ ] 确认 React 状态应进入领域 store、临时组件状态还是 `localStorage`，不复制
      Dockview 或 Rust 已拥有的事实。
- [ ] 前端 window/panel/editor UI 放入 `modules/<name>/internal` 并只从根 `public.ts` 暴露；
      跨业务组合放入 `app/`，workflow、domain、projection/runtime state 分别进入
      `features/application/`、`features/domain/`、`features/core/`，IPC adapter 进入 `services/`，
      保持依赖单向流动。
- [ ] Rust 工作流由 application/domain owner 承担；Tauri command 只负责输入校验、
      调用、DTO 映射和事件发送，不承载文件 I/O 或长流程。
- [ ] 明确基础设施依赖的注入位置，避免 domain 反向依赖 UI、Tauri 或具体 adapter。

## 3. IPC 与异步契约

- [ ] 根据语义选择 command、event 或 channel/worker：请求响应使用 command，低频
      状态变化使用 event，有序或高频流使用 channel/worker。
- [ ] IPC 边界只传可序列化 DTO；前端通过 `src/services/` 调用，并让普通 invoke
      经过 `src/services/ipc/invokeCommand.ts`。
- [ ] Rust owner 使用 `thiserror` 类型化错误，并只在 command/IPC seam 映射一次。
- [ ] 命令失败保持精确的 `{ code, details, incidentId }` wire；成功 DTO 和异步状态
      不携带后端用户文案，也不通过 `message`、`detail` 或 `hint` 偷渡文本。
- [ ] 新增或修改 stable code、DTO、event/channel payload 时，同步 Rust、TypeScript、
      本地化映射和边界测试；0.x 变更直接替换旧路线，不保留双协议。
- [ ] 对有序流明确容量、背压、丢弃策略、终止事件和来源身份。

## 4. 生命周期、持久化与身份

- [ ] 说明功能是否修改项目文件、注册表、图资源或历史，并定义提交点、原子性、
      失败恢复和保存/关闭行为。
- [ ] 将 `graphPath` 和资源路径视为 opaque value；不要从字符串格式推导资源类型、
      加载状态或 UI 身份。
- [ ] 区分 graph/resource ID、node/pin/connection UUID、`panelInstanceId`、Dockview
      group ID 和 graph session；不要在这些身份之间转换或复用。
- [ ] 图打开仍遵守 load → runtime bind → dynamic pin materialization → frontend
      hydration；不能仅从 `graphEntities` 推断后端 resident state。
- [ ] 乐观更新只抑制与 pending key 匹配的回声，其他 project event 必须正常投影。
- [ ] 定义项目替换、窗口/面板关闭、任务取消、超时和过期 session 的行为；明确
      commit point-of-no-return 之后哪些操作不可撤销。
- [ ] 全局锁只用于短快照或短提交；文件 I/O、sleep、模型加载、数据库扫描和长任务
      必须在锁外执行，并在提交前重新验证 currentness。

## 5. 诊断、错误、结果与程序输出

- [ ] 应用错误由 React 根据 stable code/details 本地化，并选择页面/区块 `Alert`、
      字段内反馈或单按钮 `MessageDialog`；破坏性确认才使用 `AlertDialog`。
- [ ] 内部诊断只走 Rust `tracing`，保持有界、可丢弃、脱敏且不参与领域决策。
- [ ] 科学计算产物进入 Results；用户控制的 Workflow/tool stdout/stderr 进入有序有界的
      Run Output channel 和 Output panel；两者都不能改走 diagnostic logs。
- [ ] 日志和错误 details 不包含表格行/单元格、文档、剪贴板、SQL、连接字符串、
      token 或其他秘密；输出事件保留 opaque graph/node 来源身份。

## 6. 工作台、交互与平台行为

- [ ] root `DockviewReact` 继续唯一拥有 panel/group 拓扑、顺序、激活态、edge 状态
      和布局恢复；不在 Zustand 建立 layout mirror。
- [ ] 面板元数据使用 `panelInstanceId` 关联资源，允许同一资源出现在多个 group；
      所有关闭操作先经过 dirty/save confirmation。
- [ ] 普通控件优先组合现有 shadcn/ui，图标复用 `react-icons/vsc`，用户滚动区域
      使用项目 `ScrollArea` 并保留 `flex`/`min-h-0`/`flex-1` 契约。
- [ ] 覆盖键盘操作、焦点顺序、可访问名称、disabled/loading/error 状态和必要的
      屏幕阅读器语义。
- [ ] 所有用户文案进入中英文资源；Rust 不拥有应用错误文案。
- [ ] 检查 Windows、macOS 和 Linux 的路径、快捷键、文件对话框、窗口及打包差异；
      全局 `window`/`document` listener 统一经过 `src/shared/utils/globalEvent.ts`。

## 7. 数据规模与长任务

- [ ] 写出预期 graph、table、result 和并发 task 规模，不以小样例假定生产负载。
- [ ] 大表使用分页、投影、虚拟化或批量传输，避免通过 IPC 复制完整 DataFrame。
- [ ] 高频更新有容量上限、合并/节流或背压策略，不让日志、结果或 UI 队列无限增长。
- [ ] 长任务支持可观察进度和合理取消；取消、项目替换与 late result 不得污染新 session。
- [ ] 性能敏感变更使用代表性数据验证；列统计等既有路径运行对应 benchmark 并记录基线差异。

## 8. 测试范围

- [ ] 行为变更先添加最小聚焦回归测试，再修改实现；行为保持重构优先依赖现有覆盖。
- [ ] 通过稳定 public seam 测试项目自有契约，不测试私有实现、语言、框架或第三方行为。
- [ ] 每个测试对应一个独立回归风险；新增超过两个测试前说明额外用例捕获的不同故障。
- [ ] 跨 IPC 或事件边界时至少覆盖代表性的成功/失败 contract，避免前后端字段漂移。
- [ ] 不添加穷举矩阵、边缘样例堆叠或仅证明历史重构发生过的测试。

## 9. 依赖、许可与分发

- [ ] 先检查标准库、平台能力和已有依赖；只有现有方案不能满足需求时才新增 dependency。
- [ ] 核对许可证、维护状态、Rust/Node 版本兼容性、二进制体积和 transitive dependency。
- [ ] 同步 lockfile，并检查 Tauri permission/capability、平台原生依赖和 release workflow。
- [ ] 外部 API 明确网络、费用、速率限制、离线行为、数据共享和 secret 注入方式；不硬编码密钥。

## 10. 交付验证

按实际改动从聚焦到广泛执行，并保存新鲜输出：

- [ ] 前端：`pnpm format:check:ts`、`pnpm check:ts`、`pnpm lint:ts` 与受影响的
      `pnpm test:ts <path> [-t <name>]`。
- [ ] Rust/Tauri：`pnpm format:check:rs`、`pnpm check:rs`、`pnpm lint:rs` 与
      `pnpm test:rs --lib test_name` 或
      `pnpm test:rs --test integration_test test_name`。
- [ ] SCI：受影响时通过 `pnpm test:rs [filter]` 运行测试；性能敏感时从根目录
      运行对应的 `cargo bench --manifest-path src-tauri/Cargo.toml ...`。
- [ ] 跨前后端、发布或执行引擎跨切面交付：`pnpm run ci`。
- [ ] 打包、权限或插件：`pnpm build` 并在目标平台手动验证关键路径。
- [ ] 未运行的检查记录原因；交付前运行 `git diff --check`，并在当天 `TODO.md`
      日期标题下追加多条修改摘要。

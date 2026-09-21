# 架构复核与文档检查

> Status: Current
> Scope: 前后端依赖边界的人工复核、业务验证和独立文档契约
> Canonical owners: 当前架构与 Agent Rules 拥有跨系统约束；文档契约测试拥有可执行文档检查
> Update when: 架构复核范围、验证方式或文档检查入口改变时

前后端均已移除源码架构审计、逐文件/逐符号的测试许可表及其专用扫描器。
状态归属、依赖方向和业务边界继续按[当前架构](../architecture/ARCHITECTURE.md)、[Agent Rules](AGENT_RULES.md)与子系统契约执行。

## 1. 架构复核

变更时检查实际 import/re-export、Cargo 依赖、模块可见性和调用方，确认职责仍归其现有 owner：

- 桌面负责组装，IPC 适配调用 Application 用例，领域不依赖 UI/Tauri。
- 前端 app 组合模块，Application 编排操作，Core/Domain 保存既有状态与规则，Services 适配 IPC。
- React 安装后端只读投影，Graph/Execution/Results、页面状态和工作台 FlexLayout 各有独立权威。
- 科学计算、插件、项目、数据库和文件系统遵循各自专项契约，不通过扩大依赖范围绕过边界。

完整规则以链接的架构文档为准，不在本文件复制一份许可表。
Cargo/TypeScript 编译检查验证类型和依赖可解析性，业务测试验证行为，二者均不能自动证明所有架构方向正确。

## 2. 按变更选择验证

依照 [Local Workflow](LOCAL_WORKFLOW.md) 选择 L1/L2 验证：

- TypeScript 修改运行类型检查、lint 和受影响的业务测试。
- Rust 修改显式选择受影响 package、target 和消费者，运行编译、Clippy 与业务测试。
- 公开契约变更检查直接与间接调用方，覆盖身份、提交、失败和恢复等真实回归风险。
- 界面交互遵守仓库规则，使用人工验收。
- 文档、生成索引和命令入口修改运行对应文档或生成器检查。

不再为每次重构新增证明旧名称已删除的全仓字符串检查，也不重建源码架构扫描框架。
没有用例的目标只报告编译结果，零匹配不能作为测试通过；未执行的验收保持未完成状态。

## 3. 文档契约

独立文档检查位于 `src/tests/documentationContract.test.ts`，覆盖 `docs/` 与文档索引直接指向的模块 README，不依赖已移除的架构扫描器。
从仓库根目录运行：

```sh
pnpm test:ts src/tests/documentationContract.test.ts
```

它继续保护：

- 系统、开发文档与索引中的模块 README 使用明确的 owner 和生命周期状态；
- 相对链接和明确 source path 存在，模块 README 可以使用模块内相对源码路径；
- 文档中的 root `pnpm` 命令对应 script 或 package-manager builtin；
- Agent 入口文件指向 `.rules`；
- roadmap、当前架构和已接受目标使用正确状态；
- generated `docs/reference/MODULE_MAP.md` 与 Cargo metadata/目录一致。

架构文档可以是 `Current`，或明确声明 `Contract: Target Architecture` 的 `Accepted Decision`；
开发工作流使用 `Current`。目标架构和历史验证记录不能作为当前生产完成证据。

[Module Map](../reference/MODULE_MAP.md) 和应用中的 crate 依赖图继续由已有生成器维护；
这些索引不执行源码架构审计。文档检查也不代替业务测试或界面人工验收。

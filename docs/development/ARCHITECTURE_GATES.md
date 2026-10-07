# 架构复核与文档检查

> Status: Current
> Scope: 原生宿主依赖边界、业务验证与文档生成检查
> Canonical owners: 当前架构与 Agent Rules 拥有跨系统约束；Cargo metadata 与文档生成器拥有可执行入口检查
> Update when: 架构复核范围、验证方式或文档检查入口改变时

前后端均已移除源码架构审计、逐文件/逐符号的测试许可表及其专用扫描器。
状态归属、依赖方向和业务边界继续按[当前架构](../architecture/ARCHITECTURE.md)、[Agent Rules](AGENT_RULES.md)与子系统契约执行。

## 1. 架构复核

变更时检查实际 import/re-export、Cargo 依赖、模块可见性和调用方，确认职责仍归其现有 owner：

- GPUI 桌面负责组装并直接调用 Application 用例，领域不依赖 GUI。
- 原生 services 调度阻塞调用与事件，Application 编排操作，领域 owner 保留既有事实。
- 原生视图安装后端只读投影；Graph/Execution/Results 与根 DockArea 各有独立权威。
- 科学计算、插件、项目、数据库和文件系统遵循各自专项契约，不通过扩大依赖范围绕过边界。

完整规则以链接的架构文档为准，不在本文件复制一份许可表。
Cargo 编译检查验证类型和依赖可解析性，业务测试验证行为，二者均不能自动证明所有架构方向正确。

## 2. 按变更选择验证

依照[根验证规则](../../.rules)选择 L1/L2 范围，具体命令见[根 README](../../README.md)、[GPUI host](../../crates/yss-desktop-gpui/README.md)及受影响模块的说明：

- Rust 修改显式选择受影响 package、target 和消费者，运行编译、Clippy 与业务测试。
- 公开契约变更检查直接与间接调用方，覆盖身份、提交、失败和恢复等真实回归风险。
- 界面交互遵守仓库规则，使用人工验收。
- 文档、生成索引和命令入口修改运行对应文档或生成器检查。

不再为每次重构新增证明旧名称已删除的全仓字符串检查，也不重建源码架构扫描框架。
没有用例的目标只报告编译结果，零匹配不能作为测试通过；未执行的验收保持未完成状态。

## 3. 文档与入口检查

从仓库根目录运行生成器的只读检查：

```sh
node scripts/generate-crate-dependencies.mjs --check
```

它以根 Cargo metadata 为唯一 workspace 来源，保护模块索引及保留的依赖图数据。
文档改动同时检查相对链接、owner 与状态声明；移动模块后修正其入口，不把参考 React
说明当作原生功能完成证据。旧 `react/src/tests/documentationContract.test.ts` 保留为参考，
不作为当前原生构建的必需检查；根目录已没有 pnpm scripts。

架构文档可以是 `Current`，或明确声明 `Contract: Target Architecture` 的 `Accepted Decision`；
开发工作流使用 `Current`。目标架构和历史验证记录不能作为当前完成证据。
生成索引不执行源码架构审计，也不代替业务测试或人工验收。

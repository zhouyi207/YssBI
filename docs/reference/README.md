# 实现参考

> Status: Current
> Scope: 生成的模块索引和源码旁专项说明的导航
> Canonical owners: 模块清单由生成器维护；源码旁 README 拥有具体模块说明
> Update when: 实现说明入口或生成方式改变时

[模块索引](MODULE_MAP.md)从根 Cargo metadata 生成。运行
`node scripts/generate-crate-dependencies.mjs` 更新，使用同一命令的 `--check` 只读校验；
不手工维护另一份 workspace 清单。生成器同时更新 `react/` 中保留的依赖图数据，
该参考数据不参与原生应用启动或构建。

## 原生宿主与应用

- [GPUI 原生宿主](../../crates/yss-desktop-gpui/README.md)
- [Application 用例与会话](../../crates/yss-application/README.md)
- [结构化日志与运行观测](../../crates/yss-logging/README.md)

## 项目、节点与数据

- [Project runtime](../../crates/yss-project/README.md)
- [Filesystem primitives 与 watcher lifecycle](../../crates/yss-filesystem/README.md)
- [Node definitions、registry 与 catalog](../../crates/yss-node-catalog/README.md)
- [Node kernel contracts](../../crates/yss-node-kernel/README.md)
- [Database runtime](../../crates/yss-database-runtime/README.md)
- [Dataset snapshot store](../../crates/yss-database-store/README.md)

## 科学计算与插件

- [SCI neutral contracts](../../crates/yss-sci-contract/README.md)
- [SCI numerical models](../../crates/yss-sci/README.md)
- [SCI synchronous runtime](../../crates/yss-sci-runtime/README.md)
- [Linear algebra boundary](../../crates/yss-sci-linalg/README.md)
- [插件运行时](../../crates/yss-plugin-runtime/README.md)与[共享插件协议](../../crates/yss-plugin-protocol/README.md)

系统关系见[架构索引](../architecture/README.md)，模块契约统一进入源码旁 README；Graph、Workbench、Results、Harness 与 UI 页面等入口见[模块契约导航](../README.md#模块契约入口)。测量与样本见[基准索引](../benchmark/README.md)。

[返回文档索引](../README.md)

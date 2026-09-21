# 实现参考

> Status: Current
> Scope: 生成的模块索引和源码旁专项说明的导航
> Canonical owners: 模块清单由生成器维护；源码旁 README 拥有具体模块说明
> Update when: 实现说明入口或生成方式改变时

[模块索引](MODULE_MAP.md)从 Cargo metadata 和前端模块目录生成。运行 `pnpm docs:module-map` 更新，使用 `pnpm docs:module-map:check` 只读校验；不手工维护平行模块清单。

## 前端与应用

- [前端源码](../../src/README.md)与[分层边界](../../src/features/README.md)
- [Application 用例与会话](../../src-tauri/crates/yss-application/README.md)
- [Tauri / IPC transport](../../src-tauri/crates/yss-application/src/ipc/README.md)
- [结构化日志与运行观测](../../src-tauri/crates/tauri-plugin-tracing/README.md)

## 项目、节点与数据

- [Project runtime](../../src-tauri/crates/yss-project/README.md)
- [Filesystem primitives 与 watcher lifecycle](../../src-tauri/crates/yss-filesystem/README.md)
- [Node definitions、registry 与 catalog](../../src-tauri/crates/yss-node-catalog/README.md)
- [Node kernel contracts](../../src-tauri/crates/yss-node-kernel/README.md)
- [Database runtime](../../src-tauri/crates/yss-database-runtime/README.md)
- [Dataset snapshot store](../../src-tauri/crates/yss-database-store/README.md)

## 科学计算与插件

- [SCI neutral contracts](../../src-tauri/crates/yss-sci-contract/README.md)
- [SCI numerical models](../../src-tauri/crates/yss-sci/README.md)
- [SCI synchronous runtime](../../src-tauri/crates/yss-sci-runtime/README.md)
- [Linear algebra boundary](../../src-tauri/crates/yss-sci-linalg/README.md)
- [Julia 插件开发](../../plugins/julia/README.md)与[Bayes worker protocol](../../plugins/julia/runtime/julia/README.md)

跨模块契约见[架构索引](../architecture/README.md)，测量与样本见[基准索引](../benchmark/README.md)。

[返回文档索引](../README.md)

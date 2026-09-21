# 前端入口

> Status: Current
> Scope: 前端职责入口、类型检查、测试和格式化命令
> Canonical owners: package.json 拥有脚本定义，各模块 README 拥有对应功能契约
> Update when: 前端入口、工具链或验证命令改变时

`app/` 组合窗口与路由，`modules/` 提供界面，`features/application/` 编排用例，`features/core/` 保存投影与交互状态，`features/domain/` 保存纯领域规则，`services/` 适配 IPC。

Rust 拥有已提交项目状态及当前 Graph 文档、历史与保存身份。查询返回数据；修改返回提交结果，事件可交付同一提交的回声，前端由发布协调器统一去重和更新投影。React 保留图的只读投影与临时交互，其他资源的配置草稿遵循各自模块契约。

依赖方向和状态归属见[当前架构](../docs/architecture/ARCHITECTURE.md)，验证范围见[根规则](../.rules)。

## 前端验证

从仓库根目录运行：[package.json](../package.json) 中的 TypeScript 检查使用 tsc，lint 使用 Oxlint，格式化使用 Oxfmt，测试使用 Vitest。

```sh
pnpm check:ts
pnpm lint:ts
pnpm test:ts <test-file>
pnpm test:ts <test-file> -t "test name"
pnpm format:check:ts <changed-files>
pnpm format:ts <changed-files>
```

替换占位文件和用例名。`test:ts` 将参数透传给 Vitest；确认实际匹配了相关用例。
`lint:ts` 固定扫描整个前端，追加文件名不能缩小范围。界面交互使用人工验收。

文档检查使用 `pnpm test:ts src/tests/documentationContract.test.ts`，并包含模块索引的只读校验。
Graph 诊断模板的生成归 [yss-graph-diagnostics](../src-tauri/crates/yss-graph-diagnostics/README.md)，插件投影 schema 归 [yss-plugin-protocol](../src-tauri/crates/yss-plugin-protocol/README.md)；本地化专项约束见 [i18n 规则](app/i18n/.rules)。

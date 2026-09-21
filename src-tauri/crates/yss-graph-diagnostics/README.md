# Graph diagnostics

> Status: Current
> Scope: 图诊断代码、模板词汇、定义校验与前端模板生成
> Canonical owners: src/lib.rs 拥有定义，scripts/generate-graph-diagnostics.mjs 拥有生成逻辑
> Update when: 诊断代码、模板、参数或生成流程改变时

[src/lib.rs](src/lib.rs) 定义稳定的图诊断词汇与本地化模板；运行期诊断值由 `yss-graph-analysis-contract` 拥有。

## 修改与生成

修改 Rust 中的诊断词汇、模板键或参数后，从仓库根目录运行：

```sh
pnpm generate:diagnostics
pnpm generate:diagnostics:check
```

生成器是 [scripts/generate-graph-diagnostics.mjs](scripts/generate-graph-diagnostics.mjs)；生成表以 Rust 定义为来源，不维护另一套前端词汇。

按实际改动选择 `pnpm test:rs:package -p yss-graph-diagnostics --lib`；跨 wire 的消费方随契约变化一起检查，范围遵循[根规则](../../../.rules)。

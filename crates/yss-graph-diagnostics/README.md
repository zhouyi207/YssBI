# Graph diagnostics

> Status: Current
> Scope: 图诊断代码、模板词汇、定义校验与原生本地化读取
> Canonical owners: src/lib.rs 拥有定义，原生宿主拥有呈现与本地化回退
> Update when: 诊断代码、模板、参数或宿主读取契约改变时

[src/lib.rs](src/lib.rs) 定义稳定的图诊断词汇与本地化模板；运行期诊断值由 `yss-graph-analysis-contract` 拥有。

`define_graph_diagnostics!` 是唯一声明源，同时生成运行期 `GraphDiagnosticKind`、代码与定义查询，以及本地化模板表；不另维护枚举到代码的映射或无生产者的模板。前端仍为未知代码或缺失参数显示通用诊断文本。

GPUI 原生宿主直接消费 `GRAPH_DIAGNOSTIC_DEFINITIONS` 的本地化模板和参数声明，不复制诊断词汇。
模板替换及未知代码/缺失参数的显示回退属于宿主呈现，不改变诊断严重程度或 blocking 准入。

## 修改与验证

修改 Rust 诊断词汇、模板键或参数时，同步维护定义与类型化消费方。
GPUI 直接读取 Rust 定义，不需要生成前端模板或运行 JavaScript 工具。
诊断显示与人工验收见 [GPUI host](../yss-desktop-gpui/README.md)。

按实际改动选择 `cargo test -p yss-graph-diagnostics --lib`；契约变化时一并检查
Graph Analysis 与原生消费方，范围遵循[根规则](../../.rules)。

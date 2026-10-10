# Graph diagnostics

> Status: Current
> Scope: 图诊断代码、模板词汇、定义校验与原生本地化读取
> Canonical owners: src/lib.rs 拥有定义、校验与文本格式化，原生宿主拥有呈现及无效诊断的显示回退
> Update when: 诊断代码、模板、参数或宿主读取契约改变时

[src/lib.rs](src/lib.rs) 定义稳定的图诊断词汇与本地化模板；运行期诊断值由 `yss-graph-analysis-contract` 拥有。

`define_graph_diagnostics!` 是唯一声明源，同时生成运行期 `GraphDiagnosticKind`、代码与定义查询，以及本地化模板表；不另维护枚举到代码的映射或无生产者的模板。前端仍为未知代码或缺失参数显示通用诊断文本。

GPUI 原生宿主调用 `render_diagnostic`，不复制诊断词汇。该入口核对 code/message key 与必需参数，通过共享 `yss-i18n` / `rust-i18n` 按显式语言查询并执行 `%{name}` 单次插值；缺失翻译沿用原生宿主的 `zh-CN` 回退。
每个参数最多显示 512 个 Unicode 字符，控制字符变为空格，参数内的占位符保持字面量。未知代码、key 不匹配或缺少参数返回 `None`，由宿主显示安全的通用提示；不改变诊断严重程度或 blocking 准入。

## 修改与验证

修改 Rust 诊断词汇、模板键或参数时，同步维护定义与类型化消费方。
模板使用原生 `%{name}` 语法；定义校验拒绝不匹配的参数名及不完整占位符。GPUI 直接调用 Rust 格式化入口，不需要生成前端模板或运行 JavaScript 工具。
诊断显示与人工验收见 [GPUI host](../yss-desktop-gpui/README.md)。

按实际改动选择 `cargo test -p yss-graph-diagnostics --lib`；契约变化时一并检查
Graph Analysis 与原生消费方，范围遵循[根规则](../../.rules)。

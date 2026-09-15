# JSON 语义报告试点的取舍

> Status: Historical
> Scope: 将早期 JSON 驱动界面讨论收敛为 OLS 报告布局试点
> Canonical owners: 生产契约由 Graph 与 Execution 的 Results 章节维护
> Update when: 本轮实现与对比保留于实施记录；未来能力进入专项计划

## 已采用的范围

OLS 报告使用 `OlsReportSpec` 配置现有章节的顺序和显示状态。用户可以直接配置或导入完整 JSON；
Spec 引用当前报告的 `{ executionSessionId, resultId }`，所有统计值仍由 ResultStore 及既有查询产生。

```json
{
  "schemaVersion": 1,
  "type": "regressionReport",
  "source": {
    "executionSessionId": "00000000-0000-0000-0000-000000000001",
    "resultId": "42"
  },
  "sections": [
    { "id": "summary", "kind": "modelSummary", "visible": true },
    { "id": "coefficients", "kind": "coefficientTable", "visible": true },
    { "id": "residuals", "kind": "residualPlot", "visible": false }
  ]
}
```

示例引用仅说明格式；导入必须使用当前报告的真实引用。Spec 不接受 R²、AIC、系数、p 值或残差数组，
也不接受脚本、任意 IPC 名称或网络请求。Result payload 还需与 descriptor 身份匹配。
具体章节词汇、容量、错误和会话生命周期以[生产契约](../architecture/GRAPH_AND_EXECUTION.md#ols-语义报告布局)为准。

## 库选型证据

同一个由 Rust 实际计算的 OLS 样本，分别使用轻量类型化解释器和受限 json-render catalog 渲染。
两者复用相同章节组件，规范化 HTML 一致；catalog 仍需应用自己的结果身份、数量和结构约束校验。
本次平面报告没有显示出引入生产依赖的维护收益，因此保留轻量实现。

完整适配代码、实际版本、bundle 差值、校验与更新耗时、复现步骤和测量局限见
[实施记录](../reviews/2026-09-15-motion-json-driver-implementation.md)。
json-render 的 catalog 与 renderer 机制见[官方文档](https://json-render.dev/docs/catalog)。

## 必须分开的契约

| 数据                   | 用途与 owner                                          |
| ---------------------- | ----------------------------------------------------- |
| GraphDocumentPatch     | 类型化业务编辑与可逆历史，由 Graph / Project 拥有     |
| Graph projection delta | 后端到前端的只读同步，由现有投影安装器处理            |
| OlsReportSpec          | 展示章节配置；不持有图数据、结果数值或 Workbench 拓扑 |

Immer patch 的 path 数组、RFC 6902 的 JSON Pointer 和当前 Graph projection 的 set/remove 路径不是同一种 wire。
不能以“都使用 JSON”为由混用，也不引入统一状态总线。现有报告组件已覆盖所需展示，
本次没有额外引入 Immer、Zod、RJSF、JSON Forms、ECharts 或 Vega-Lite。

## 后续条件

若出现真实的流式生成、跨结果复用模板或报告持久化需求，再定义 schema 迁移、引用重绑定、
增量安装和资源预算。它们不是当前会话试点已支持的能力。
现有结果引用不能跨执行会话恢复；配置错误保留上一份有效报告；桌面人工验收仍按实施记录执行。

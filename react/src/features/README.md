# Features 参考实现

> Status: Historical
> Scope: 保留的 React 用例编排、读取投影与交互状态
> Canonical owners: 本文只说明参考源码；当前业务用例由 Rust Application 与领域 crates 拥有
> Update when: 参考行为或当前契约路由改变时

本目录不参与当前原生构建。当前入口见[根 README](../../../README.md)、[文档索引](../../../docs/README.md)和 [Application](../../../crates/yss-application/README.md)；原生呈现见 [GPUI host](../../../crates/yss-desktop-gpui/README.md)。以下是 React 参考代码的职责，不是原生架构或迁移完成清单。

## 参考分层

- `application/` 协调用户操作、异步请求、身份接纳和投影发布。
- `core/` 保存只读业务投影、资源特定的未提交输入和交互暂态。
- `domain/` 保留不依赖 React 或应用服务的纯规则；样式与尺寸留在界面模块。
- Application 可调用 Core、Domain 和 Services；Core/Domain 不反向引用 Application 或界面。

## 可供对照的行为约束

- `ResourceStore` 一次发布资源索引、文档标记、图会话、实体和结果摘要。投影不能成为第二份已提交文档或 Graph 历史。
- 读取通过 `readProjection` 的快照与订阅接口取得不可变值，按图、资源或实体选择所需字段。结构共享在安装边界完成，不在 render/selector 中深拷贝整图。
- 图、数据库和文档的迟到回复必须核对原项目、资源修订及请求生命周期；同步通知也可能使原操作失效。关闭、重命名或替换项目不能被旧回复撤销。
- 数据库声明和元数据与资源版本一起发布；分页属于读取视图。精确行 ID 保留字符串，真实 null、空字符串和文字 null 不混同；未知计数不显示为零。
- Chart 草稿、Mind/Doc 输入缓冲按各自 owner 保留。保存只结算捕获的编辑版本，不能覆盖更新的输入；缺失文件的未提交内容不能恢复索引成员身份。
- 图运行事实归 Execution 投影，Results 只协调查询、缓存和持有关系。当前输出绑定与已打开报告快照是不同生命周期。
- 连接决策、参数显隐、类型和诊断来自 Rust 投影；参考视图只展示候选、收集输入并提交意图，不复制语义分析规则。
- 画布手势、选择、视口和弹窗输入是有作用域的暂态。旧清理句柄不能清除后继手势、请求或进度；关闭界面不等于取消已提交业务。
- 日志不是业务事实来源。Problems、Results、运行失败和 Assistant 会话分别消费对应 owner 的投影。

实现细节继续留在对应源文件，不把这里的 Zustand、Immer、React Flow 或 FlexLayout 方案作为 Rust 实现要求。

## 参考模块与当前契约

| 行为 | React 参考 | 当前 Rust owner |
| --- | --- | --- |
| 图编辑与手势 | [Editor](application/editor/README.md)、[Data Store](core/dataStore/readme.md) | [Graph application](../../../crates/yss-application/src/graph/README.md) |
| 资源与未提交输入 | [Resource](application/resource/README.md) | [Project application](../../../crates/yss-application/src/project/README.md) |
| 结果查询与报告 | [Results](application/results/README.md) | [Graph application](../../../crates/yss-application/src/graph/README.md)、[Execution](../../../crates/yss-graph-execution/README.md) |
| 日志与反馈 | [Observability](application/observability/README.md) | [Logging](../../../crates/yss-logging/README.md) |

原生功能覆盖和未完成验收见[迁移计划](../../../docs/roadmap/GPUI_MIGRATION.md)，不能从参考实现存在推断完成状态。

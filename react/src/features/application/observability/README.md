# Observability and feedback 参考

> Status: Historical
> Scope: React 参考实现中的信号区分与用户反馈行为
> Canonical owners: 当前日志由 yss-logging 拥有，业务事实由对应 Rust 领域与 Application 拥有
> Update when: 参考行为或当前信号契约入口改变时

本目录不是当前日志装配、传输或宿主反馈入口。当前入口见[根 README](../../../../../README.md)、[文档索引](../../../../../docs/README.md)、[Logging](../../../../../crates/yss-logging/README.md)、[Application](../../../../../crates/yss-application/README.md)和 [GPUI host](../../../../../crates/yss-desktop-gpui/README.md)。

## 信号边界参考

| 信息 | 行为边界 | 参考视图 |
| --- | --- | --- |
| Graph Problems | 读取语义诊断投影，不能由日志重建 | [Problems](../../../modules/problems/README.md) |
| Results | 查询结果及其完整身份，通知不代替数据 authority | [Results](../results/README.md) |
| 运行失败 | 来自执行事实或类型化拒绝，独立于诊断与日志 | [Output](../../../modules/output/README.md) |
| 技术日志 | 有界、可丢失的技术观察，不驱动业务状态 | [Logs](../../../modules/logs/README.md) |
| Assistant 会话 | 使用会话事件与持久记录，不复制到日志或运行失败面板 | [Harness](../../../../../crates/yss-harness-core/README.md) |

## 反馈与安全参考

- 由类型化结果和安全错误代码决定反馈，不能从日志文本推断操作成功或失败。
- 可继续操作的页面使用持久区块错误；输入错误就近展示；破坏性操作先确认；成功优先以新状态表达。
- `incidentId` 只关联技术诊断。原始异常消息、cause、未知对象和自由文本不直接进入用户文案或结构化日志。
- 日志只保留排障所需的安全身份、计数与阶段，不记录完整数据、报告或 Assistant transcript。
- 生命周期清理只属于原安装或操作；迟到清理不能解除后继订阅，离开视图也不等于撤销已提交业务。

原 React logger 和反馈组件仅供源码对照。当前采集、存储、订阅、故障语义与原生反馈由上述 Rust owners 说明，不从这里的历史适配推导。

# yss-diagnostics

> Status: Current
> Scope: 显式运行诊断数据、输入校验、recent snapshot 和订阅交付
> Canonical owners: [src/](src/) 拥有诊断 API；跨信号边界由 [Runtime Signals](../../../docs/architecture/RUNTIME_SIGNALS.md) 维护
> Update when: 诊断输入、缓冲、订阅或生命周期改变时

本 crate 接收后端显式提交的诊断数据，并向订阅者交付快照和后续批次。它不依赖 `tracing`、Tauri 或日志插件，不从日志派生诊断，也不安装日志 subscriber。

Application 在初始化时创建 `DiagnosticsRuntime` 并通过 `app.manage()` 管理它。后端持有该 runtime 的引用即可提交诊断：

```rust
use yss_diagnostics::{
    DiagnosticDomain, DiagnosticEvent, DiagnosticLevel,
    DiagnosticSubmissionError, DiagnosticsRuntime,
};

fn publish_validation_result(diagnostics: &DiagnosticsRuntime) -> Result<(), DiagnosticSubmissionError> {
    diagnostics.publish(DiagnosticEvent {
        level: DiagnosticLevel::Warn,
        domain: DiagnosticDomain::Data,
        target: "dataset.validation".into(),
        event: Some("missingValues".into()),
        message: "Dataset contains missing values".into(),
        source: None,
        fields: std::collections::BTreeMap::from([
            ("missingCount".into(), serde_json::json!(3)),
        ]),
    })
}
```

`publish` 校验和清理输入，由 runtime 分配 origin、时间、streamId 和 sequence。订阅者通过 `subscribe_batches` 得到初始快照并接收后续批次，通过 `unsubscribe` 释放订阅。队列与缓存有界，丢弃情况通过诊断标记报告。

Tauri 适配保留在 `yss-application::ipc` 与 `yss-ipc-channel` 中，前端使用 [DiagnosticsService](../../../src/services/diagnostics/diagnosticsService.ts)。显式前端诊断提交的现有 wire 保持不变。日志继续由 `tauri-plugin-tracing` 管理；Graph Problems 和模型诊断保留各自的业务事实源。

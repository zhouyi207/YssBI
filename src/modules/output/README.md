# Output

> Status: Current
> Scope: 图运行失败摘要、节点定位与终态交付
> Canonical owners: Rust Execution 拥有运行事实；本模块展示失败摘要
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## 运行失败反馈

Execution channel 直接交付 `RunEventDto`，传递运行生命周期与结果通知；前端严格解析事件并以执行会话和运行身份隔离迟到消息。Analysis Graph 没有 Print/Effect，不提供 stdout/stderr 消息、缓存或订阅。

Output panel 展示当前图的运行失败摘要：从 RunErrored 投影原因、阶段和节点，可定位失败节点；运行开始前的 command rejection 使用安全错误代码回退。失败时 Application 打开 Output；清除错误或开始下一次运行清除本地摘要，不改变 Rust 的运行结果。日志、Assistant text 和 Graph Problems 各自保持原有职责与生命周期。

API 在成功交付 terminal event 后，用 command error details 的 `terminalRunEventSent: true` 标识拒绝路径；ProjectService 等待 channel 排空后才结束失败调用，确保原因投影先于错误收尾。incidentId 只用于关联技术诊断，前端不把 IpcError.message 作为用户文案。

Harness 事件及其恢复、插件进程通信由各自协议拥有，不通过 Graph 执行通道转发。

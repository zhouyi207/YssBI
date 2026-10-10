# Logs 参考视图

> Status: Historical
> Scope: React 日志 recent buffer、筛选与展示行为参考
> Canonical owners: 本模块只拥有参考呈现；当前日志记录、存储和订阅由 yss-logging 拥有
> Update when: 参考行为或当前日志契约入口改变时

本目录不参与原生构建。当前入口见[根 README](../../../../README.md)、[文档索引](../../../../docs/README.md)、[Logging](../../../../crates/yss-logging/README.md)和 [GPUI host](../../../../crates/yss-desktop-gpui/README.md)。

## Logs UI

- 日志视图只展示技术观察，不接收或重建 Graph Problems、运行失败、Results 或 Assistant 会话。
- 参考 recent buffer 有界保存记录、stream、watermark 和截断状态，一批有效数据只发布一次；重复批次不通知。
- 缺口、流替换与存储故障必须可见，不能把不完整数据当成完整历史。刷新重新读取快照；清空视图不删除持久记录。
- 领域索引只引用原有有界集合，未变化领域保留引用。级别筛选、搜索、选择和自动滚动属于 UI 暂态，不成为记录 authority。
- 工具栏与列表按需订阅，日志追加不广播整个工作台。Details 查看选中的日志，不改变业务编辑目标。
- 用户日历时间保留原日期与钟面，不通过显示时区移动它们；耗时与序列顺序不由显示文本推断。

参考日志路由见 [Observability](../../features/application/observability/README.md)。原生采集、查询和故障语义必须查阅当前 Rust owner；本文件不保留历史宿主传输或窗口契约，也不声明人工验收完成。

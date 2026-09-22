# Logs

> Status: Current
> Scope: 日志订阅、recent buffer、gap 恢复与展示
> Canonical owners: 日志插件拥有记录，本模块与 Application log 拥有前端投影
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## Logs UI

[LogService](../../services/log/logService.ts) 只调用日志插件。[subscription](../../features/application/log/useLogSubscription.ts)、[buffer](../../features/application/log/logBuffer.ts) 和 `src/modules/logs/` 只投影 `Log*Dto`，不消费 `Diagnostic*Dto` 或 Graph Problems。

实时 receiver 发现 gap、stream replacement、malformed batch 或存储失败后停止推进 watermark，释放旧 Channel 并进行有界重订阅。丢弃和截断可检测；“刷新”重新读取 recent snapshot，“清空”只修改当前前端 buffer，不删除 SQLite 或后端 ring。历史分页与统计接口由日志插件拥有，当前前端 `LogService` 仅封装记录提交和 recent/live 订阅。

项目与日志新产生的日历时间使用本机钟面时间，序列化为不带时区的 `YYYY-MM-DDTHH:mm:ss.SSS`；不附加 `Z`、UTC 名称或偏移。已有项目元数据的带偏移时间在解析时保留原日期和钟面并移除偏移。Unix 秒/毫秒和单调时钟仍用于内部时间点、排序或耗时，不添加时区文本。图表日期/日期时间表示无时区日历字段，显示时不得通过浏览器时区移动它们。

# Logs

> Status: Current
> Scope: 日志订阅、recent buffer、gap 恢复与展示
> Canonical owners: 日志插件拥有记录，本模块与 Application log 拥有前端投影
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## Logs UI

[LogService](../../services/log/logService.ts) 只调用日志插件。[subscription](../../features/application/log/useLogSubscription.ts)、[buffer](../../features/application/log/logBuffer.ts) 和 `src/modules/logs/` 只投影 `Log*Dto`，不消费 `Diagnostic*Dto` 或 Graph Problems。

实时 receiver 发现 gap、stream replacement、malformed batch、订阅积压（`subscriber_lagged`）或存储失败（`storage_unavailable`）后停止推进 watermark，释放旧 Channel 并进行有界重订阅。两种终止通知只允许空记录批次，由既有 parser 校验，接收器将原因交给原恢复流程。丢弃和截断可检测；“刷新”重新读取 recent snapshot，“清空”只修改当前前端 buffer，不删除 SQLite 或后端 ring。历史分页与统计接口由日志插件拥有，当前前端 `LogService` 仅封装记录提交和 recent/live 订阅。

Recent buffer 由其内部 Zustand store 唯一持有 stream、entries、watermark 和截断标记。
一个有效批次只发布一次，直接发布新生成的有界数组，不再维护平行局部快照、监听器表或重复复制数组。
重复批次不通知，清空保留序列水位，前端仍不拥有后端日志记录。
重订阅按同一 stream 的已提交 sequence 复用仍在缓冲内的记录，未变化领域和同值完整快照
保持引用。该身份依据日志存储的只追加契约；stream replacement 不复用旧记录，清空后刷新
仍重新恢复后端历史，恢复的 gap 和截断状态继续独立接纳。

缓冲在同一次发布中维护只读领域索引，`all` 直接引用原 entries，其他领域只持有该有界集合内的记录引用。
追加批次只按新增记录和被裁剪前缀更新受影响领域，快照替换与清空同步更新索引；未变化领域复用原数组。
领域视图通过已有 Zustand 选择订阅桥接读取，
按实际需要订阅列表、计数、初次加载或截断状态。筛选结果由原 `logStore` 模块按不可变记录数组、筛选对象
和领域复用，列表与标题计数共享结果；弱引用缓存不保留已退出缓冲且无人持有的旧输入。
筛选级别、搜索、选中记录和自动滚动仍归原 UI store，同值写入不发布。

Application 的 [workspace controller](../../features/application/log/useLogWorkspaceController.ts) 拥有订阅挂载、
刷新、清空与日志选择到 Details 的联动，操作时读取当前自动滚动设置和查看上下文。Logs Context 仅传递稳定操作、
低频订阅状态和显式刷新 token，不广播记录、筛选或选择。工具栏不订阅日志批次，计数更新限制在状态组件；
主窗口与独立窗口沿用同一虚拟列表和原生视口滚动实现，不传递没有行为差异的呈现参数。

Application 公共入口只暴露所需的读取 hook、UI store、workspace controller 和领域识别；
缓冲写入、订阅恢复及内部筛选函数留在 Application 内部。Workbench 继续拥有 Logs 标签布局。

项目与日志新产生的日历时间使用本机钟面时间，序列化为不带时区的 `YYYY-MM-DDTHH:mm:ss.SSS`；不附加 `Z`、UTC 名称或偏移。已有项目元数据的带偏移时间在解析时保留原日期和钟面并移除偏移。Unix 秒/毫秒和单调时钟仍用于内部时间点、排序或耗时，不添加时区文本。图表日期/日期时间表示无时区日历字段，显示时不得通过浏览器时区移动它们。

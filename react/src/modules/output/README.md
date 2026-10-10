# Output 参考视图

> Status: Historical
> Scope: React 图运行失败摘要与节点定位的行为参考
> Canonical owners: 本模块只拥有参考呈现；当前运行事实由 Rust Execution/Application 拥有
> Update when: 参考反馈行为或当前执行契约入口改变时

此处不是当前原生运行事件或宿主反馈接口。入口见[根 README](../../../../README.md)、[文档索引](../../../../docs/README.md)、[Graph application](../../../../crates/yss-application/src/graph/README.md)、[Execution](../../../../crates/yss-graph-execution/README.md)和 [GPUI host](../../../../crates/yss-desktop-gpui/README.md)。

## 运行失败反馈

- Output 参考面板从类型化运行失败事实展示原因、阶段、节点及可用的 incidentId；运行开始前的拒绝使用安全错误代码反馈。
- 失败摘要不从日志重建。Graph Problems、Results、Assistant 文本和技术日志各自独立；Analysis Graph 不把该面板当作用户程序 stdout/stderr。
- 当前图来自所属编辑器选择；切到非 Graph 标签不回退到旧会话。运行身份、语义身份和执行会话共同约束可见失败。
- 分组失败保留外层到内层来源及函数路径，节点定位使用实际失败位置；界面不推测组或重放数据。
- 清除摘要或开始下一次运行只影响对应展示，不修改已有 Result 数据。旧终态不能覆盖后继运行或后继项目。
- incidentId 用于关联排障，原始异常文字不是用户文案；本地化由呈现层处理。

原生终态交付与清理顺序由当前 Rust owners 说明，不沿用参考适配细节。人工验收状态见[图与结果验收待办](../../../../TODO.md#graph-and-results-acceptance)。

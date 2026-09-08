# yss-application

`yss-application` 是跨 Project、Execution、Database、Graph 与 SCI authority 的用例编排层。它拥有应用会话替换、工作流顺序、失败分类和提交后事件事实，但不拥有 Tauri command、IPC DTO 或事件发送。`plugins` 实现通用 HostServices：项目绑定的数据列表、Arrow 快照 lease、来源记录和 Core 结果提交；Julia/Bayes 编排属于外部插件的 `yss-bayes-runtime`，不进入宿主依赖图。

边界约束：

- 只依赖领域与运行期 crate，不依赖根包、Tauri、Commands 或 Schema；
- `ApplicationState` 是运行期 authority 组合后的应用会话入口，不是全局 backend 容器；
- 对外返回 Application-owned typed facts，由 `yss-api` transport adapter 投影为 wire DTO；
- `test-support` 只开放跨 crate contract 测试所需的构造与 publication seam。

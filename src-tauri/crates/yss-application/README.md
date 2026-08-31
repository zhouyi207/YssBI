# yss-application

`yss-application` 是跨 Project、Execution、Database、Graph、SCI 与 Bayes authority 的用例编排层。它拥有应用会话替换、工作流顺序、失败分类和提交后事件事实，但不拥有 Tauri command、IPC DTO 或事件发送。

边界约束：

- 只依赖领域与运行期 crate，不依赖根包、Tauri、Commands 或 Schema；
- `ApplicationState` 是运行期 authority 组合后的应用会话入口，不是全局 backend 容器；
- 对外返回 Application-owned typed facts，供 transport adapter 投影为 wire DTO；该 adapter 后续收口为 `yss-api`；
- `test-support` 只开放跨 crate contract 测试所需的构造与 publication seam。

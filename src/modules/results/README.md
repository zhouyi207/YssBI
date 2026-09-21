# Results views

> Status: Current
> Scope: Result 面板、报告组件和语义页面呈现
> Canonical owners: 本模块拥有呈现；查询与租约由 Application results 维护，数值由 Rust ResultStore 拥有
> Update when: 结果视图、报告渲染或页面布局接入改变时

[public.ts](public.ts) 提供结果面板入口，报告组件位于 [internal/ui/info](internal/ui/info/)。
读取、分页、分析和结果生命周期见 [Results application](../../features/application/results/README.md)；
组件目录、受控动作与增量协议见 [UI contract](../../../src-tauri/crates/yss-ui-contract/README.md)。

## 线性回归语义报告布局

线性回归报告接入 Rust 拥有的 JSON 页面。GUI 的排序、显隐、重置、导入和 Harness 修改共用 Application，前端通过快照与稳定元素增量呈现容器、文本、报告章节及受控按钮。
页面结构、组件目录、修订校验、回执和会话恢复见 [JSON 页面与界面意图](../../../src-tauri/crates/yss-ui-contract/README.md)；不再维护独立的前端报告布局权威或旧 Spec 转换。

报告章节继续复用模型概览、系数、残差图、分页观测表和检验组件。Results 与查询协调器拥有统计值和能力校验，数值不来自 Spec；观测表和残差图按需加载，检验由用户提交参数触发。
布局编辑文本保留在前端，完整有效配置经后端提交后才安装。页面绑定当前结果引用，不创建新的数据所有者或结果租约。
同一 Application session 内重开恢复页面；项目或执行会话结束后不跨结果重绑定。隐藏章节不释放真实面板的结果租约，保留快照的失效与回收仍遵循 Results 契约。
模板落盘、更多页面类型和人工验收仍由 [JSON Driver 计划](../../../docs/roadmap/jsonDriver.md) 跟踪。

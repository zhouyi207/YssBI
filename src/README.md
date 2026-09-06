# 前端入口

`app/` 组合窗口与路由，`modules/` 提供界面，`features/application/` 编排用例，`features/core/` 保存投影与交互状态，`features/domain/` 保存纯领域规则，`services/` 适配 IPC。

Rust 拥有已提交项目状态。查询返回数据；修改返回提交结果，事件可交付同一提交的回声，前端由发布协调器统一去重和更新投影。未保存的 Graph draft 与已提交状态分开管理。

依赖方向和状态归属见[当前架构](../docs/architecture/ARCHITECTURE.md)，验证命令见[本地工作流](../docs/development/LOCAL_WORKFLOW.md)。

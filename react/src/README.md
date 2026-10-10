# React 参考源码

> Status: Historical
> Scope: `react/src/` 保留的 React 实现与交互行为参考，不参与原生构建
> Canonical owners: 本目录 README 只说明参考源码；当前架构、构建与模块契约由根文档和 Rust crates 拥有
> Update when: 参考源码范围、行为说明或当前契约入口改变时

本目录保留原 React 实现，供理解交互、比较行为和查阅历史设计；它不是当前前端、开发服务器或打包入口，也不表示这些行为已经移植或验收。

当前项目是 GPUI 前端的纯 Rust workspace。启动与验证入口见[根 README](../../README.md)，模块路由见[文档索引](../../docs/README.md)，原生界面契约见 [GPUI host](../../crates/yss-desktop-gpui/README.md)。业务用例见 [Application](../../crates/yss-application/README.md)；领域状态不由这里的 React stores 拥有。

## 参考目录

- `app/`：原 React 组合入口。
- `modules/`：画布、文档、结果、工作台等界面实现。
- [features/](features/README.md)：原用例协调、读取投影和交互状态。
- `services/`：保留的历史适配代码，不是原生宿主调用契约。
- [局部规则](.rules)：只约束本参考目录，不适用于 GPUI/Rust 实现。

本目录不提供现行安装、启动、构建或验证命令。旧脚本、平台适配和浏览器实现不构成原生能力的证据；开放项与人工验收以[原生工作台待办](../../TODO.md#native-workbench)为准。

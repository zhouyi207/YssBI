# JSON Driver：语义组件驱动页面计划

> Status: Planned
> Scope: 语义组件目录、JSON 页面、数据绑定、动作、AI 修改与剩余验收
> Canonical owners: 本文维护进度；当前页面协议由 Presentation 架构与源码维护
> Update when: 页面范围、组件、持久化、AI 接入或验收状态改变时

首个完整接入场景是线性回归结果报告：GUI 和 Harness 共用 Rust Application 的页面状态与动作，后端提交 JSON 并生成稳定元素增量，React 安装投影。当前契约见 [JSON 页面与界面意图](../../src-tauri/crates/yss-ui-contract/README.md)，统计数据仍归 [Results](../../src/features/application/results/README.md#results)。

## 已接入

- [x] 封闭组件目录：column、row、text、reportSection、button；稳定元素 ID、受限 props/children、单根树和完整结构校验。
- [x] Rust 生成默认页面与组件 JSON Schema；Renderer 组合目录内的已有报告组件，支持嵌套布局和当前结果绑定。
- [x] GUI 的排序、显隐、重置及 JSON 导入调用后端动作；前端不再持有独立的已提交报告布局。
- [x] 页面修改使用修订比较和原子提交；后端生成 set/remove/root 元素差分，通过命令回复与 Channel 交付。
- [x] 订阅先于快照；重复回复忽略，缺口和非法增量只读恢复，失败保留有效页面；窗口卸载和会话替换隔离迟到回复。
- [x] Harness 接入 inspect_ui / update_ui，可读取目录、生成完整页面并提交有效的增量批次；按钮仅调用登记的界面意图。
- [x] 同一结果在当前 Application session 内关闭重开保留页面；页面本身不取得结果租约，不写入 Project 或工作台布局。
- [x] 未引入 json-render、Zod、Immer 或 JSON Patch npm 包；复用现有 Results、错误与生命周期入口，选型理由见当前契约。

这些勾选表示源码接入和对应自动检查的范围，不表示桌面人工验收或整个页面产品已完成。当前元素差分是专用协议，不是完整 RFC 6902。

## 剩余工作

- [ ] 人工验收嵌套布局、受控按钮、GUI/Harness 交错修改、重复消息、错误配置保留页面、关闭重开、独立报告窗口和项目/结果会话切换。
- [ ] 人工验收连续有效生成批次的呈现；测量代表性页面的请求量、传输量、安装与绘制成本。未完成的原始 JSON 文本不直接安装。
- [ ] 按第二个实际页面场景扩展通用结果表、图表、Markdown、表单和数据引用；每项继续使用现有数据/业务 owner，不能把当前报告组件当作这些能力已完成。
- [ ] 按产品需求定义模板落盘、跨结果复用和引用重绑定；当前只保留会话内配置，不支持应用重启恢复或跨结果模板。
- [ ] 在有需求时增加生成进度、取消未提交批次和大页面分段安装；继续保留页面原子校验，不增加旧格式迁移。

[返回专项计划](README.md) · [React / Harness 共用入口](motion.md)

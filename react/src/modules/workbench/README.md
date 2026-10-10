# Workbench 参考实现

> Status: Historical
> Scope: 保留的 React/FlexLayout 工作台及交互行为参考
> Canonical owners: 本模块只拥有参考布局实现；当前原生工作台由 yss-desktop-gpui 拥有
> Update when: 参考行为或当前工作台契约入口改变时

此处不是当前项目架构、窗口平台或构建入口。参见[根 README](../../../../README.md)、[文档索引](../../../../docs/README.md)和 [GPUI host](../../../../crates/yss-desktop-gpui/README.md)。当前原生布局由根 DockArea 拥有；不能把下面的 FlexLayout 结构、浏览器状态或缓存格式作为原生契约。

## 布局与状态参考

- 参考实现以 FlexLayout Model 作为拓扑、顺序、选择、尺寸和折叠的唯一可写来源；React 只呈现它，不另存第二份布局模型。
- 面板身份、资源身份和业务会话是不同概念。移动、隐藏、分屏与重新挂载不等于关闭资源，也不转移 Graph 文档或结果的业务所有权。
- 活动资源由布局选择决定，不能用 Details 查看对象或输入焦点替代。画布动作绑定自己的可见面板，菜单动作绑定捕获的活动目标。
- Reveal 已有面板保留它的位置；布局 reset 不改变仍保留的资源与结果身份。项目替换使旧排队操作和恢复任务失效。
- 关闭统一经过协调入口；保存或放弃前等待资源编辑队列，支持取消。未完成输入与结果持有关系由业务 owner 结算，布局不直接修改它们。
- Assistant 列表与会话内容分离；关闭面板不删除持久会话，也不默认取消后台执行。当前会话契约见 [Harness](../../../../crates/yss-harness-core/README.md)。

## Activity 文档与模板

以下仅记录参考目录中 `ActivityPanelDocumentView`、`useActivityPanelDocument` 与 `sidebarStore` 的配合：

- 列表内容来自接纳后的目录投影，模板接收文档、展开状态和有限操作回调，不自行拼装业务目录。
- Project、Nodes、Commands、Plugins、Assistant 复用标题栏、分类行、空状态和滚动容器。业务组件只提供条目渲染与操作。
- 搜索、展开、重命名输入是局部交互；修改提交后由所属发布或失效入口刷新，不能从操作回执拼接第二份权威目录。
- 目录读取核对项目、语言和生命周期，过期响应不覆盖新目录；Assistant 列表不为展示条目读取完整消息或隐式创建会话。
- 资源单击与 Open 动作走同一打开入口；选择显示依赖资源身份，不因拖拽描述尚未加载而禁用普通打开。

局部编辑约束见 [.rules](.rules)。原生侧栏、面板注册和界面意图以 [GPUI host](../../../../crates/yss-desktop-gpui/README.md)及 [UI contract](../../../../crates/yss-ui-contract/README.md)为准。

## 交互对照与开放验收

[画布](../graph-editor/README.md)、[文档](../document-editor/README.md)、[资源操作](../../features/application/resource/README.md)和 [Results](../../features/application/results/README.md)保留对应行为参考。

拖动、分屏、停靠、折叠、主题、面板状态保留、关闭取消、结果持有与项目切换需要在原生工作台实际验收。参考实现、历史浏览器测试或接口存在不能替代这些验收；完成状态见[原生工作台待办](../../../../TODO.md#native-workbench)。

# Editor Application 参考

> Status: Historical
> Scope: 保留的 React 编辑器用例、画布手势和 Details 行为
> Canonical owners: 本目录只拥有参考实现；当前图用例由 Rust Graph application 拥有
> Update when: 参考交互约束或原生契约入口改变时

本目录不是当前应用层。参见[参考入口](../../../README.md)、[根 README](../../../../../README.md)、[文档索引](../../../../../docs/README.md)、[Graph application](../../../../../crates/yss-application/src/graph/README.md)和 [GPUI host](../../../../../crates/yss-desktop-gpui/README.md)。

## 保留的行为参考

- `useEditorCanvas` 为物理面板提供 commands/workspace/interaction；菜单捕获活动编辑器，画布动作捕获自身可见面板。两者提交前均重验项目、面板、分组与资源身份。
- 加载属于可见面板与显式读取用例。仅切换焦点不触发加载、卸载或重试；同一目标共享在途读取，迟到响应不恢复已关闭资源。
- React Flow 只拥有测量、变换与手势暂态。节点和连线来自只读图投影，移动结束提交一次修改，不引入独立文档或撤销历史。
- Escape、隐藏、保存锁、关闭图与替换项目使手势失效。取消恢复原选择和坐标，旧修改的完成不能清除更新的拖动预览。
- 可见分屏独立交互；侧栏焦点不使另一可见画布变成只读。菜单、输入字段与模态交互保留自己的快捷键作用域。
- 创建前配置保留局部表单，取消不创建节点；预览完成后才提交参数和端口数量。提交与关闭目录都绑定原目录实例。
- 参数输入通过同一图修改队列提交；选择、开关和排序即时提交，文本在完成编辑时提交，Escape 恢复投影。未完成输入留在局部，失败不替换已发布图，不隐式保存。
- Event/Function/Mind 的 Details 随所属面板选择显示单节点或文件；Doc/Chart/Data 显示资源信息。加载失败或资源缺失保留资源身份，不混同无选择。
- Problems 定位在每次异步等待后重验原面板；晚到的定位不能重新激活用户已离开的标签。
- 保存、全部保存与关闭先等待资源编辑队列，再收集 dirty 状态。Save/Discard/Cancel 与输入缓冲的版本检查共用资源操作入口。
- 运行事件按执行会话、语义身份和逐输出归属接纳，Results 查询与租约独立于运行事实。

## 参考模块

[画布](../../../modules/graph-editor/README.md) · [资源操作](../resource/README.md) · [文档编辑器](../../../modules/document-editor/README.md) · [结果](../results/README.md)

这里描述的交互不能证明原生实现已完成。参数编辑、取消、撤销重做、分屏、保存关闭和项目替换仍须在原生实现中按[迁移计划](../../../../../docs/roadmap/GPUI_MIGRATION.md)验收。

# Graph editor

> Status: Current
> Scope: React Flow 画布呈现、输入手势与编辑器投影消费
> Canonical owners: 本 module 的画布组件；应用操作和共享 viewport 由 features/application/editor 与 Core 提供
> Update when: 本模块的公开入口、状态归属、生命周期或契约改变时

## Canvas presentation and input

节点画布使用 React Flow，每个 FlexLayout panel/group/resource 挂载独立 provider。
节点、连线及稳定 handle ID 单向派生自现有 editor projection；React Flow 的测量、
临时坐标和连接手势只是 UI 状态，不参与 Graph document 序列化，不创建第二套草稿或历史。
节点标题、端口、参数输入、执行状态和诊断仍由项目自己的 React 组件展示。
受控节点保留 React Flow 返回的 measured 尺寸和 dragging 标记；未变化节点复用对象引用，
避免拖动或选择时丢失 handle 测量并反复隐藏节点。测量回调在只读预览面板中也被接收，
零尺寸观察不覆盖已知有效尺寸；这些数据只属于渲染器，不提交到草稿。

拖动中的位置覆盖只保存在该画布，结束时通过现有 Application 命令提交一次 MoveNodes。
未完成 mutation 保留自己的预览；失败后恢复投影，迟到回调不能清除较新手势的预览。
Escape、面板隐藏、保存锁定、关闭图和替换项目都撤销手势；回调在提交前复核物理面板与项目身份。
取消旧面板的选择预览不能改变新面板的 Details 上下文。

普通单击画布节点会更新节点详情并切换、展开右侧 Details 标签；再次点击已选中的节点也会执行该操作。修饰键多选和程序化选择仅同步原有上下文，不触发详情标签切换。

可见分屏画布按自身面板和图路径接收操作；侧栏输入焦点不改变画布交互模式。运行工具栏持续显示，保存时禁用运行，各按钮操作其所属图。

鼠标平移只接受当前有效手势的 viewport 更新；取消后忽略剩余移动，松键时也恢复画布库内部
变换，防止下一次平移跳动。受控视口的程序化同步不创建或结束鼠标手势。
框选在按下时捕获完整的节点/连线选择，Escape 立即关闭选框并恢复原选择，后续松键不重新提交
框选或触发空白点击。Shift 轻点空白保留选择，普通空白点击清空；框选与节点/连线拖动均不
隐式自动平移。端口命中区域始终隔离节点拖动和平移，包括禁用和 orphan 端口。

普通连接、Ctrl 迁移端口连接、Alt 断开、空白处创建并连接节点和双击连线插入 Reroute，
继续走已有草稿命令与 Rust Resolve。React Flow loose handles 只表达连接锚点，方向和兼容性
来自投影，受损连接仍可显示以供修复；动态端口重排即使未改变节点尺寸，也重新测量 handle。
删除、复制粘贴和撤销重做不使用画布库自建的文档修改或历史。

受控 viewport 复用既有 group/graph viewport session，缩放、导航定位、侧栏拖入和视图偏好
共享同一坐标系。当前保持全部节点挂载，供 F/Home 与 Problems 定位读取完整节点边界；
不默认启用可见区域裁剪，也不随迁移引入自动布局引擎。

工作台空白区域的 `WatermarkView` 居中显示 YssBI 和本地化描述，以主题强调色、静态柔光和细分隔线呈现品牌层次；不接收操作命令，也不包含按钮或动画。

## 相关模块

[编辑器应用层](../../features/application/editor/README.md) · [工作台布局](../workbench/README.md)

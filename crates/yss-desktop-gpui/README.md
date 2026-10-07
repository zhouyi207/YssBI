# GPUI desktop host

> Status: Current
> Scope: GPUI 原生桌面入口、工作台、图编辑与执行结果交互
> Canonical owners: Cargo.toml、src/main.rs、src/services.rs、src/workbench/、src/canvas/
> Update when: 原生宿主能力、状态所有权、启动方式或验收范围变化时

这是 `gpui` 分支的原生桌面迁移入口，使用 GPUI 0.2.2 和 GPUI Component 0.5.1。
窗口及画布由 GPU 原生绘制，不启用组件库的 WebView feature。

```sh
pnpm dev:gpui
pnpm dev:gpui /absolute/path/to/project
pnpm dev:gpui /absolute/path/to/project events/example.yssbi-event
pnpm build:gpui
```

未提供目录时显示项目打开入口；未指定图时优先打开首个 Event 图，否则打开首个 Function 图。
项目和图由现有 Application 用例打开，启动读取发生在窗口事件循环之前，后续阻塞操作在
Tokio blocking pool 执行，指针事件不读取磁盘。

## 模块、状态与数据流

- `main`：命令行参数、执行器、原生资源、窗口及快捷键；执行器存活到窗口循环结束。
- `appearance`：统一原生配色、字体尺寸、圆角与空状态，不持有业务或布局状态。
- `assets`：自有 SVG 与现有组件图标资源组合；原生视图直接使用矢量图标。
- `services`：接入 ApplicationServices，提供任务执行、Graph/Project/UI/Harness 事件交付及 FS sink。
- `project`：持有后端的项目索引、Activity 文档和图编辑只读投影，不拥有另一份图文档。
- `workbench`：组合 Project/Nodes、编辑标签、Details、Problems、Output、Results 和 Logs。根 DockArea 是布局拓扑的唯一 owner；
  图实体的弱引用表只用于按资源查找，不保存面板位置、顺序或选中状态。
- `workbench/lifecycle`：项目切换、显式逐图保存和窗口关闭；保存失败保留窗口，已成功的保存分别接收回执。
- `workbench/chrome`：原生标题栏、项目名称、窗口控件与状态栏；客户端窗口关闭和系统关闭
  都进入 lifecycle 的同一判断与保存确认，不能因自绘标题栏绕过未保存保护。
- `workbench/resources`：通过现有 Application 新建 Event/Function 图，从提交回执取得实际路径后打开；
  资源目录仍通过发布与失效查询更新，不从操作回执拼接 Activity 条目。
- `workbench/graph_properties`：未选择节点时呈现函数签名和图常量，复用原始函数文档与图编辑事务。
  元数据在 worker 中读取，按图实体、编辑版本与查询代次安装；常量完整值按需读取，内容指纹只用于
  保留未变化控件的输入，不代替提交前的版本校验。读取失败时禁用旧表单，刷新后重新读取。
- `workbench/details`：协调当前图的选择、版本与属性表单；参数分组、条件字段及选项来自原编辑器投影。
  `parameters` 负责文本、数值、开关、选项、常量引用及分页数列；`relational` 负责有序列选择和类型化筛选，
  `domain` 负责编码/标签/正值映射，`ports` 负责标量字面量和端口实例操作。未变化字段保留暂态输入，
  字段事实变化时重新安装对应控件；投影或选择变化使旧菜单失效。常量引用选择与图常量表单共用
  `graph_properties` 的匹配版本查询，迟到查询不能安装到另一图或项目。
- `workbench/events`：会话订阅、索引失效恢复和原生投影更新。项目身份与本地 lifecycle 拒绝旧回调，
  索引恢复合并并发需求，broadcast 缺口通过权威查询恢复。
- `workbench/intents`：复用 WorkbenchBinding，先认领请求，再打开图/定位节点或显示已实现面板，
  成功后结算 applied；未实现的目标结算 failed。
- `canvas/geometry` 与 `canvas/render`：端口锚点、节点、连线、视口、可见节点裁剪和 GPU 路径。
  节点行固定保留左右两列，input 在左、output 在右；缺少一侧端口时另一侧仍贴对应边缘。
  节点尺寸与连线锚点共用 geometry，标签过长只截断显示，不改变端口身份或连接规则。
- `canvas/toolbar`：画布之外的固定操作栏，常用动作使用带提示的图标，局部运行和刷新放入更多菜单。
  指针交互 bounds 只覆盖画布，工具栏不会遮挡节点或触发选择/平移；菜单动作绑定对应画布焦点。
- `canvas/catalog`：从现有 Application 查询连接判定与兼容节点目录，按捕获版本和手势拒绝迟到回复。
- `canvas/commands`：提交现有 Application 图用例，接收完整投影、历史能力和 dirty 状态；
  编辑期间仅保存暂态位置，松键才提交，预览保持到回执返回。
- `canvas/authoring`：签名提交保留原始 before-state 与版本，常量输入在 worker 中解析后进入原编辑事务。
  `canvas/constant_drag` 只持有拖动预览和捕获的图身份；画布接收时拒绝跨图拖动及过期版本。
- `canvas/execution`：捕获与当前编辑投影匹配的后端文档，提交原 RunGraphRequest；公共图活动拥有运行事实，
  命令拒绝仅为本地反馈。原生运行读投影按执行会话与 RunId 安装，恢复期间有界暂存通知后重放。
  单节点可选择当前输入或补算依赖，取消复用完整运行身份；退出时请求取消仍在运行的计算。
- `workbench/output`：只展示当前图语义与执行会话匹配的失败原因、阶段和分组上下文；定位使用原 PlanSourceIdentity。
  清除只影响显示，切换到结果标签清除图上下文，不从日志恢复失败事实。
- `workbench/results`：从当前图的 GraphResultState 和运行投影派生结果目录，区分上次过期结果与等待同步的输出；
  按完整结果引用打开原生结果标签，并支持现有 OpenResult UI intent。
- `results/query`：在 blocking worker 内申请并持有原 Application 租约、读取投影或每页 100 行数据。
  失败、取消和实际关闭释放租约；面板拖动重挂载保留同一租约。普通重跑不替换已打开的结果。
- `results/table` 与 `results/value`：虚拟表格、概览与按需展开字段；结构化报告的数组只展示后端数据引用，
  点击后按原 ResultTablePart 读取有界页。标量保留空值、布尔值与宽整数，前端不计算统计量。
- `workbench/logs`：直接订阅中立 LogRuntime 的已提交记录，recent buffer 有界、虚拟列表绘制；
  清空只清视图，缺口和慢消费恢复 recent snapshot，存储失败显示不可用。关闭释放原生订阅。
- `services/paths`：保持应用标识和各平台数据/日志目录约定；`YSSBI_APP_DATA_DIR` 可指定绝对路径，
  用于隔离迁移验收的项目注册、Harness、模型设置、插件状态和日志；示例优先使用可执行文件旁的 resources/samples。
- `text`：原生 UI 本地化；assets 中的中文资源取自现有桌面文案，图诊断直接读取 Rust 的原始模板定义，
  校验 code/message key 及所需参数后一次替换。未知模板或缺失参数显示通用提示，不展示原始模板键；
  Problems 按后端 severity 呈现图标和颜色，blocking 仍由后端独立决定。

依赖方向为原生宿主 → Application/共享投影与协议。Application/Project/Graph 不依赖 GPUI。
Project 是 GraphDocument、历史和保存身份的唯一 authority；没有原生图草稿、UI 历史或格式转换。
Activity 条目直接消费 Rust ActivityPanelDocument；搜索与分类展开留在视图，不从磁盘重建目录。
Details 参数输入是未提交暂态；应用参数时通过匹配版本的后端文档保留其他参数，再提交现有编辑事务。

## 当前交互

- 右键/中键平移、鼠标位置锚定缩放、重置视图；右键菜单只在未移动的右键释放后打开。
- 左键选择、Shift/Ctrl 多选、框选、节点及多节点拖动、Escape 取消手势；拖动不隐式保存。
- 从 Nodes 目录或画布菜单创建节点；端口拖动连接，Ctrl 拖动迁移连接，Alt 点击断开端口。
  创建并连接由后端作为一次事务校验并提交；端口反馈显示 append/replace/invalid，拉出菜单仅查询兼容节点。
- 删除普通节点、标签编辑、参数输入、Undo/Redo、单图保存和全部保存。
  参数支持恢复默认值、布尔开关、选项、图常量选择、分页数列编辑与排序、类别编码映射；
  列名保留原字符串及选择顺序，未知 Schema 可逐项填写，已消失的列继续显示以供修复。
  筛选条件的列、比较方式和值类型消费后端配置，空值比较不提交多余 literal。
- Details 支持 Numeric/Binary/Text 输入端口的字面量与清除覆盖、端口断开、增删和重排。
  连接生效时保留字面量覆盖并禁用输入；端口身份、联动成员及连接清理由原编辑事务生成，
  不由视图分配。整数输入直接保留 Rust 支持的整数范围，溢出不会回退成舍入浮点数。
- 标题栏新建 Event/Function 图；未选择节点时在 Details 编辑函数参数名称、类型、顺序与返回类型。
  函数参数身份保持不变，签名事务沿用后端的消费者失效与持久化，不隐式保存当前图正文。
- 图常量支持新建、改名、类型和值编辑、删除、中心插入引用及拖动到当前画布。
  标量直接编辑；数组与对象使用原 DataValue 类型化 JSON，整数和小数保留精确表示；
  DataFrame/DataSeries 使用有序列名到值数组的 JSON。未修改值时保留原值与表格 sidecar，
  名称冲突、类型不匹配和引用诊断由原图事务校验。常量修改可撤销，显式保存才写入图正文。
- 图标签、原生面板拖动/分屏、边栏调整；右侧 Details、底部 Problems 消费当前编辑器投影。
  工作台使用一致的深色层级、资源图标、当前资源高亮、分节属性表单与空状态。
- 全图运行、单节点运行、运行至此、取消、执行快照恢复、失败定位、结果目录与分页表格。
  结果概览支持普通结构值、线性模型元数据与线性报告的系数/观测页；完整报告、分析与图形呈现仍待迁移。
- 未保存图暂不允许直接关闭标签；窗口关闭提供保存/放弃/取消。后台提交及全部保存期间不退出。

常用快捷键为 Ctrl+S、Ctrl+Z、Ctrl+Shift+Z/Ctrl+Y、Ctrl+A、Delete、Escape、Home；macOS
同时登记对应 Cmd 保存、历史与全选。输入框由原生组件处理文本快捷键。
F5 运行当前图，Shift+F5 请求取消。
保存与执行快捷键同时登记在工作台上下文，焦点位于 Details 输入框或侧栏时仍作用于当前图；
文本撤销继续归输入组件，图撤销归画布。Ctrl/Cmd+Shift+S 显式保存全部更改。

## 迁移范围与验收

完整迁移尚未完成。当前仍需补齐完整连接替换预览、Reroute、创建前配置、复杂常量的图形编辑器、
完整 Results 报告/图形/分析、完整项目管理、布局持久化、数据库编辑/图表/Mind/Markdown、Assistant、插件 UI 和打包。
现有 Tauri 入口在替代条件达成前保留。已确认的 Fedora 人工验收包括首阶段 Event 图显示与导航，
以及本轮单侧/双侧端口的 input 左、output 右布局和连线对齐。
其余视觉、属性和编辑交互仍需单独人工验收。UI 不添加单元测试。

真实项目验收使用副本：项目激活沿用目录初始化与数据库维护，编辑通过 Project 修改内存状态，
只有显式保存或现有助手事务写图正文。原项目路径 `/home/zy/New YssBI Project/` 不作为迁移编辑验收目标。

```sh
pnpm check:rs:package -p yss-desktop-gpui --bin yss-desktop-gpui
pnpm lint:rs:package -p yss-desktop-gpui --bin yss-desktop-gpui
pnpm format:rs:package -p yss-desktop-gpui -- --check
```

Fedora 需要 C/C++ 工具链以及 fontconfig、libxkbcommon、Wayland/X11、OpenSSL 开发库；
本机链接还需 `libxkbcommon-x11-devel`。需要可用图形驱动。Windows/macOS 的输入、平台路径、
窗口和分发尚未在目标系统验收。剩余工作见[迁移路线图](../../../docs/roadmap/GPUI_MIGRATION.md)。

原生 Logs 人工验收包括日志条目、刷新、清空显示、再次刷新恢复持久记录和退出后的订阅释放；
图编辑验收包括拖动、多选/框选、取消、连接、目录创建、撤销/重做、显式保存与脏窗口关闭确认。
属性验收包括开关和选项、数值/数列及顺序、列选择/未知列/失效列修复、筛选的空值与精确值类型、
编码映射和正值、参数默认值恢复、连接与字面量切换、端口增删/重排及相应的撤销。
同时确认图投影更新保留未变化字段的输入，打开着的旧选择菜单不会修改后来选中的节点。
从属性输入框按 Ctrl/Cmd+S、F5 和 Shift+F5，确认保存/运行/取消仍定位当前图。
图属性验收包括新建两类图、函数参数增删/重排与调用方端口更新、签名修改保留未保存正文、
常量改名/精确数值/复杂值/删除、拖动和中心插入引用、撤销/保存及过期菜单不能修改另一图。
运行与结果验收包括 F5、取消、单节点需求、无输出运行、命令拒绝与真实失败的区别、失败定位、
结果表分页与嵌套数据按需读取、重跑后的已打开结果保持、标签拖动/关闭和项目切换后的资源释放。

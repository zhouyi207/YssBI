# Sidebar 空状态统一设计

## 背景

编辑器 Sidebar 当前将空状态混入 `FlatSidebarRow`，再统一交给虚拟列表和 `OverlayScrollbar` 渲染。节点目录暂不可用时，Sidebar 只有一条英文提示，但该提示仍作为固定高度的虚拟行渲染。文案换行后产生垂直溢出，最终显示出没有实际用途的滚动条和上下箭头。

这不仅是 CSS 问题，也说明当前模型混淆了三类不同概念：资源数据行、分区空状态和标签页级空状态。

## 目标

- 统一所有 Sidebar 标签页与分区的空状态表达和视觉样式。
- 从 `FlatSidebarRow` 领域模型中移除 `empty` 类型。
- 标签页级空状态不创建虚拟列表或滚动容器。
- 分区级空状态保持在对应分区下方，并与资源列表共享唯一的外层滚动区域。
- 保留长列表虚拟化、分区展开状态、资源操作和右键菜单行为。
- 所有用户可见空状态文案通过 i18n 提供。

## 非目标

- 不恢复或实现节点目录数据源。
- 不修改 Zustand 资源 Store 或后端权威状态。
- 不修改共享 `OverlayScrollbar` 的滚动判断逻辑。
- 不改变 Graph、Variable、Database、Worksheet 的选择、打开、拖放或右键菜单行为。
- 不移除正常资源列表的虚拟化。

## 空状态分类

### 标签页级空状态

整个标签页当前没有可展示的主体内容：

- 节点目录暂不可用。
- Commands 标签页没有活动图。
- 节点目录搜索没有匹配结果，同时仍需保留搜索控件。

标签页级空状态使用独立的 `SidebarEmptyState`，不进入虚拟列表。

### 分区级空状态

标签页结构仍然有效，但某个展开分区没有资源：

- 没有 Event。
- 没有 Function。
- 没有局部变量。
- 没有全局变量。
- 没有数据。
- 没有 Worksheet。

分区级空状态由分区模型表达，并由 View 层适配成内部渲染行。

## 数据模型

### 移除领域层空行

`FlatSidebarRow` 不再包含 `empty` 分支，只描述可交互的 Sidebar 结构和资源：

```ts
type FlatSidebarRow =
  | SidebarSectionRow
  | SidebarGroupRow
  | SidebarGraphRow
  | SidebarVariableRow
  | SidebarDatabaseRow
  | SidebarWorksheetRow
  | SidebarNodeRow;
```

### 结构化分区模型

Graph、Variable、Data 和 Charts 的 Builder 返回结构化面板模型，而不是提前混入空行的扁平数组：

```ts
interface SidebarSectionModel<Row> {
  key: SidebarSectionKey;
  label: string;
  expanded: boolean;
  rows: Row[];
  emptyMessage?: string;
}

interface SidebarPanelModel<Row> {
  sections: SidebarSectionModel<Row>[];
  emptyState?: SidebarEmptyStateModel;
}
```

标签页级空状态模型为：

```ts
interface SidebarEmptyStateModel {
  title: string;
  description?: string;
  action?: {
    label: string;
    command: string;
  };
}
```

当前范围不要求任何空状态操作按钮，但保留可选字段，使组件不需要在未来增加操作时重新设计接口。操作执行仍应由应用层传入，展示组件不得直接调用服务。

### View 层渲染适配器

新增纯函数 `flattenSidebarPanelModel()`：

```text
SidebarPanelModel
        ↓
flattenSidebarPanelModel()
        ↓
SidebarRenderRow[]
        ↓
virtualizer
```

`SidebarRenderRow` 是 View 层内部类型，可以包含 `sectionEmpty`。它不从 `features/core/sidebar` 导出，也不成为资源领域模型的一部分。

适配规则：

1. 每个分区首先生成分区标题渲染行。
2. 折叠分区不生成内容行或空状态行。
3. 展开且有内容的分区生成对应资源行。
4. 展开、无内容且提供 `emptyMessage` 的分区生成一个 `sectionEmpty` 渲染行。
5. `sectionEmpty` 保留 `sectionKey`，用于绑定现有的分区内容区右键菜单。
6. 多分区的顺序严格遵循模型中的 `sections` 顺序。

节点树可继续使用节点树自身的分组模型；当整个目录不可用或搜索无匹配时，标签页负责绕过虚拟列表并渲染 `SidebarEmptyState`。

## 组件设计

### `SidebarEmptyState`

职责：展示整个 Sidebar 标签页的不可用、无上下文或无搜索结果状态。

建议属性：

```ts
interface SidebarEmptyStateProps {
  title: string;
  description?: string;
  action?: ReactNode;
  className?: string;
}
```

视觉规则：

- 左对齐并使用 `px-3 py-3`。
- 标题使用普通 Sidebar 前景色。
- 说明使用 `text-muted-foreground`。
- 文案允许自然换行。
- 不使用卡片边框、大图标或强调背景。
- 没有操作时不保留按钮占位。
- 组件自身不引入 `OverlayScrollbar`。

### `SidebarSectionEmptyState`

职责：展示展开分区中的紧凑空状态。

视觉和交互规则：

- 保持 `h-7`，与普通 Sidebar 行一致。
- 使用分区内容的一级缩进。
- 使用 `whitespace-nowrap overflow-hidden text-ellipsis`。
- 文案超出可视宽度时通过 Tooltip 展示完整内容。
- 不显示图标。
- 保留对应分区内容区域的右键菜单。
- 分区折叠时不渲染。

### 列表与滚动容器

- 标签页级空状态不挂载 `SidebarFlatRowList`、virtualizer 或 `OverlayScrollbar`。
- 正常资源面板只保留一个外层 `OverlayScrollbar`。
- 分区级空状态参与这个外层列表的布局，不建立嵌套滚动区域。
- `OverlayScrollbar` 只在内容实际超出可视区域时显示。
- 共享滚动条组件不为 Sidebar 增加空状态特例。

## 标签页行为

### Nodes

节点目录不可用时：

```text
SidebarNodesTab
└── SidebarEmptyState
```

不再创建仅包含一条错误提示的 `SidebarFlatRowPanel`。

节点目录可用但搜索无匹配时：

```text
SidebarNodesTab
├── Search
└── SidebarEmptyState
```

搜索框保留，结果列表和滚动容器不挂载。

### Commands

没有活动图时使用 `SidebarEmptyState`，替换当前标签页内的临时文本 `div`。有活动图时保持 Undo/Redo 内容和行为不变。

### Graphs、Variables、Data、Charts

这些标签页使用结构化 `SidebarPanelModel`：

- 分区标题始终由分区模型生成。
- 展开且为空的分区显示 `SidebarSectionEmptyState`。
- 有资源时显示原有资源行。
- 折叠、选择、添加按钮和右键菜单行为保持不变。

## i18n 文案

所有空状态文案进入中文和英文 locale，不直接在组件中显示英文常量。

建议中文文案：

| 场景 | 标题/文案 | 说明 |
| --- | --- | --- |
| 节点目录不可用 | 节点目录暂不可用 | 等待稳定的节点目录描述信息 |
| 节点搜索无结果 | 未找到匹配的节点 | 无 |
| Commands 无活动图 | 未打开活动图 | 打开一个 Event 或 Function 后可查看命令状态 |
| 空 Event 分区 | 暂无 Event | 无 |
| 空 Function 分区 | 暂无 Function | 无 |
| 空局部变量分区 | 暂无局部变量 | 无 |
| 空全局变量分区 | 暂无全局变量 | 无 |
| 空数据分区 | 暂无数据 | 无 |
| 空 Worksheet 分区 | 暂无工作表 | 无 |

英文 locale 提供语义对应的简短文案。现有应用层能力说明常量可以继续用于非 Sidebar 场景，但 Sidebar 不应直接展示未本地化的英文常量。

## 状态与数据流

```text
Zustand/resource projection
          ↓
Sidebar tab hook
          ↓
Pure panel builder
          ↓
SidebarPanelModel
          ↓
┌──────────────────────────────────────┐
│ tab-level emptyState exists          │──→ SidebarEmptyState
└──────────────────────────────────────┘
          │ otherwise
          ↓
flattenSidebarPanelModel
          ↓
SidebarRenderRow[]
          ↓
SidebarFlatRowList + virtualizer
          ↓
resource row / SidebarSectionEmptyState
```

所有 Builder 保持纯函数，不读取 React 状态、不调用服务，也不执行资源操作。

## 错误处理

- 缺少可选 `emptyMessage` 时，空分区只显示分区标题，不生成空状态行。
- 未知渲染行类型通过 TypeScript 穷尽检查暴露，而不是静默忽略。
- Tooltip 仅改善被截断文案的可访问性，不承担错误恢复职责。
- 节点目录不可用是明确的能力状态，不作为异常抛出。

## 测试设计

### Builder 单元测试

覆盖 Graph、Variable、Data、Charts Builder：

- 返回结构化 `sections`。
- 空分区的 `rows` 是空数组。
- 空分区通过 `emptyMessage` 表达。
- 分区顺序保持不变。
- 展开状态正确投影到模型。
- `FlatSidebarRow` 不再包含 `empty`。

### 渲染适配器单元测试

为 `flattenSidebarPanelModel()` 覆盖：

- 展开的空分区生成一个 `sectionEmpty`。
- 折叠的空分区不生成 `sectionEmpty`。
- 有内容的分区不生成 `sectionEmpty`。
- 多分区顺序保持稳定。
- `sectionEmpty` 保留正确的 `sectionKey`。
- 缺少 `emptyMessage` 的空分区不会生成占位行。

### 组件测试

覆盖：

- `SidebarEmptyState` 不包含 `OverlayScrollbar`。
- `SidebarSectionEmptyState` 使用固定行高和单行截断。
- 超长分区空状态可通过 Tooltip 查看全文。
- 节点目录不可用时不挂载虚拟列表。
- Commands 无活动图时使用统一标签页级空状态。
- 正常资源列表仍挂载虚拟列表。
- 分区空状态仍能触发现有内容区右键菜单。

### 回归验证

实现完成后运行：

```text
pnpm test <Sidebar 相关测试文件>
pnpm typecheck
git diff --check
```

如变更触及更广泛的前端测试依赖，再运行相关的 `pnpm test` 覆盖，但不在本设计中引入 Rust 变更。

## 验收标准

- 截图中的节点 Sidebar 空状态不再出现滚动条或上下箭头。
- 所有标签页级空状态使用同一组件和一致排版。
- 所有分区级空状态使用同一组件和一致排版。
- 空状态文案均已本地化。
- `FlatSidebarRow` 不再包含 `empty`。
- 正常长列表仍然可以滚动并保持虚拟化。
- 分区折叠、添加、选择和右键菜单行为无回归。

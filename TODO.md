# TODOLIST

## 📌 优先级：高 (核心功能扩展)

- [ ] **连线交互增强**：
  - [ ] 实现点击连线进行“选中”状态显示。
  - [ ] 选中连线后按 `Delete` 键可直接删除该连接。
- [ ] **属性编辑器深度整合**：
  - [ ] 在右侧 `Detail` 中显示选中节点的详细参数（如常量值、函数配置）。
  - [ ] 支持在右侧栏直接修改节点的非 Pin 参数。
- [ ] **子图系统 (Functions & Macros)**：
  - [ ] 实现双击侧边栏的函数/宏进入独立编辑画布。
  - [ ] 函数/宏节点的输入输出定义逻辑。
- [ ] **自动吸附**：节点拖拽时支持网格对齐 (Grid Snapping)。
- [ ] **文档修改**：状态

## 🛠️ 已完成 (Done)

- [x] **撤销/重做系统 (Undo/Redo)**：实现基于历史快照的完整撤销重做功能 - 2026.01.20
- [x] **全局快捷键**：完成 `Ctrl + S/Shift+S`, `Ctrl + O`, `Ctrl + Z/Y`, `Ctrl + C/V/X`, `Delete` 等全局监听 - 2026.01.20
- [x] **多标签页系统 (Tabs)**：支持多画布并行编辑，独立状态管理，支持文件路径关联 - 2026.01.20
- [x] **变量作用域体系**：
  - [x] 实现全局变量 (App-wide) 与局部变量 (Tab-local) 的隔离。
  - [x] 支持通过“小眼睛”图标进行变量作用域的快速提升/降级 - 2026.01.20
- [x] **界面架构重构 (Sidebar)**：
  - [x] 新增右侧属性栏 (`Detail`)，迁移变量详情编辑逻辑。
  - [x] 左侧栏实现 **Functions**, **Macros**, **Variables** 的可折叠列表管理 - 2026.01.20
- [x] **智能连线交互**：
  - [x] 连线释放于空白处自动弹出节点菜单，并保持连线状态。
  - [x] 节点菜单根据起始 Pin 类型和方向自动筛选匹配的节点 - 2026.01.20
  - [x] 优化 Pin 的视觉反馈，连接中状态即时更新图标 - 2026.01.20
- [x] **执行按钮迁移**：将“Execute”按钮从顶栏移至标签栏右侧，更符合操作流 - 2026.01.20
- [x] **后端驱动架构**：节点元数据由 Rust 后端统一定义并同步至前端 - 2026.01.19
- [x] **窗口自定义**：隐藏原生标题栏，实现自定义拖拽及窗口控制 - 2026.01.19
- [x] **文件操作协议 (v1.0.0)**：接入 Tauri Dialog/FS，支持多页数据、作用域变量、函数/宏的完整序列化 - 2026.01.20

## 🎨 前端 (视觉与 UX)

- [ ] **视觉修复**：解决毛玻璃的残影无法解决的问题 - 2026.01.17。
- [x] **菜单栏完善**：完成 File 与 Edit 菜单中大部分核心功能的快捷键映射与逻辑。

## ⚙️ 后端 (Rust/Tauri)

- [ ] **本地设置存储**：使用插件存储用户偏好（如窗口位置、上次打开的文件列表）。
- [ ] **Rust 逻辑计算层**：考虑将复杂的图计算或验证逻辑移至 Rust 端以提升性能。


## 项目文件

文件扩展名建议
扩展名	含义
.yss	主项目文件
.yssb	蓝图/子图模板文件（可导出单个 function/macro）
.yssp	项目包（包含资源的完整导出）
总结
项目规模	推荐方案
小型（< 100 节点）	单 JSON 文件（当前方案）
中型（100-1000 节点）	JSON + 压缩
大型（> 1000 节点）	ZIP 混合方案
超大型/团队协作	文件夹结构 + Git

## 文件读取

阶段 1（现在）: 前端 Plugin
├── 简单 JSON 读写
└── 快速开发

阶段 2（功能稳定后）: 前端 + 后端混合
├── 简单操作 → 前端 Plugin
└── 复杂操作 → Rust Command
    ├── 数据校验
    ├── 版本迁移
    └── 压缩/加密

阶段 3（大规模使用）: 主要后端
├── 所有文件操作 → Rust
└── 前端只负责 UI


好问题！让我看看你项目中还有哪些定义，然后给出全面的建议。

[4 tools called]

根据你的项目结构，以下是我推荐在后端定义的内容完整列表：

---

## 推荐后端定义的内容

### 1. ✅ 节点定义 (NodeDefinition) — 你已经在做

```rust
// 后端已有，继续保持
pub struct NodeDefinition {
    pub node_type: String,
    pub category: String,
    pub title: String,
    pub inputs: Vec<PinDefinition>,
    pub outputs: Vec<PinDefinition>,
    pub ui_style: String,
    // ...
}
```

---

### 2. ✅ Pin 类型定义 — 上一个问题讨论的

包括颜色、显示名称、图标等 UI 元数据。

---

### 3. 🔴 类型兼容性/转换规则

**目前前端可能在做的：** 判断两个 pin 能否连接

**推荐后端定义：**

```rust
pub struct TypeCompatibility {
    pub from_type: String,
    pub to_type: String,
    pub conversion: ConversionKind,  // Implicit, Explicit, None
}

// 或者在 PinTypeDefinition 中
pub struct PinTypeDefinition {
    pub name: String,
    pub can_connect_to: Vec<String>,      // 可以连接的类型
    pub implicit_convert_to: Vec<String>, // 可以隐式转换的类型
}
```

**为什么后端定义更好：**
- 执行时后端需要知道如何转换类型
- 避免前端允许连接但后端执行失败的情况

---

### 4. 🔴 变量类型系统

你的 `Variable` 接口：

```typescript
export interface Variable {
  name: string;
  type: string;  // 这个 type 应该由后端定义可选值
  value: any;
}
```

**推荐后端定义：**

```rust
pub struct VariableTypeDefinition {
    pub type_name: String,         // "int", "float", "string", "bool"
    pub default_value: Value,      // 默认值
    pub pin_type: String,          // 对应的 pin 类型
    pub editor_widget: String,     // 前端用什么控件编辑 ("number", "text", "checkbox")
}
```

---

### 5. 🔴 节点分类 (Categories)

**目前：** 分类字符串散落在代码中 `"Internal"`, `"Variable"`, `"Function"`, `"Macro"`

**推荐后端统一定义：**

```rust
pub struct CategoryDefinition {
    pub name: String,           // "Flow Control"
    pub display_name: String,   // 显示名
    pub icon: Option<String>,   // 图标
    pub sort_order: i32,        // 排序
    pub color: Option<String>,  // 分类颜色
}
```

---

### 6. 🔴 UI 样式定义 (UI Styles)

**目前：** `ui_style: "default" | "math" | "event"` 散落在代码中

```typescript
// 前端硬编码了样式判断
if (this.uiStyle === "math") {
  if (this.type === "add") this.centerSymbol = "+";
}
```

**推荐后端定义：**

```rust
pub struct UIStyleDefinition {
    pub name: String,           // "math"
    pub has_header: bool,       // 是否有标题栏
    pub center_symbol: Option<String>,  // 中心符号
    pub header_color: Option<String>,
    pub compact: bool,          // 紧凑模式
}
```

---

### 7. 🔴 验证规则

**推荐后端定义：**

```rust
// 例如：哪些节点必须有连接、哪些输入是必填的
pub struct ValidationRule {
    pub node_type: String,
    pub required_inputs: Vec<String>,   // 必须连接的输入 pin
    pub required_outputs: Vec<String>,  // 必须连接的输出 pin
}
```

---

## 推荐保留在前端的内容

| 内容 | 原因 |
|------|------|
| **交互状态** (选中、拖拽、手势) | 纯 UI 状态，后端不需要 |
| **视口状态** (缩放、平移) | 纯 UI |
| **Undo/Redo 历史** | 前端响应更快 |
| **布局计算** | 需要实时响应 |
| **动画/过渡效果** | 纯视觉 |

---

## 推荐的架构模式

```
┌─────────────────────────────────────────────────────────┐
│                      后端 (Rust)                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │  Schema Registry (类型权威数据源)                 │   │
│  │  - PinTypeDefinitions                            │   │
│  │  - NodeDefinitions                               │   │
│  │  - VariableTypeDefinitions                       │   │
│  │  - CategoryDefinitions                           │   │
│  │  - UIStyleDefinitions                            │   │
│  │  - TypeCompatibilityRules                        │   │
│  └─────────────────────────────────────────────────┘   │
│                          │                              │
│                    Tauri Commands                       │
│                          │                              │
└──────────────────────────┼──────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│                      前端 (React)                        │
│  ┌─────────────────────────────────────────────────┐   │
│  │  Schema Cache (启动时获取并缓存)                  │   │
│  │  - pinTypes: Map<string, PinTypeMeta>            │   │
│  │  - categories: Map<string, CategoryMeta>         │   │
│  │  - uiStyles: Map<string, UIStyleMeta>            │   │
│  └─────────────────────────────────────────────────┘   │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │  UI State (纯前端)                               │   │
│  │  - gesture, selection, viewport, history        │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

---

## 启动流程建议

```typescript
// 前端启动时
async function initializeEditor() {
  // 一次性获取所有 schema
  const [nodeDefinitions, pinTypes, categories, uiStyles] = await Promise.all([
    invoke('get_node_definitions'),
    invoke('get_pin_types'),
    invoke('get_categories'),
    invoke('get_ui_styles'),
  ]);
  
  // 缓存到全局 store
  schemaStore.setNodeDefinitions(nodeDefinitions);
  schemaStore.setPinTypes(pinTypes);
  // ...
}
```

这样做的最大好处是：**将来添加新类型、新节点、新分类时，只需要修改后端代码，前端自动适应**。
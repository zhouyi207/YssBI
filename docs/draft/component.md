**可以，而且这个方向适合 YssBI。但我建议把目标明确为：组件通过受控接口协作，内部实现和可变状态相互隐藏；跨组件业务由应用层编排，而不是允许组件任意相互依赖。**

换句话说，你想要的不是简单的“把代码分到四个文件夹”，而是：

> **单一应用入口 + 按业务能力划分的组件 + 显式接口 + 统一的跨组件协调。**

我检查了仓库 `main` 分支的 `769f8d7`，包括 `yss-application` 的入口、会话、图运行准备、项目生命周期、数据库变更，以及节点注册和图运行时代码。以下是基于这些实现的静态架构分析，没有执行项目构建。

## 一、这种设计叫什么？不是严格意义上的 ECS

你提到的 `esc`，结合上下文，应该是在说 **ECS：Entity Component System，实体—组件—系统**。标准 ECS 的核心是：实体提供身份、组件承载数据、系统处理具有特定组件的数据集合；它不是“几个业务服务通过接口互相调用”的同义词。([bevy.org][1])

对于你描述的设计，**我会称为“组件化的模块化单体架构”**。组件具有自己的公开接口、内部实现，以及对其他组件接口的明确依赖，这与模块化应用的典型定义一致。需要进一步隔离实现时，可以结合 Ports & Adapters，也就是端口与适配器。([Home][2])

名称不是最重要的，但这个区别会影响实现：

**你需要的是封装和职责边界，不一定需要 ECS 的 World、Entity、System 调度，也不需要先造一个组件框架。**

---

## 二、先澄清：“互相引用，但是彼此不可见”应该做到哪一层？

这句话可以分成三种情况：

| 你的实际目标                     | 是否可行      | 建议                |
| -------------------------- | --------- | ----------------- |
| 可以调用其他组件的 API，但不能访问其内部实现   | 可行        | **作为默认方案**        |
| 连其他组件的具体实现类型都不认识，只认识 trait | 可行        | 用于确实需要隔离或替换的边界    |
| 几个独立 crate 任意循环依赖，但彼此内部不可见 | 不可按这种方式实现 | 普通 Cargo 依赖必须避免循环 |

Rust 的模块可见性可以隐藏内部实现；而独立 package 的普通依赖不能形成循环。Cargo 对某些开发依赖循环有特殊处理，但这不适用于你的业务组件互相依赖。([Rust 文档][3])

因此，我建议你把要求写成：

> **允许组件依赖其他组件的公开契约，不允许穿透内部实现；组件依赖方向必须明确，不能形成无约束的双向调用网。**

“实现不可见”和“完全没有依赖”是两件事。调用方至少需要知道它调用的操作、输入、输出和失败条件。

## 三、你的项目已经具备组件化基础，但有几个地方不能直接搬

### 1. 单一应用入口已经基本建立了

当前 `src-tauri/src/lib.rs` 已通过：

```rust
.setup(yss_application::initialize)
.invoke_handler(yss_application::invoke_handler())
```

接入应用初始化和命令处理。这部分已经接近你希望的“宿主通过 `yss-application` 使用业务能力”的形式。

但是，`yss-application/src/lib.rs` 仍然公开了大量模块，例如：

```rust
pub mod database;
pub mod execution;
pub mod graph_run;
pub mod graph_contracts;
pub mod project_lifecycle;
pub mod project_query;
pub mod resource_mutation;
pub mod runtime;
```

所以目前是**入口集中，但公开表面仍然比较宽**。

我的建议是：保留单一入口，逐步将这些散落的业务操作归入组件或应用用例，最后只导出宿主真正需要的接口。

还有一个具体的收口点：宿主现在通过 `yss_application::execution::ApplicationState` 处理窗口关闭后的结果清理。以后可以提供应用级的窗口关闭通知接口，而不是让宿主知道应用状态放在 `execution` 模块中。

### 2. `ApplicationSession` 不应该成为某一个业务组件的内部状态

当前 `ApplicationSession` 同时持有：

```text
ProjectState
GraphRuntimeState
ExecutionRuntimeState
DatabaseRuntimeSession
ResourceProviderFactory
```

并关联项目实例、项目会话、执行会话、运行代次和应用会话 epoch。

`session_factory.rs` 还会检查这些身份是否匹配，再生成尚未发布的完整会话候选。也就是说，这不是一个随意存放几个对象的容器，而是在维持**跨组件的一致性边界**。

所以我的判断是：

> **Project、Graph、Database、Execution 可以拆，但把它们绑定到同一应用会话的逻辑，应留在应用级 `session` / `lifecycle` 层。**

当前这些类型位于：

```text
execution/session_slot.rs
execution/session_factory.rs
```

从它们实际承担的职责看，我更倾向于迁到：

```text
session/slot.rs
session/factory.rs
```

而不是原封不动塞进 `execution-component`。

**统一协调会话，不等于所有组件共享一个任意读写的大状态。** 协调层应掌握整体绑定关系，业务组件只获得完成任务所需的能力。

### 3. `graph_run.rs` 实际上是跨组件用例，不只是 Graph 内部逻辑

当时的图解析用例协调以下步骤；当前入口与归属以 Graph 与 Execution 架构文档为准：

```text
捕获 ApplicationSession
    ↓
检查项目身份与图资源
    ↓
读取项目事实和数据库 catalog
    ↓
构造图运行准备资源输入
    ↓
解析图、生成编辑器投影
    ↓
重新验证项目、数据库和会话
    ↓
更新执行结果输入观察信息
```

这个函数同时协调 Project、Database、Graph 和 Execution。

这类代码放在应用层是有理由的，**不能因为名字叫 `graph_run`，就把它全部当成 Graph 的内部实现搬进去**。

我建议拆成两个层次：

```text
workflows/resolve_graph.rs
    负责取哪些事实、何时验证、如何协调其他组件

components/graph/
    负责给定输入后，如何分析和生成图投影
```

这样 Graph 不需要自己去找 Project、再找 Database、最后通知 Execution。

### 4. 数据库变更已经有你想要的“通过接口协作”的雏形

当前 `database_mutation.rs` 已定义：

```rust
pub(crate) trait ProjectDatabaseMutationPort: Send + Sync {
    fn prepare(/* ... */) -> /* ... */;

    fn finalize(/* ... */) -> /* ... */;
}
```

其 `finalize` 契约还明确规定不能执行 Database I/O，并且必须消费已准备的 Project 权限对象。周围也已经有数据库提交、项目最终确认和补偿相关的类型。

**这部分值得保留和整理，而不是为了“新组件架构”全部推倒。**

我的建议是将职责整理为：

```text
Database 组件：数据库操作及其状态
Project 组件：项目声明、提交权限及其状态
应用工作流：准备 → 提交 → 最终确认 → 失败补偿
```

接口可以隐藏实现，但提交顺序、补偿责任不能被隐藏成一串难以追踪的事件。

### 5. 独立 crate 拆分还有一个具体障碍：`impl ApplicationState`

当前 `project_lifecycle` 和 `automation` 等模块都在为同一个 `ApplicationState` 定义固有方法。这在同一 crate 的不同模块中是合法的。

但拆成独立 crate 后，不能继续在另一个 crate 中写：

```rust
impl ApplicationState {
    // 为外部 crate 定义的类型添加固有方法
}
```

这会碰到 Rust 的 `E0116`：固有实现必须与类型定义位于同一 crate。([Rust 文档][4])

因此，**真正拆 crate 前，需要先把业务行为重新归属到组件自己的类型或独立用例函数中**。我不建议为了维持原样，再给每个组件补一堆扩展 trait。

## 四、我建议你如何划分这些组件？

### 推荐结构：组件层之外，保留应用编排层

第一阶段，我建议在现有 `yss-application` 内建立如下逻辑结构：

```text
yss-application/
└─ src/
   ├─ lib.rs                  应用公开入口
   ├─ runtime.rs              组装具体实现、初始化
   ├─ ipc/                    Tauri 命令及传输适配
   │
   ├─ session/                跨组件会话、一致性和生命周期
   │
   ├─ workflows/              跨组件业务用例
   │  ├─ resolve_graph.rs
   │  ├─ run_graph.rs
   │  ├─ switch_project.rs
   │  └─ mutate_database.rs
   │
   └─ components/
      ├─ graph/
      ├─ node_catalog/
      ├─ database/
      ├─ project/
      ├─ execution/
      └─ ...
```

这是建议的目标结构，不是仓库当前目录。

**四个组件可以作为起点，但不要为了凑四个，把执行、图表、Harness 等所有能力都硬塞进去。** 当前应用本来就包含这些不同能力，应该按照实际职责逐步归类。

我对主要边界的建议如下：

| 组件或层                  | 应负责                        | 不应负责                       |
| --------------------- | -------------------------- | -------------------------- |
| `graph`               | 图结构规则、编辑运算、语义分析、图相关派生缓存 | 项目切换、数据库连接管理、全局会话协调        |
| `node_catalog`        | 节点类型、协议、注册、目录、节点定义查询       | 某张图中的节点实例及连线的独立可写状态        |
| `database`            | 数据集访问、连接、schema、查询、数据变更能力  | 所有使用 SQLite 的业务存储、项目全局生命周期 |
| `project`             | 项目文档、资源声明、项目级写入权限、持久化规则    | 图执行器、查询执行器、其他组件的内部状态       |
| `execution`           | 执行任务、运行值、结果、取消和结果生命周期      | 全应用会话的所有业务行为               |
| `workflows + session` | 跨组件顺序、一致性检查、会话切换、失败协调      | 复制实现每个组件内部的算法              |

### 最重要的一点：Node 的“定义”和“实例”不能混在一起

你的仓库已经分别存在：

```text
节点定义体系
    yss-node-protocol
    yss-node-registry

图中实例体系
    GraphDocument
    DocumentNode
    DocumentConnection
```

注册表提供节点协议查询，并通过 `NodeRegistryBuilder::freeze` 构建注册表；图文档则持有图中的节点实例、连线、参数和端口绑定等数据。

因此，我建议：

```text
“OLS 节点是什么、有哪些参数和端口”
    → NodeCatalog

“这张图里的 OLS 节点连接了哪些节点”
    → Graph
```

不要出现：

```text
GraphComponent 管理连线
NodeComponent 单独管理图中节点实例
```

否则删除一个节点时，就得跨两个组件协调节点、连线、端口绑定和缓存。你本来希望减少耦合，却可能把一个本应局部完成的图编辑操作拆成跨组件事务。

另外，当前 `GraphRuntimeState` 已经持有注册表和目录引用，以及编辑解析和语义缓存。拆分时应该整理这些职责，**不是让 NodeComponent 和 GraphComponent 再各建一份注册表或各建一份权威图状态**。

### Database 也不应该变成“所有存储的总管”

当前运行时已经分别组装项目注册表的 SQLite 存储，以及 Harness 相关服务。

我建议按业务归属管理它们，而不是因为底层都用了数据库，就全部搬到 `database-component`：

```text
用户分析数据的数据库能力 → Database
项目注册表存储           → Project 相关能力
Harness 会话存储         → Harness 相关能力
```

**组件按业务职责划分，底层存储技术相同，不代表应该属于同一个业务组件。**

## 五、组件之间应该怎么协作？

我建议采用“简单依赖直接调用，复杂业务集中编排，需要隔离时才加端口”的方式。

### 情况一：稳定的单向依赖，直接调用公开 API

例如 Graph 需要查询节点定义：

```text
Graph → NodeCatalog 的公开查询接口
```

只要 NodeCatalog 不反向依赖 Graph 的实现，这个关系就很清楚。

**不必为了“解耦”给每个方法都创建 trait。** 一个隐藏字段、隐藏内部模块的具体服务类型，也可以提供很好的封装。

### 情况二：横跨多个组件的流程，放到工作流中

例如解析图、运行图、切换项目、修改数据库声明：

```text
              ResolveGraphWorkflow
                 /     |     \
                /      |      \
           Project   Database   Graph
```

这里箭头表示工作流调用组件能力，不是组件之间互相持有整个应用。

针对当前解析流程，我建议工作流获取**绑定到同一会话、带有版本依据的事实或快照**，交给 Graph 处理，再执行所需的重新验证。不要为了接口简洁而删掉现有身份和版本校验。当前实现已经在做这类检查。

对于写操作，还应继续保留由状态所属组件执行的最终权限和版本检查；工作流负责协调，但不绕过组件的提交规则。

### 情况三：确实要求“不认识对方的实现”，使用窄 trait

例如你希望 Graph 连 `NodeCatalogComponent` 这个具体类型都不依赖，可以设计：

```text
Graph 实现 ───────→ NodeCatalogRead 契约
                               ↑
NodeCatalog 实现 ───────────────┘

runtime 负责创建具体对象并注入
```

这是端口与适配器的用法：调用方依赖有明确用途的契约，具体实现由外层连接。([Alistair Cockburn][5])

但是，**加 trait 本身不会自动消除依赖循环**。契约必须放在不会反向依赖实现的位置，组装代码也不能被底层组件反向引用。

对于 YssBI，我不建议立刻新建一个巨大的：

```text
yss-component-contracts
```

把所有类型、错误、事件、trait 都塞进去。你的依赖中已经有节点协议、图资源契约、数据库契约、项目身份等细分 crate；应先复用其中真正适合的契约，只为确实缺少的边界增加接口。

同样不建议给每个组件注入：

```rust
Arc<ApplicationState>
```

然后让它自己寻找其他组件。这样虽然字段表面上藏起来了，但每个组件仍然间接拥有访问全应用的能力。

**应注入所需能力，而不是注入整个应用。**

## 六、Rust 中如何实现“接口可见，内部不可见”？

同一个 crate 内就能先实现相当清晰的封装。

例如下面是一个可见性结构示意：

```rust
// components/graph/mod.rs

mod api;
mod cache;
mod analysis;
mod projection;

pub(crate) use api::{
    GraphComponent,
    ResolveRequest,
    ResolveReceipt,
    ResolveError,
};
```

这样，其他组件使用的是：

```rust
use crate::components::graph::GraphComponent;
```

而不是：

```rust
use crate::components::graph::cache::AnalysisCache;
```

Rust 的私有子模块、私有字段和受限可见性能够支持这种边界。`pub(crate)` 表示当前 crate 内可见，不表示仅某个组件可见；组件内部需要相互访问的项，可以用 `pub(super)` 或限定到所属组件祖先模块的 `pub(in ...)`。([Rust 文档][3])

这里还有一个需要注意的限制：

> **Rust 的 `pub(in path)` 只能指定祖先模块，不能用它给任意一个兄弟组件授予“友元访问”。**([Rust 文档][3])

所以真正有价值的封装是：

```text
公开：业务操作、明确的输入输出、受控的句柄
隐藏：可变状态、锁、缓存结构、内部辅助模块
```

而不是只把目录设为私有，然后又通过 API 返回内部可变状态的引用。

### 那么现在应该拆成独立 crate 吗？

**我的建议是：先完成逻辑组件化，再选择性拆 crate。**

不是说多 crate 不好，而是你现在直接物理拆分，会同时面对：

```text
ApplicationState 的固有方法需要重新归属
pub(crate) 的跨模块访问失效
跨组件流程需要重新划分
公开输入输出需要稳定下来
会话一致性约束需要保留
```

这些问题应该先在清晰的职责模型下解决，而不是一边处理编译错误，一边临时发明 `shared`、`common` 和转发层。

另一方面，已有的底层算法、协议和运行时 crate，也不需要为了“统一组件化”全部合并回 `yss-application`。**逻辑组件与 Cargo crate 不必一一对应。**

一个 Graph 组件可以组合使用现有多个图相关 crate，而不是复制它们；一个小型业务组件也可以暂时只是一个 Rust module。

## 七、这样做的优势、代价，以及我建议的迁移顺序

### 真正的收益与代价

| 方面   | 能得到的收益                     | 需要承担的代价           |
| ---- | -------------------------- | ----------------- |
| 修改范围 | 内部实现变化更容易限制在组件内部           | 需要认真设计公开接口        |
| 状态管理 | 谁能修改什么更加明确                 | 跨组件提交不能靠随意调用完成    |
| 测试   | 可以围绕组件行为和关键契约验证            | 会话切换、补偿等跨组件测试仍需保留 |
| 复用   | UI、Harness、插件适配更容易调用同一业务能力 | 不能只是给旧函数再包一层转发    |
| 构建边界 | 独立 crate 可提供更明确的依赖约束       | 不应预先假定拆得更多就一定编译更快 |

对你而言，我认为最值得警惕的是：

> **不要从“模块太散”，走向“接口、转发、契约和组件管理器太多”。**

每个新边界都应该能够回答：它隔离了什么变化？谁拥有状态？调用方因此少知道了什么？

### 我建议分三步迁移

**第一步：先把会话与跨组件编排识别出来。**

优先整理 `ApplicationSession`、会话构建/切换，以及图运行准备、数据库变更这些跨组件用例。现有运行时已经有集中组装具体服务的入口，可以沿用，不需要再造一个全局组件注册中心。

**第二步：建立组件外观，收窄可见性。**

把真正属于 Graph、NodeCatalog、Database、Project 的行为归位。组件内部默认私有，只开放业务 API；跨组件调用只走这些 API。逐步减少业务模块通过整个 `ApplicationSession` 获取其他所有状态的情况，但保留应用级会话协调。

**第三步：边界稳定后，再决定哪些组件值得成为 crate。**

需要独立复用、独立依赖集合或更强编译约束的组件再拆出去。不要同时改变目录、状态权威、IPC 语义、异步执行方式和错误处理模型。

验证也应围绕这次改动最容易破坏的边界：会话切换、过期快照拒绝、图运行准备输入一致性、数据库提交与补偿，而不是为了每个新目录添加形式化测试。

---

## 我的最终建议

**我赞成你做组件化，但不赞成“四个组件任意互相引用，只把内部字段藏起来”的版本。**

我更推荐你把 YssBI 的目标定义为：

> **`yss-application` 保留统一入口、组装和跨组件工作流；Graph、NodeCatalog、Database、Project 等组件封装各自业务能力与状态访问。组件可以依赖其他组件的公开 API，但依赖方向必须受控；内部可变状态不得穿透，共同业务由明确的工作流协调。**

其中，当前最重要的三件事是：**把应用会话从 Execution 的概念中独立出来，区分节点定义与图中节点实例，保留图运行准备和数据库变更中的跨组件一致性约束。**

这样拆，才是在降低耦合；否则很可能只是给现有耦合关系换一组 `*-component` 名字。

[1]: https://bevy.org/learn/quick-start/getting-started/ecs/?utm_source=chatgpt.com "ECS"
[2]: https://docs.spring.io/spring-modulith/reference/fundamentals.html?utm_source=chatgpt.com "Fundamentals :: Spring Modulith"
[3]: https://doc.rust-lang.org/reference/visibility-and-privacy.html?highlight=pub&utm_source=chatgpt.com "Visibility and privacy - The Rust Reference"
[4]: https://doc.rust-lang.org/error_codes/E0116.html "E0116 - Error codes index"
[5]: https://alistair.cockburn.us/hexagonal-architecture?utm_source=chatgpt.com "hexagonal-architecture - Alistair Cockburn"

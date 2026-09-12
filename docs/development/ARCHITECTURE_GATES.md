# Production architecture gates

> Status: Current
> Scope: Rust/Frontend production source discovery、layer classification、dependency policy 和 semantic architecture checks
> Canonical owners: gate 源码与测试拥有 exact policy；本文解释模型和修改方式
> Update when: source discovery、layer taxonomy、origin resolution、policy row 或 architecture test entry 改变时

YssBI 的 architecture gate 是 test-owned fitness function，不是 production runtime。总架构只说明依赖方向；本文件说明门禁如何发现并验证真实 production graph。

## 1. Gate locations

| Gate                             | Owner                                                                                   |
| -------------------------------- | --------------------------------------------------------------------------------------- |
| Rust production architecture     | `src-tauri/src/architecture_tests/`                                                     |
| Frontend dependency architecture | `src/tests/architecture/frontendArchitecture.test.ts` 及相邻 model/policy/audit modules |
| Frontend semantic boundaries     | `src/tests/architecture/frontendSemanticArchitecture.test.ts` 及相邻 audit modules      |
| Frontend state authority         | `src/tests/architecture/frontendStateAuthority.test.ts` 及 authority manifest/audit     |
| Documentation contract           | `src/tests/architecture/documentationContract.test.ts`                                  |

Production modules 不导入 classifier、policy、debt 或 test fixtures。门禁从 repository snapshot 读取事实并 fail closed。

图表包的依赖与 SVG 生命周期检查位于 `src/shared/charts/chartArchitecture.test.ts`，复用共享 TypeScript project、production source discovery 和 AST/依赖解析工具。图表专属边界与 ResizeObserver 归属仍由该测试验证。

## 2. Production discovery

### Rust

Rust audit 从 Cargo metadata 发现 workspace 中的 library、binary、runnable example 和 custom-build roots，排除 test/bench targets，再沿每个 root 的真实 `mod` graph 收集 reachable production source。

Cargo 成员和直接依赖断言读取解析后的 package identity、依赖种类与 workspace authority，避免把路径、版本字段的文本格式当成依赖关系。Julia/Bayes 专属 crates 位于插件目录，协议、SDK 和通用库保留原路径；它们均由同一 workspace 的 metadata 发现并执行层级检查。插件的本地依赖图只允许插件内部 crates、协议/SDK 及明确的纯通用库，禁止经过适配器间接引入宿主 SCI、项目或数据库实现。

AST discovery 覆盖 use/re-export/path/macro/include/attribute、`#[path]` 和 cfg reachability。Custom build root 与其 local modules 单独分类，不能借普通 crate layer 获得依赖权限。

### Frontend

Frontend audit inventory 完整 `src/` production tree，排除 `src/tests/`、test files、generated declarations 和明确 fixture。TypeScript module dependencies 与 repository stylesheet dependencies 进入同一个 dependency graph；relative CSS、`@import` 和 `url(...)` target 必须解析为存在的 repository asset 或允许的 exact external style target。

插件网页使用自己的 `sdk.ts`。宿主前端不导入插件网页源码，也不通过额外 npm 包共享插件内部实现。

参与运行的 generated modules 与 JSON imports 同样进入 discovery/classification；只有 declaration/fixture 被排除，生成文件名不是绕过生产依赖审计的依据。

Discovery 不能依赖一份手写“应该存在的文件”清单。新增 production source 若未被分类，门禁必须失败。

## 3. Exact layer classification

分类采用 closed membership，不使用 rule priority。每个 production source 必须命中且只命中一层；zero 或 multiple membership 都是 hard failure。

Rust 当前 taxonomy：

```text
Composition Root
Build Script
Commands
Platform Adapter
Application
Project
Graph
Execution
SCI Core
Database Core
Backend Adapter
Built-in Composition
Transport
Logging
Diagnostics
Pure Leaf
```

Frontend 当前 taxonomy：

```text
App Composition
Views
Application
Core
Domain
Services
Components / Shared UI
Wire Schema
Diagnostics
Pure Shared
```

这些名称是 gate policy vocabulary，不是要求每个 crate 或目录各自写一份 README。一个 Cargo package 可能包含由 source-level policy 精确判断的不同 root；不要在 `MODULE_MAP.md` 手工复制分类。

## 4. Canonical origin resolution

Dependency 在应用 allow/deny policy 前先解析到 canonical origin。

Rust origin 只能是 repository declaration、repository asset、language builtin 或 external Cargo dependency。Workspace member alias 必须解析到 member library/re-export graph，不能伪装成 external package；Cargo declaration 还需匹配 package/alias、runtime/build/dev scope 和 target condition。

Frontend origin 只能是 repository declaration、repository asset 或 external package。Alias、barrel 和 re-export 要解析到真实 declaration；type-only/runtime、module/stylesheet resource kind 和 external package subpath 分别审计。Development dependency 不自动授权 production import。

Frontend resolver 在同一个 `TypeScriptAuditProject` 快照内复用完整项目路径索引和每个 source 的成功解析结果，供 dependency/semantic audits 共用。缓存绑定快照上下文及其 source root，不跨新快照或 isolated fixture 复用；解析失败不写入结果缓存，也不改变原有 fail-closed 规则。

Missing、escaping、remote、non-literal、cyclic 或未登记 target 都 fail closed。finding identity 使用 stable rule ID、repository-relative source、owner、dependency kind 和 canonical target；line/column 只用于诊断。

## 5. Policy and semantic checks

Layer policy 只允许显式 dependency direction/capability。除 import graph 外，semantic checks 保护难以仅靠目录表达的 contract，例如：

- canonical Tauri command registry、thin command 和 transport error shape；
- domain/application DTO 或 framework leakage；
- Build Script 的 exact call surface；
- Application/Project/Graph/Execution/SCI/Database/adapter purpose limits；
- frontend raw invoke/dialog consumers；
- projection write ownership 和 View-to-Core read capability；
- root/nested Dockview constructor ownership；
- stable symbol/variant/field contract。

优先使用 AST、type resolution 和可执行 behavior seam。只有无法在该层表达的窄 contract 才使用 source-token guard；不要用大范围字符串扫描永久证明一次历史删除。

当前 gate 不保留 debt exemption list。真实 finding 直接失败；如果 policy 与目标架构需要共同改变，在同一变更中修改 implementation、policy、focused regression 和当前架构文档。

`faer` 的外部依赖声明限定于 `yss-linalg`、`yss-sci`、`yss-sci-runtime` 和当前构造假设检验输入的 `yss-application`；生产使用限定于 Pure Leaf、SCI Core 和 Application 分类。Graph、Execution、Transport 和其他适配层通过业务契约获取科学计算能力，不直接使用 faer。原生矩阵与向量可以在科学计算实现中共享；项目数值错误、分解检查和秩阈值由 [`yss-linalg` README](../../src-tauri/crates/yss-linalg/README.md) 维护。

`walkdir` 的运行时直接依赖声明限定于 `yss-project-registry`，生产使用归 Project 分类。它只承担私有 `discovery` 模块的目录遍历；根目录校验、目录排除、元数据识别、取消及错误语义仍由项目层拥有。根路径与子目录均不跟随符号链接，重解析点判断复用 `yss-project-filesystem`；这不开放其他层直接使用遍历实现的权限。项目名称规则由 `yss-project-model` 统一拥有，项目运行时不依赖注册或扫描实现。

`yss-file-replace` 按 Platform Adapter 分类。Application 的数据库导出模块和 Julia worker 的 assets 模块仅获 `atomic_replace` 的精确调用权限；这不会开放 Application/Backend Adapter 对整个平台层的依赖。
原生窗口几何改由官方 `tauri-plugin-window-state` 提供，运行时直接依赖仅声明于 `yssbi`，生产使用仅开放给 Composition Root。
自有窗口状态 crate、窗口命令及其依赖 capability 已移除；这不开放领域层使用 Tauri 插件的权限。

执行 command 的精确 capability 包含识别 terminal event 和映射安全错误码所需的 enum variants；execution DTO 的 capability 包含映射结构化运行失败所需的类型。权限绑定到对应 source、owner 和 canonical target，wire 契约由 [`yss-api` README](../../src-tauri/crates/yss-api/README.md#error-contract) 维护。

Graph mutation DTO 可映射 `SetConfiguration`、`SetConstant` 和 `InsertConstantReference`。`yss-graph-document` 按 Pure Leaf 分类，常量定义及只读校验归属该层；Project 可校验持久化数据，不依赖 Graph 编辑或分析层。JSON 门禁仅允许 `model.rs` 的值类型别名，以及 `constant_value.rs` 解析常量字面量所需的精确 `serde_json` 操作，不开放其他 JSON 业务逻辑。

科学计算端口和 OLS 配置按 Pure Leaf 归属 `yss-sci-contract`；runtime 的 service 实现按 SCI Core 分类，依赖中性契约与模型。Composition root 只获 runtime 构造器的精确调用权限；Execution 不依赖 SCI runtime 或模型实现。行为契约见 [Graph 与 Execution](../architecture/GRAPH_AND_EXECUTION.md)。

Activity panel command 的 capability 只开放 Project/Nodes/Commands/Plugins 的 Application 文档查询、Plugin Manager 只读列表与
Activity DTO，以及按游标返回增量的传输缓存。Composition root 只获缓存的构造权限，
缓存按 Transport 分类，既不写入项目也不拥有 UI 状态。Transport mapper 只开放 Activity projection 类型及其固定 variants；
不增加 Commands → Application 或 Transport → Application 的通配依赖。
Project query 的精确 capability 还允许返回与同次 ProjectIndex 对应的面板增量；Project 文档由后端纯投影生成。
前端资源与面板共用已有发布入口，不从 ResourceStore 再次生成或查询 Project 文档。
文档与 UI 状态边界由 [Workbench](../architecture/WORKBENCH_DOCKVIEW_ARCHITECTURE.md) 维护。

示例目录沿用 database Application owner。Composition root 仅获 `SampleCatalog::new`
的构造权限；DataFrame commands 只获目录、导入失败类型和示例 DTO 的明确映射权限；
database transport mapper 仅获 `SampleDataset` 的读取权限。示例数据仍通过标准
Database import、DatasetStore 和 Project publication 进入项目，不开放新的跨层通配边。
Application 的 `serde` 直接依赖用于静态目录和离线源定义的反序列化，外部依赖声明清单与 Cargo 保持一致。

`yss-tabular-arrow` 的 `chrono` 依赖用于将外部带时区的时间转换为保留钟面的无时区值；
这属于 Database Core 的类型适配职责。Project 与 Logging 自己生成无时区的展示时间，
不依赖 tabular adapter 作为时钟服务。

## 6. Changing the architecture policy

React Flow 的运行时、类型与基础样式依赖仅开放给 Views；Application、Core 和 Domain
不导入该画布库。graph-editor 的局部样式按确切 consumer/asset 路径登记，视图只获
Graph read snapshot 和既有 viewport 契约，不增加 projection 写权限。

数据库的语义 Schema/revision facts 归 `yss-database-schema`，按 Pure Leaf 分类；具体引擎映射归适配器。
`yss-relational-contract` 是执行期关系句柄、快照绑定和 Arrow 流端口的 Pure Leaf 契约。
`yss-tabular-arrow` 与 `yss-datafusion` 按 Database Core 分类，Arrow/DataFusion 外部依赖只授予实际声明的包；
持久化 Graph 和统计数值算法不直接依赖 DataFusion。IPC/CSV/Parquet 宿主 I/O 不再获 Polars 物化权限。

`yss-dataset-profile` 只拥有中性的统计 DTO 与显示规则，DataFusion 适配器在固定快照上执行聚合。
SCI runtime 的输入准备使用 Arrow；其 Polars/Polars Arrow 使用权限已移除，数值核心不增加数据引擎依赖。
profile 的行为由真实快照回归覆盖，不再用断言源码包含 Polars 代码的历史迁移测试固定旧实现。
旧 DuckDB/Polars 适配器及其依赖权限已删除。`yss-bayes-artifact-datafusion` 按 Backend Adapter 审计，
只为该包声明 DataFusion 查询依赖；Julia exchange 的两个消费者使用 Arrow。密度计算权限绑定到该适配器的 `plots` 模块。
独立 `dataset_engine_bench` example 与桌面 composition root 可装配 `SciRuntimeBackend`；
其他 Application 源码继续通过 `ScientificBackend` 端口，不能引用该实现。

`yss-dataset-store` 按 Database Core 分类，只拥有数据集目录和文件提交，不接管 Project 文档或 publication authority。
结果页 command 只调用 Application 查询；页面预算、快照读取和查询结束后的 currentness 检查位于 Application。
其 DTO converter 仅获 ResultPageProjection/ResultPageKind 的精确读取权限。
结果租约 commands 只获 Application state、结果快照及引用 DTO 的登记权限；报告 commands 只获报告查询错误和相关 DTO。
`schema/result.rs` 只映射结果引用、表和分析投影，不能访问 ApplicationState；假设检验输出的转换归 statistics schema。
这些 capability 绑定到具体 source 和 symbol，不开放 Commands/Transport 对 Application 的通配依赖。
另存为枚举文件路径后使用现有文件事务的流式复制入口；目标必须不存在，源根必须在租约中，文档仍逐项验证。

插件协议与清单属于 Pure Leaf；通用进程/签名安装和 IPC SDK 属于 Backend Adapter。`yss-bayes-runtime` 属于插件内部 Application，只有 Julia extension 的确切 adapter source 可访问其编排入口。宿主 composition root 只构造通用 Plugin Manager 和 HostServices；Julia adapter 构造器只允许出现在外部 extension 中。文件发布、Arrow 适配和签名库权限均为对应 source/package 的显式登记，不开放通配业务桥接。

1. 在 [Change Process](CHANGE_PROCESS.md) 中明确 owner、依赖理由和 acceptance criteria。
2. 用 isolated fixture 证明新 rule 能检测目标 violation，且 finding identity 稳定。
3. 修改 real production policy/classification，并保持 every-source-exactly-once。
4. 运行真实 repository audit，确认没有用 broad allowlist 掩盖其他 dependency。
5. 若顶层 direction 或 authority 改变，同步更新[系统架构](../architecture/ARCHITECTURE.md)；若只是检查实现改变，只更新本文。
6. 通过 [Local Workflow](LOCAL_WORKFLOW.md) 中的 Architecture policy 验证范围交付。

## 7. Documentation contract

架构文档可为 `Current`，或明确声明 `Contract: Target Architecture` 的 `Accepted Decision`。前者描述生产事实，后者定义已接受的实现契约，二者都必须被文档索引收录。开发工作流文档仍必须为 `Current`；目标架构不能通过状态标签被误当作生产完成证据。

Documentation contract 是轻量 Vitest gate，保护机器可验证的漂移：

- `docs/architecture/` 和 `docs/development/` 中的 Current 文档都被 `docs/README.md` 索引；
- 维护中文档的相对链接和明确 source path 存在；
- 文档中的 root `pnpm` 命令对应 `package.json` script 或 package-manager builtin；
- `AGENTS.md`、`CLAUDE.md` 和 `GEMINI.md` 都指向 `.rules`；
- `docs/version/` 只包含 Historical 文档；
- generated `docs/reference/MODULE_MAP.md` 与 Cargo metadata/目录一致。

该 gate 不把 prose 内容、未来 roadmap 或历史源码路径当作可执行事实，也不尝试建设复杂文档平台。

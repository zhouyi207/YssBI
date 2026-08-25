# Project → Application 反向依赖清理设计

日期：2026-08-25

状态：已确认

## 1. 背景

YssBI 后端的规范依赖方向是：

```text
commands → application
              ├─→ project ─→ database
              │      └─────→ node_system
              ├─→ database
              ├─→ node_system
              └─→ sci
```

`application` 编排跨模块用例，`project` 拥有项目 session、resource revision、事务提交和 publication authority，`database` 拥有 DuckDB storage 与 runtime database semantics。

当前生产代码仍存在 `project → application` 反向依赖：

- project activation 调用 Application 中的 DuckDB runtime binding helper；
- database delete transaction 调用 Application 中的 DuckDB table removal helper；
- project projection 调用 Application 中的 `ColumnInfoDTO` conversion helper。

Application 同时依赖 ProjectState，因此这些调用形成真实的双向依赖。测试代码中也有 Project 测试通过 Application use case 布置 fixture；这些调用只在 `#[cfg(test)]` 下可达，不属于生产依赖。

## 2. 目标

- 清除所有生产可达的 `project → application` Rust 依赖。
- 将四个 concrete helper 移到已有职责的 owner，不新增 service trait 或 facade。
- 保持函数签名、控制流、锁与 I/O 顺序、schema 表示、compiler fingerprint、IPC DTO、错误 wire 和用户可见行为不变。
- 增加可执行的架构守卫，禁止生产 Project 再次依赖 Application。
- 更新当前架构文档，使依赖方向和 database/schema ownership 与实现一致。

## 3. 非目标

- 不将 compiler/project 当前使用的 `ColumnInfoDTO` 替换为 `DataSchema`。
- 不重构 project activation、database delete 或 projection workflow。
- 不处理 `Project ↔ Node System`、Tauri framework leakage、Execution UI intent 或前端依赖环。
- 不拆独立 crate，不新增 dependency-inversion trait，也不保留旧 helper 的 compatibility re-export。
- 不要求测试模块停止使用 Application use case；架构规则只约束生产可达模块。
- 不修改现有 IPC contract、project serialization 或 database persistence format。

## 4. 模块归属

### 4.1 Database project storage

新增 `src-tauri/src/database/project_storage.rs`，只负责 persisted database resource 与 session runtime storage 之间的具体衔接：

- `bind_duckdb_instance(decl, project_root)`：根据 `DatabaseDecl` 和项目根路径读取 DuckDB metadata，构造 `DatabaseInstance`；
- `remove_duckdb_table_if_needed(engine, project_root)`：根据 `DatabaseEngine` 删除对应 DuckDB table 及其 metadata。

该模块可以依赖 Database 自己的 declaration、engine、state、metadata reader 和 edit history，但不得依赖 ProjectState、Application、commands、events 或 Tauri。

函数从 `application/database.rs` 直接迁移，名称、可见性、参数、返回值和分支保持不变。`database/mod.rs` 公开其 crate-internal callers 所需的 symbol，不保留 Application 路径下的 alias。

### 4.2 Wire schema conversion

`src-tauri/src/schema/database.rs` 已拥有 `ColumnInfoDTO`，并已负责 Database domain type 与 wire DTO 的映射。因此以下 converter 移入该文件：

- `column_info_from_schema(&polars::prelude::Schema)`；
- `column_info_from_duckdb(&[DuckDbColumnMeta])`。

converter 必须保持输入列顺序，并继续输出当前 Polars raw dtype 字符串或 DuckDB metadata dtype 字符串。

它们不放入 `database/database_schema.rs`，因为返回值是 wire DTO；让 Database module 返回 `ColumnInfoDTO` 会新增 `database → wire schema` 依赖。

### 4.3 Application

`application/database.rs` 保留 database import/read/edit/rename/save/export 等用例编排，只改为从 `crate::database` 和 `crate::schema` 使用迁移后的 helpers。

`application/database_schema.rs` 保留以下 application-facing projection 职责：

- 从 `DatabaseInstance` 提取当前 DTO snapshot；
- 将 snapshot 应用到 `DatabaseDeclDTO`；
- 从 coherent `ProjectResourceSnapshot` 组合 database/variable query DTO。

底层 Polars/DuckDB metadata 到 `ColumnInfoDTO` 的转换不再由 Application 拥有。

### 4.4 Project

Project 的 production call sites 改为：

```text
project_activation          → database::bind_duckdb_instance
project_state_database      → database::remove_duckdb_table_if_needed
project_state::projection   → schema::{column_info_from_schema,
                                      column_info_from_duckdb}
```

`project_state::compile_resources` 中位于 `#[cfg(test)]` 的 helper path 同步机械更新，但不扩大生产 scope。

## 5. 数据流与行为不变量

### 5.1 Project activation

Activation 继续枚举 `ProjectData.databases`，并为 DuckDB declaration 重建 session-bound `DatabaseInstance`：

- 有项目根路径且 metadata 读取成功时，保留绝对 DuckDB path、table、row count、columns 和新的空 `EditHistory`；
- 项目根路径缺失时，仍构造 `DatabaseState::Failed`，不让整个项目激活失败；
- metadata 读取失败时，仍降级为 `DatabaseState::Failed`；
- 非 DuckDB declaration 仍由现有 caller branch 处理；迁移后的 binding helper 保留当前 `unreachable!` contract。

### 5.2 Database deletion

删除事务顺序保持：

```text
reserve operation
  → acquire database write lease
  → capture revisioned database snapshot
  → delete DuckDB table outside authority lock
  → commit database declaration removal
  → publish existing mutation receipt
```

本批不得把磁盘 I/O 移入 `commit_database_delete` 的锁区。非 DuckDB engine 或无项目根路径仍返回成功 no-op。物理删除继续调用现有 `drop_data_table`，以同时清理 table metadata。

### 5.3 Projection and schema

Project projection 仅替换 converter 的模块路径：

- 已驻留 database schema 仍在既有短锁 snapshot 中复制；
- 未驻留 DuckDB metadata 仍在锁外读取；
- metadata 读取后的 authority generation/currentness validation 不变；
- `DatabaseState::Failed` 的既有磁盘 fallback 行为不变；
- `ColumnInfoDTO` 的列顺序和 dtype 字符串不变。

因此 compiler resource fingerprint、schema invalidation 与 execution behavior 均不应变化。

### 5.4 Errors and transport

本批不新增错误类型，不改变错误映射，也不改变 exact `{ code, details, incidentId }` wire。现有 Application result DTO、project events、run events 和 serialization contract 均保持原样。

## 6. Architecture guard

新增 crate-level、仅在 Rust unit-test build 中编译的 `architecture_tests/dependency_audit`。它沿用现有 `node_system/testing/source_audit` 的 `syn` 审计方式，但不把跨模块架构规则放入 Node System 的测试命名空间。审计从 `src/project/mod.rs` 开始遍历真实的生产模块图，而不是递归扫描 `src/project` 目录。

审计器必须：

- 支持 `foo.rs`、`foo/mod.rs` 和 `#[path = "..."] mod foo`；
- 跳过 exclusively-test 的 item、inline module 和 external module；
- 将 `#[cfg(all(test, ...))]` 视为 test-only；
- 不把 `#[cfg(any(test, feature = "..."))]` 误判为 test-only；
- 检查普通/grouped/aliased `use`，以及 expression、type 和其他生产 `syn::Path`；
- 在路径 segment 精确等于 `application` 时报告违规；
- 对 production source parse error 或无法解析的 module declaration fail closed；
- 规范化、排序并去重违规文件，保证 Windows 输出稳定；
- 不使用永久 allowlist。

只有两个新测试：

1. `project_dependency_audit_respects_production_module_reachability`：fixture 验证 import/path 形式、cfg 语义和 `#[path]` module resolution；其独立失败模式是审计器漏报或误报。
2. `production_project_modules_do_not_depend_on_application`：对真实 Project production module graph 执行规则；其独立失败模式是架构边界回归。

架构测试应先在迁移前失败并准确指出当前生产反向引用，迁移后通过。

## 7. 文档更新

实现完成后更新：

- `docs/architecture/ARCHITECTURE.md`
  - 明确 Project 可以依赖 Database/Node System，但不得依赖 Application/commands；
  - 在顶层目录表记录 `src-tauri/src/schema/` 的 wire DTO 职责；
  - 将 `application::database_schema` 描述收窄为 coherent snapshot 到 query/presentation DTO 的组合。
- `src-tauri/src/database/README.md`
  - 记录 `project_storage.rs` 的 runtime binding/physical removal 职责；
  - 补充 database domain schema 与 wire schema conversion 的边界；
  - 更新 module map。

文档只描述本批完成后的真实实现，不提前宣称 Project/compiler 已改用 transport-neutral schema。

## 8. 验证

实施遵循 test-first 顺序：

1. 添加架构审计 fixture 与真实规则测试。
2. 运行真实规则并确认它因现有生产反向边失败。
3. 机械迁移四个 helpers 和所有调用路径。
4. 重跑两个架构测试。
5. 运行覆盖 project activation、database deletion 和 projection 的现有 focused tests。
6. 运行：

```text
pnpm rust:check
pnpm verify:rust
git diff --check
```

`pnpm verify:rust` 只执行 Rust format/compile checks，不替代 focused Rust tests。除非 focused coverage 无法覆盖风险或出现跨域失败，本批不运行完整 `pnpm rust:test`。

## 9. 交付边界

当前工作树已有用户的前端 editor/drag-drop 改动。本批只修改上述 Rust、架构文档和测试文件；提交时逐文件暂存，不纳入任何现有前端改动。

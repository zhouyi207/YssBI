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

## 2. Production discovery

### Rust

Rust audit 从 Cargo metadata 发现 workspace 中的 library、binary、runnable example 和 custom-build roots，排除 test/bench targets，再沿每个 root 的真实 `mod` graph 收集 reachable production source。

AST discovery 覆盖 use/re-export/path/macro/include/attribute、`#[path]` 和 cfg reachability。Custom build root 与其 local modules 单独分类，不能借普通 crate layer 获得依赖权限。

### Frontend

Frontend audit inventory 完整 `src/` production tree，排除 `src/tests/`、test files、generated declarations 和明确 fixture。TypeScript module dependencies 与 repository stylesheet dependencies 进入同一个 dependency graph；relative CSS、`@import` 和 `url(...)` target 必须解析为存在的 repository asset 或允许的 exact external style target。

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

## 6. Changing the architecture policy

1. 在 [Change Process](CHANGE_PROCESS.md) 中明确 owner、依赖理由和 acceptance criteria。
2. 用 isolated fixture 证明新 rule 能检测目标 violation，且 finding identity 稳定。
3. 修改 real production policy/classification，并保持 every-source-exactly-once。
4. 运行真实 repository audit，确认没有用 broad allowlist 掩盖其他 dependency。
5. 若顶层 direction 或 authority 改变，同步更新[系统架构](../architecture/ARCHITECTURE.md)；若只是检查实现改变，只更新本文。
6. 通过 [Local Workflow](LOCAL_WORKFLOW.md) 中的 Architecture policy 验证范围交付。

## 7. Documentation contract

Documentation contract 是轻量 Vitest gate，保护机器可验证的漂移：

- `docs/architecture/` 和 `docs/development/` 中的 Current 文档都被 `docs/README.md` 索引；
- 维护中文档的相对链接和明确 source path 存在；
- 文档中的 root `pnpm` 命令对应 `package.json` script 或 package-manager builtin；
- `AGENTS.md`、`CLAUDE.md` 和 `GEMINI.md` 都指向 `.rules`；
- `docs/version/` 只包含 Historical 文档；
- generated `docs/reference/MODULE_MAP.md` 与 Cargo metadata/目录一致。

该 gate 不把 prose 内容、未来 roadmap 或历史源码路径当作可执行事实，也不尝试建设复杂文档平台。

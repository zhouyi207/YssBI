# Julia 计算插件

Julia/Bayes 的可选安装单元，包含独立原生程序与独立网页。宿主不链接这些业务 crates，也不导入这里的前端源文件。共享 Cargo workspace 只用于源码维护和构建；发布物不需要用户安装 Rust、Node.js 或访问本仓库。

## 构建与安装

在仓库根目录执行：

```sh
pnpm plugin:julia:package
```

输出在 `target/plugin-packages/`。发布版本由本目录的 `plugin.json` 显式维护；相同发布版本不能覆盖不同内容。
本插件的构建入口调用 `plugins/scripts/package-plugin.mjs` 完成通用签名与封装；打包规则测试也位于该目录。
需要优化构建时使用 `pnpm plugin:julia:package --release`；`--skip-build` 只封装已有产物。
开发迭代使用 `--dev`，生成带时间顺序和完整清单/资产摘要的预发布版本。`packageDigest` 始终标识完整签名载荷，
版本中的 build metadata 不用于新旧排序，遵循 [SemVer](https://semver.org/)。

在 YssBI 左下角「插件」中选择「从文件安装插件包」，打开生成或下载的 `.yssplugin`，检查发布者、签名指纹、权限与原生代码信任提示后安装。安装后会出现 Julia sidebar 入口；依赖准备是单独的显式操作，不在启动宿主、安装包或打开页面时自动执行。

当前插件复用用户已安装的兼容 Julia（版本约束以 `plugins/julia/runtime/julia/Project.toml` 为准）。依赖项目、首选 depot 和产物写入插件私有目录；用户原有 Julia/depot 不因禁用或卸载而删除。缺少 Julia 时页面显示依赖缺失，不伪装成插件未安装。

## 签名与信任

默认生成只用于本机开发的 Ed25519 密钥，保存在忽略的构建目录。发布时通过 `YSSBI_PLUGIN_SIGNING_KEY` 指定受控的 PKCS#8 PEM 密钥路径；不要发布私钥，不要把开发密钥当成可信发布者身份。清单、完整文件索引和内容哈希共同签名，宿主在安装、启动和资源读取时验证内容。

当前执行配置为 Windows x64 `trustedNative`：用户明确授权运行原生代码。这不是 OS sandbox；声明必须沙箱的包会被拒绝，不会降级。网页由宿主 CSP、sandbox iframe 和仅绑定当前页面的 MessagePort 限制，不能直接调用 Tauri。

安装后保留签名身份绑定；更换签名者需要单独确认旧、新指纹。卸载不会清除此绑定。
本版本使用插件协议 2、共享 SDK 0.2，要求宿主支持该协议；旧协议包需重新构建，不能混用请求语义。

## 职责与数据

- `yss-julia-extension`：插件 IPC adapter 和进程入口。
- `yss-bayes-runtime`：插件内部 Bayes 编排，依赖原有模型、worker 和 artifact crates，不是 YssBI 的业务 bridge。
- `yss-bayes-artifact-datafusion`：用 DataFusion 查询 Julia Arrow 产物，负责样本筛选、分页、CSV 批流导出和图表数据投影。
- `web/`：独立 React、图表、模型编辑器和桥接适配；不依赖宿主 React 实例。
- 宿主：通用安装 registry、进程监督、任务账本、项目上下文、数据快照授权和结果提交。

项目数据通过有界 Arrow 文件快照传入插件，不穿过网页。任务使用 operation ID 去重，关闭页面不终止计算；取消意图不会被迟到状态覆盖。进程异常后的未知结果不会自动重试。结果以 JSON、CSV 摘要及原始 artifacts 提交到项目 `extension-results/`，包含来源和内容哈希，禁用/卸载不删除已有结果。

插件与宿主统一使用 Arrow 数据边界，workspace 不再依赖 Polars。输入保留跨批次的行对齐、类别标签和缺失值；
Julia worker 分批写出 Float64/Utf8 交换文件，交换表模式与 Julia 模型不变。
插件直接使用 Arrow 读写交换文件，写入时显式使用 8 字节对齐，兼容 Julia Arrow 对首条消息位置的读取要求。
后验查询共用受控的 DataFusion 内存池，保持文件行顺序；分页前检查整个产物的必要列，分页之外的坏行也会失败。
CSV 导出消费批流，图表输入另有内存预算；密度估计和自相关由插件完成，保持原有数值规则。

环境准备和推理互斥，共用现有 Julia worker。取消环境准备会终止对应的准备/worker 子进程；
宿主需要终止整个插件时，会将同一进程中受影响的任务统一标记为未知结果。
Bayes 页面保留真实进度，明确区分失败与结果未知，并提供可见的超时选项。
传输失败后的“重试同一提交”复用操作标识和已捕获输入；“开始新任务”才创建新操作。

## 源码与构建边界

同一仓库继续发布一个 `yssbi.julia` 包：

```text
plugins/julia/
  plugin.json          源清单与发布版本
  native/crates/       Julia/Bayes 专属 Rust 实现
  runtime/julia/       Julia 项目、锁文件和计算脚本
  web/                 插件前端
  scripts/             插件构建与兼容性验证入口
  tests/               插件系统测试和 Bayes fixtures
src-tauri/crates/
  yss-plugin-protocol/  现有公共协议
  yss-plugin-sdk/       现有 Rust 通信 SDK
  yss-plugin-runtime/   宿主安装与任务管理
  ...                   宿主实现和已有通用基础库
```

网页通信实现保留在 `web/src/sdk.ts`，由本插件的页面直接复用，不单独发布 npm 包。
协议和 Rust SDK 保留原 crate 名称与路径。插件还复用 `yss-math-expr`、`yss-sci-contract` 和
`yss-file-replace`：它们分别提供纯数学解析、数据值/取消契约和平台文件替换，不拥有宿主业务状态。
插件不依赖宿主的 SCI 算法运行时、项目模型、数据库适配器或数据库连接；数据通过协议授权的 Arrow 文件交换。

Cargo workspace 和原有根命令继续使用 `src-tauri/Cargo.toml`，通过显式成员包含插件 crates，
构建输出仍在 `src-tauri/target`。当前在本仓库内选择插件构建、签名与发布，继续交付一个安装包。
公共 SDK 独立发布、脱离宿主仓库的源码分发和拆仓按实际复用需求另行考虑。

## 验证与能力边界

```sh
pnpm check:plugin:julia
pnpm test:plugin:julia
pnpm test:plugin:package
pnpm test:rs:package -p yss-plugin-runtime --test installation
pnpm test:plugin:native
```

原生测试使用本地最新包和真实 Julia，冷环境可能需要数分钟。宿主侧生成的投影契约使用 `pnpm generate:plugins:check` 检查。
历史、预算、缓存、签名身份和诊断的当前宿主行为见 [Plugin runtime](../../src-tauri/crates/yss-plugin-runtime/README.md)。

[Plugin 架构](../../docs/architecture/PLUGIN.md) 是完整目标契约。当前发行能力是签名本地包、通用受监督任务和隔离网页；在线目录/自动更新、OS sandbox、声明式标准 UI provider、动态 Graph compute provider、完整任务 checkpoint/recover 与依赖自动下载尚未开放，未知必需能力直接拒绝。目录服务未接入时不制造可安装市场条目。

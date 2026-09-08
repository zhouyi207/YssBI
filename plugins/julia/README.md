# Julia 计算插件

Julia/Bayes 的可选安装单元，包含独立原生程序与独立网页。宿主不链接这些业务 crates，也不导入这里的前端源文件。共享 Cargo workspace 只用于源码维护和构建；发布物不需要用户安装 Rust、Node.js 或访问本仓库。

## 构建与安装

在仓库根目录执行：

```sh
pnpm plugin:julia:package
```

输出在 `target/plugin-packages/`。需要优化构建时使用 `pnpm plugin:julia:package --release`；`--skip-build` 只重新封装已经构建的对应 profile 产物。包名包含代码与网页内容摘要，变更内容不会冒用同一个版本。

在 YssBI 左下角「插件」中选择「从文件安装插件包」，打开生成或下载的 `.yssplugin`，检查发布者、签名指纹、权限与原生代码信任提示后安装。安装后会出现 Julia sidebar 入口；依赖准备是单独的显式操作，不在启动宿主、安装包或打开页面时自动执行。

当前插件复用用户已安装的兼容 Julia（版本约束以 `src-tauri/julia/Project.toml` 为准）。依赖项目、首选 depot 和产物写入插件私有目录；用户原有 Julia/depot 不因禁用或卸载而删除。缺少 Julia 时页面显示依赖缺失，不伪装成插件未安装。

## 签名与信任

默认生成只用于本机开发的 Ed25519 密钥，保存在忽略的构建目录。发布时通过 `YSSBI_PLUGIN_SIGNING_KEY` 指定受控的 PKCS#8 PEM 密钥路径；不要发布私钥，不要把开发密钥当成可信发布者身份。清单、完整文件索引和内容哈希共同签名，宿主在安装、启动和资源读取时验证内容。

当前执行配置为 Windows x64 `trustedNative`：用户明确授权运行原生代码。这不是 OS sandbox；声明必须沙箱的包会被拒绝，不会降级。网页由宿主 CSP、sandbox iframe 和仅绑定当前页面的 MessagePort 限制，不能直接调用 Tauri。

## 职责与数据

- `yss-julia-extension`：插件 IPC adapter 和进程入口。
- `yss-bayes-runtime`：插件内部 Bayes 编排，依赖原有模型、worker 和 artifact crates，不是 YssBI 的业务 bridge。
- `web/`：独立 React、图表、模型编辑器和桥接适配；不依赖宿主 React 实例。
- 宿主：通用安装 registry、进程监督、任务账本、项目上下文、数据快照授权和结果提交。

项目数据通过有界 Arrow 文件快照传入插件，不穿过网页。任务使用 operation ID 去重，关闭页面不终止计算；取消意图不会被迟到状态覆盖。进程异常后的未知结果不会自动重试。结果以 JSON、CSV 摘要及原始 artifacts 提交到项目 `extension-results/`，包含来源和内容哈希，禁用/卸载不删除已有结果。

## 验证与能力边界

```sh
pnpm check:plugin:julia
pnpm test:plugin:julia
pnpm test:rs:package -p yss-plugin-runtime --test installation
pnpm test:plugin:native
```

原生测试使用本地最新包和真实 Julia，冷环境可能需要数分钟。宿主侧生成的投影契约使用 `pnpm generate:plugins:check` 检查。

[Plugin 架构](../../docs/architecture/PLUGIN.md) 是完整目标契约。当前发行能力是签名本地包、通用受监督任务和隔离网页；在线目录/自动更新、OS sandbox、声明式标准 UI provider、动态 Graph compute provider、完整任务 checkpoint/recover 与依赖自动下载尚未开放，未知必需能力直接拒绝。目录服务未接入时不制造可安装市场条目。

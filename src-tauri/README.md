# Rust workspace 与桌面构建

> Status: Current
> Scope: 本仓库的 Cargo workspace、Tauri 构建、按 crate 验证与构建缓存
> Canonical owners: Cargo.toml、tauri.conf.json、根 rust-toolchain.toml 和 package.json 拥有可执行配置
> Update when: workspace、桌面构建钩子或 Rust 验证入口改变时

## Workspace 与输出

[Cargo.toml](Cargo.toml) 同时包含宿主 crates 和 `plugins/julia/native/crates` 中显式登记的插件成员，共用锁文件与根构建入口。Rust 版本和 components 由[根工具链文件](../rust-toolchain.toml)固定。

仓库没有 `.cargo/` target 覆盖；当前 workspace 输出为 `src-tauri/target/`。根 Rust scripts 均显式选择本 workspace manifest，保留 Cargo 默认 build jobs 和 libtest threads。

## 桌面构建

从仓库根目录运行 `pnpm dev` 或 `pnpm build`。[tauri.conf.json](tauri.conf.json) 的开发钩子运行 Vite，构建钩子先检查[示例资源](resources/samples/README.md)，再构建 Vite。钩子不能回调根 `dev` / `build` scripts，否则会递归。

宿主构建不自动构建或捆绑 [Julia 插件](../plugins/julia/README.md)。

## 按 crate 验证

按[根验证规则](../.rules)选择范围，从仓库根目录运行：

```sh
pnpm check:rs:package -p <crate-name> --lib
pnpm check:rs:package -p <crate-name> --tests
pnpm lint:rs:package -p <crate-name> --lib
pnpm test:rs:package -p <crate-name> --lib <test-name>
pnpm test:rs:package -p <crate-name> --test <integration-target> <test-name>
pnpm format:rs:package -p <crate-name>
```

`-p` 可以重复，用于选择受影响的多个 package；`--lib` / `--test` 选择测试目标，测试名只过滤目标内的用例，不会只编译该函数。按消费者需要选择 `--tests`、`--all-targets` 或 features；不要默认追加全部 targets/features。

这些 `:package` 入口不自动添加 workspace 或 all-features 参数。相对地，`check:rs`、`lint:rs`、`test:rs` 是全 workspace 入口；在 `test:rs` 后追加 `-p` 不能将它变成按 crate 检查。

Clippy 保留默认 warning 级别，不统一提升为 error；实际问题仍按影响处理。
`pnpm format:check:rs` 是只读的 workspace 格式检查，不编译或执行测试。局部写入格式化选择 `format:rs:package`，不顺手修改其他包。

## 耗时诊断与维护

Windows 链接可能较慢。先区分构建/链接与用例执行成本；确有需要时对代表性 crate 测量一次：

```sh
pnpm test:rs:package -p <crate-name> --lib --no-run --timings
```

该命令只构建测试程序，不运行用例，不能作为测试通过证据。保留可复用产物并检查不必要的依赖/features；不要靠减少覆盖或全局限制线程来替代范围选择。

只有缓存损坏、依赖切换或需要释放磁盘空间时才使用 `cargo clean --manifest-path src-tauri/Cargo.toml`，这不是日常验证步骤；清理后会重新编译依赖。

业务测试与具体前置条件继续在各 crate README 中维护；源码架构扫描器已移除，依赖方向仍需要按实际声明与调用复核。

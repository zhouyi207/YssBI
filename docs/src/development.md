# Developing YssBI

Build the native desktop application from the repository root. YssBI uses a Rust Cargo workspace with `yss-desktop-gpui` as its default member. The `react/` tree is reference source, not a second application build.

Start with the [Linux instructions](./development/linux.md). Windows and macOS input, window behavior and distribution still require target-platform acceptance; the presence of platform dependencies is not evidence that those workflows work.

## Build and run

Install [rustup](https://rustup.rs/). The repository's [rust-toolchain.toml](https://github.com/zhouyi207/YssBI/blob/main/rust-toolchain.toml) selects the Rust version and development components.

```sh
cargo run
cargo build --release
```

Use the [CLI reference](./reference/cli.md) to open a project or resource from the command line. The first development build also optimizes selected rendering dependencies, so it can take longer than subsequent builds.

For a standalone release binary, stage the sample resources beside the executable:

```sh
cargo run -p yss-application --example build_samples -- --check --stage target/release/resources/samples
```

Distribute the resulting `resources` folder with the binary. See the [sample asset guide](https://github.com/zhouyi207/YssBI/blob/main/resources/samples/README.md) for resource generation and validation. Native installers, signing and updater delivery remain open work.

## Bird's-eye view of YssBI

[Cargo.toml](https://github.com/zhouyi207/YssBI/blob/main/Cargo.toml) owns workspace membership and shared dependencies. These are useful starting points, not a second complete package inventory:

| Area                                       | Source entry                                                                                                 |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------ |
| Desktop windows, workbench and views       | [yss-desktop-gpui](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-desktop-gpui/README.md)           |
| Typed use cases and composition            | [yss-application](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-application/README.md)             |
| Project resources and persistence          | [yss-project](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-project/README.md)                     |
| Graph edits, execution and result delivery | [Application graph](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-application/src/graph/README.md) |
| Graph-independent node invocation          | [yss-node-kernel](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-node-kernel/README.md)             |
| Dataset generations and commits            | [yss-database-store](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-database-store/README.md)       |
| Statistical orchestration                  | [yss-sci-runtime](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-sci-runtime/README.md)             |
| Assistant runtime and tools                | [yss-harness-core](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-harness-core/README.md)           |
| External plugin lifecycle                  | [yss-plugin-runtime](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-plugin-runtime/README.md)       |
| Structured observations                    | [yss-logging](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-logging/README.md)                     |

Read the [architecture guide](./development/architecture.md) for state ownership and dependency direction. Module READMEs describe the detailed interfaces and lifecycle contracts.

## Making a change

Use the repository's [coding rules](https://github.com/zhouyi207/YssBI/blob/main/.rules) and the applicable module rules. Prefer the existing owner of a behavior. Keep API, serialization and persistence changes consistent across callers; this unreleased project does not maintain old-contract migration layers.

Select checks for the affected modules and consumers using [Testing](./development/testing.md). Keep genuine missing capabilities and uncompleted acceptance in [TODO.md](https://github.com/zhouyi207/YssBI/blob/main/TODO.md), rather than treating a successful build as completed product work.

## See also

- [Getting started](./getting-started.md)
- [Performance](./performance.md)
- [Documentation build instructions](https://github.com/zhouyi207/YssBI/blob/main/docs/README.md)

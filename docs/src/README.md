项目使用说明见[文档索引](SUMMARY.md)，文档构建见 [docs/README.md](../README.md)，开放工作见 [TODO](../../TODO.md)。


## 项目架构

桌面前端与业务实现均使用 Rust，以根 Cargo workspace 作为构建入口：

- **GPUI 原生前端**：`yss-desktop-gpui` 仅负责启动与最终窗口组装；`yss-workbench` 协调项目、导航和根 `DockArea`，Graph、Assistant、Data、Results、Settings、Resource、Plugin UI 分别拥有功能视图。
- **应用层**：`yss-application` 提供平台中立的服务与类型化用例；宿主在 worker 中执行阻塞业务，向视图交付类型化回执、事件和只读投影。
- **领域层**：Project、Graph、Database、Execution、SCI 和 Harness 各自拥有业务状态；领域与应用层不依赖 GPUI。
- **基础设施**：文件系统、日志、数据库存储与插件适配按各自模块边界接入。外部计算插件不改变桌面前端的 Rust 架构。
- **本地化**：`yss-i18n` 封装 `rust-i18n` 的文本查询与单次插值，供原生业务文案、节点元数据和图诊断复用；领域查询显式传入语言，不依赖 GPUI 的界面语言状态。桌面组件自身沿用 GPUI Kit 的间接依赖。
- **用户偏好**：`yss-settings` 拥有平台中立的分组类型、默认值、校验和原子持久化；`yss-ui::preferences` 发布已生效快照，各业务视图订阅使用。模型凭据、项目布局和日志历史继续归原服务。
- **桌面共享边界**：`yss-ui` 复用产品呈现与 GPUI Kit，不依赖 Application、Project 或图执行链；不含 GPUI 的 `yss-desktop-services` 共享 Application 后台调用、事件交付和只读资源目录投影。图专属目录行与反馈归 Graph UI。功能 crate 不反向依赖 Workbench，也不接收完整 `NativeServices`。

状态所有权和依赖关系见[系统架构](development/architecture.md)，模块入口见[开发指南](development.md)。

## 快速开始

从仓库根目录运行 Cargo 命令。Rust 版本及开发组件统一由
[rust-toolchain.toml](../../rust-toolchain.toml) 定义，不另行声明最低支持版本。
rustup 会自动选择该版本并安装配置的工具链及组件。根目录是纯 Rust workspace，默认成员为
`yss-desktop-gpui`；启动和构建只使用 Cargo，无需 JavaScript 工具链。

```bash
# 启动 GPUI 工作台
cargo run

# 打开项目及指定图
cargo run -- "/absolute/path/to/project" events/example.yssbi-event

# 构建
cargo build --release

# 为独立运行的可执行文件准备示例资源（将目录与程序一起分发）
cargo run -p yss-application --example build_samples -- --check --stage target/release/resources/samples
```

原生宿主、已实现交互与待验收范围见 [GPUI host](../../crates/yss-desktop-gpui/README.md)。
普通 `cargo run` 对桌面入口、Workbench、共享与功能 UI，以及渲染、曲线细分和布局依赖启用定向开发优化；首次构建这些依赖会更慢。
帧率验收使用 `cargo run --release`，并在相同图规模、窗口大小和显示器刷新率下比较。
示例 Parquet 已随源码提供，开发启动可离线读取；更新来源和资源校验见
[示例资产](../../resources/samples/README.md)。

### 打包与 GitHub 发布

采用 Zed 的原生构建方式，Linux、macOS、Windows 均覆盖 x86_64 / ARM64：
Linux 在 Ubuntu 24.04 原生 runner 上统一生成 `.deb`、`.rpm` 与 `.tar.gz`，
macOS 提供 `.dmg`，Windows 提供安装程序 `.exe` 与便携 `.zip`。
版本来自 workspace，工具链与依赖使用仓库锁定值，示例资源由原有 Rust 工具校验并随包安装。

```bash
# 在与目标系统、架构相符的机器上执行；其他 target 见发布指南
python3 scripts/package.py --target x86_64-unknown-linux-gnu
# Ubuntu 24.04 的全部 Linux 格式；Fedora 44 本机构建可单独选择 rpm
python3 scripts/package.py --target x86_64-unknown-linux-gnu --linux-format tar.gz --linux-format deb --linux-format rpm --output dist/ubuntu
```

GitHub CI 检查三个操作系统；推送与 Cargo 版本一致的 `app-v*` 标签触发六个原生构建任务，
产出六个 Rust 目标的十二份分发包。包内启动、Ubuntu runner 上的 DEB 安装／卸载、
RPM 内容及解包运行检查成功后，校验完整产物集合与摘要并创建 **Release 草稿**，不自动公开发布。
无需证书或签名 Secrets；macOS 仅保留免证书 ad-hoc 签名，下载时仍可能出现系统安全提示。
RPM 不承诺兼容所有 RPM 发行版；原生工具要求与验收边界见[打包与发布指南](development.md#packaging-and-releases)。

## 开发与验证入口

[Cargo.toml](../../Cargo.toml) 拥有 workspace、默认入口和共享依赖。修改库时明确选择
package 和 target，并检查受影响的原生调用方。界面行为仍需人工验收。

| 任务 | 根命令 |
| --- | --- |
| 原生编译检查 | `cargo check -p yss-desktop-gpui --bin yss-desktop-gpui` |
| 原生功能静态检查 | `cargo clippy -p <affected-ui-package> -p yss-workbench --all-targets --no-deps -- -D warnings` |
| 库的聚焦测试 | `cargo test -p <package> --lib <case>` |
| 包格式检查 | `cargo fmt -p <package> -- --check` |

日常改动按[验证规则](development/testing.md#select-checks)选择受影响范围，不将 workspace 全量检查作为局部改动的默认收尾。
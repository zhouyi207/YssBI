# Getting Started

YssBI is a native desktop application for building data-analysis graphs, inspecting results, and working with an AI Assistant. The current desktop uses Rust and GPUI; the `react/` directory is reference material, not a second application to launch.

## Run from source

YssBI is unreleased. These instructions use a source checkout, not a published installer or a preinstalled `yssbi` command.

1. Get the [repository](https://github.com/zhouyi207/YssBI) and install the Rust toolchain selected by its [rust-toolchain.toml](https://github.com/zhouyi207/YssBI/blob/main/rust-toolchain.toml).
2. Install the native build dependencies for your system. See [Linux development](development/linux.md) for the Linux setup.
3. From the repository root, launch the desktop:

   ```sh
   cargo run
   ```

The first build also compiles the desktop rendering dependencies. Use the [development guide](development.md) for build details and the [CLI reference](reference/cli.md) to open a project directly.

## Start with a disposable project

On the welcome page, create a project by choosing a name and directory, or open an existing project directory. Recent projects provide a shortcut to projects you have already opened.

> **Warning:** Copy an existing project to a separate directory before experimenting. Opening a project can initialize directories and perform database maintenance; avoiding Save is not a read-only guarantee.

For a first analysis:

1. Create a disposable project or open your copy.
2. Use the Data menu or the data group's **+** button to import data. Bundled samples are available through the same import flow; their catalog and Parquet files are included in the source checkout, so development use does not require downloading sample data.
3. Open the imported database from the project sidebar to inspect its columns and rows.
4. Create an Event graph from the File menu. Add nodes from the Nodes directory or the canvas menu, configure their inputs, and connect their data ports.
5. Check Problems, save the graph explicitly, and use its run control. Open available outputs from Results.

[Graphs and Results](graphs-and-results.md) explains dependencies, execution, and result snapshots. [Windows and Projects](windows-and-projects.md) explains saving and switching projects. You do not need a model account to use these workflows; configure one only if you want the [Assistant](ai/overview.md).

## Current limits

The native migration is still in progress. Some controls are unavailable, and implemented interactions do not imply completed end-to-end acceptance. Windows and macOS input, window behavior, platform paths, and distribution remain unaccepted on their target systems. Linux also has outstanding interaction and lifecycle checks.

See the [desktop implementation contract](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-desktop-gpui/README.md) and [open native work](https://github.com/zhouyi207/YssBI/blob/main/TODO.md#native-workbench) for the current boundary. These guides describe source behavior; they do not record a new application or platform test run.

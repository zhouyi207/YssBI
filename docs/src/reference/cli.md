# CLI

The desktop entry point accepts an optional project path followed by an optional resource identifier. Run it from a source checkout; YssBI does not currently provide a published installer or a separately installed `yssbi` command.

## Launch the desktop

From the repository root:

```sh
cargo run
```

Without arguments, the application opens the welcome workflow. To open an existing project, pass its directory after Cargo's argument separator:

```sh
cargo run -- "/absolute/path/to/project-copy"
```

Use a disposable project copy for experiments. Opening a project can initialize directories and perform database maintenance even if you never save a graph.

## Open a resource

Pass the project first and the resource second:

```sh
cargo run -- "/absolute/path/to/project-copy" events/example.yssbi-event
```

The resource must already belong to that project. Resource paths are relative to the project, not to the source checkout. Databases use their database ID instead of a graph or document path.

| Resource          | Example second argument      |
| ----------------- | ---------------------------- |
| Event graph       | `events/example.yssbi-event` |
| Markdown document | `docs/example.md`            |
| Mind document     | `minds/example.yssbi-mind`   |
| Chart             | `charts/example.yssbi-chart` |
| Database          | The existing database's ID   |

For example, to open a Markdown document:

```sh
cargo run -- "/absolute/path/to/project-copy" docs/example.md
```

These names are examples, not resources the command creates. Replace them with the actual path or database ID in your project. Quote paths containing spaces using your shell's normal quoting rules.

## Argument boundary

The desktop accepts at most two positional arguments:

```text
[project-path [resource]]
```

You cannot supply a resource without a project. These arguments open the graphical workbench; they are not a headless graph runner, an import command, or an automatic execution request.

In the examples, `--` belongs to Cargo and separates Cargo options from desktop arguments. For a release-mode source launch, place Cargo's `--release` before that separator:

```sh
cargo run --release -- "/absolute/path/to/project-copy" events/example.yssbi-event
```

No desktop flags for model selection, execution, export, or project creation are documented here because the entry point does not expose those workflows as flags.

## Source and related guides

The [desktop entry point](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-desktop-gpui/src/main.rs) owns argument handling; the [desktop README](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-desktop-gpui/README.md) documents launch resources. See [Getting Started](../getting-started.md) for prerequisites, [Windows and Projects](../windows-and-projects.md) for project operations, and [Development](../development.md) for building the application.

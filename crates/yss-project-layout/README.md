# Project layout

> Status: Current
> Scope: Canonical project file names, content directories and index input classification
> Canonical owners: [src/lib.rs](src/lib.rs) owns layout constants and the pure path classifier
> Update when: Project layout or index input classification changes

This crate defines project metadata, resource directory and file-extension names.
It performs no I/O and holds no runtime state. Document formats belong to their
resource owners; project reads, writes and index publication belong to
[Project](../yss-project/README.md).

`is_project_index_input_path` classifies safe project-relative paths by their
native `Path` components. Only normal components are accepted. The metadata
file is an input only at the project root; content directory paths and their
descendants are inputs. Names are compared as `OsStr` values without lossy text
conversion or foreign separator normalization. On Unix, a backslash remains
part of a name; on Windows, native separators form directory components.

Project uses this classifier to interpret
[filesystem change facts](../yss-filesystem/README.md). Application passes the
same classifier to the native file watcher. Generic filesystem code does not
maintain a separate project directory policy.

## Validation

Run from the repository root:

```sh
cargo test -p yss-project-layout --lib
cargo test -p yss-project --lib file_changes::tests
cargo check -p yss-application --lib
cargo clippy -p yss-project-layout --lib --no-deps -- -D warnings
cargo fmt -p yss-project-layout -- --check
```

The Unix regression rejects literal backslash names that resemble content
directories. The shared classification case uses a native joined path to cover
the current platform's separators. Windows execution requires a Windows target.

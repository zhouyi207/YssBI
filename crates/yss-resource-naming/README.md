# Resource naming

> Status: Current
> Scope: Portable names and unique-name allocation for file-backed project resources
> Canonical owners: [src/lib.rs](src/lib.rs) owns validation, portable comparison keys and allocation
> Update when: Allowed names, comparison or allocation behavior changes

`ResourceName` validates one resource name without I/O or project state. Graph,
Chart, Mind and Doc path owners reuse this contract and supply their own
directory and extension rules. Database display names use the distinct
[display-name allocator](../yss-display-naming/src/lib.rs).

Names must already be NFC and contain only Unicode letters, numbers, ASCII
spaces, hyphens, underscores and parentheses. Spaces cannot lead, trail or
repeat. The maximum scalar count is owned by `MAX_RESOURCE_NAME_CHARACTERS`.
Windows device names are rejected case-insensitively, including `COM` and `LPT`
with the reserved superscript digits `¹`, `²` and `³`, as specified by
[Microsoft's file naming rules](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file).
Validation does not silently normalize or rename input.

Portable comparison keys use Unicode case folding followed by NFC. Allocation
keeps the requested base if its key is free, then finds the first free numeric
suffix beginning at two. It truncates the base to keep the result within the
same length limit and validates the generated name through the same owner.
Allocation does not publish resources or reserve filesystem paths; Project
owns collision checks at commit time.

Run focused checks from the repository root:

```sh
cargo test -p yss-resource-naming --lib
cargo test -p yss-graph-document --lib graph_resource_names_preserve_exact_unicode_validation
cargo test -p yss-chart-document --lib
cargo check -p yss-desktop-gpui --bin yss-desktop-gpui
cargo clippy -p yss-resource-naming --lib --no-deps -- -D warnings
cargo fmt -p yss-resource-naming -- --check
```

The reserved-name regression runs on every platform. Actual Windows file
creation requires acceptance on Windows.

# Testing

Choose checks that exercise the changed behavior and its consumers. Compilation, behavior tests and interactive acceptance answer different questions; do not report one as proof of another.

## Select checks {#select-checks}

Run Cargo commands from the repository root. Select the affected package and target explicitly. A change to shared types, serialization or cross-module behavior also needs the relevant consumer checks; testing only the edited crate is not sufficient.

For the native desktop, include the feature crate being changed. For example, a graph UI change:

```sh
cargo check -p yss-desktop-gpui --all-targets
cargo clippy -p yss-desktop-gpui -p yss-workbench -p yss-graph-ui --all-targets --no-deps -- -D warnings
cargo fmt -p yss-desktop-gpui -p yss-workbench -p yss-graph-ui -- --check
```

The binary is now a thin entry point. Its compilation checks dependency interfaces, but selecting only that package does not run the feature libraries' tests or format/lint their source. Include `yss-ui`, `yss-desktop-services` and other affected features when their contracts change.

Check shared UI independently so feature dependencies cannot hide missing imports or feature requirements:

```sh
cargo check -p yss-ui --all-targets
cargo tree -p yss-ui --edges normal,build
cargo test -p yss-application --lib literals_bypass_translation_and_interpolation
```

The shared UI dependency closure must not pull in Application, Project, DataFusion, Arrow, Rig or the plugin runtime. For resource catalog and localized presentation changes, exercise the Assistant resource picker, node catalog and graph diagnostics in the native application.

Existing desktop behavior tests and the merged private-module tests run under their current owners:

```sh
cargo test -p yss-ui -p yss-data-ui -p yss-resource-ui --lib
cargo test -p yss-project --lib project_operation
cargo test -p yss-application --lib display_naming
```

For a library, use its package and the relevant test filter. For example, to run the Graph Runtime library tests:

```sh
cargo test -p yss-graph-runtime --lib
```

Use module READMEs for prerequisites and focused cases. A zero-match filter, a compilation-only check or failed environment initialization is not a passing behavior test. Do not run the entire workspace by default for a bounded local change. Broaden checks when a shared contract or unresolved failure requires it, and state what additional risk the broader check covers.

Loopback HTTP fixtures must not route through an outbound system proxy. On machines with a proxy enabled, scope `NO_PROXY=127.0.0.1,localhost,::1` to the test process and retain any other required bypass entries; do not change the user's global proxy settings or weaken timeout assertions. If concurrent compilation exhausts Windows commit memory (`os error 1455` / LLVM out of memory), use Cargo's `--jobs 1` for the same check rather than changing application code or omitting packages.

For functional fixtures that issue independent kernel or relation requests, create controls at each request boundary rather than sharing a deadline across earlier setup and operations. Keep the original per-request timeout, cancellation flag and memory limits, and reuse immutable registries and catalogs. Tests of deadline propagation retain their deliberate shared or expired controls.

## Regression coverage

Keep tests tied to observable behavior: state transitions, boundaries, precedence, error outcomes and durable commits. Reuse existing coverage before adding another case for the same path. Do not assert source spelling, file layout or the disappearance of an old symbol as a substitute for behavior.

Review [state ownership](./architecture.md#state-ownership), dependency direction and public callers alongside the checks. A successful compiler run does not prove that ownership remains correct.

## Desktop acceptance

Exercise the changed path in the native application with isolated application data and a project copy. Include the relevant success, failure, cancellation and late-result behavior. Check focus, keyboard/IME, pointer input, resize, theme and platform behavior where the change affects them.

A component test or event-injected preview does not establish physical input, accessibility, repaint timing, multiple-window behavior or support on another operating system. Keep those acceptance items open until they are exercised. The [desktop README](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-desktop-gpui/README.md) retains detailed scenarios; [TODO.md](https://github.com/zhouyi207/YssBI/blob/main/TODO.md) tracks remaining work.

The published GPUI Fast backend is experimental. A dependency migration needs application regressions and native checks of cached accessibility callbacks, hidden-control removal, menu focus and retained drafts. A forced-refresh headless frame is not proof of a cache hit. The retired vendored framework's property-test macro and font-fixture repairs are not maintained here; their old internal-test results do not verify the current backend.

For crate-boundary changes, exercise cross-owner paths: run a graph and open a leased result in another window; close the source tab; import and paginate a real dataset; save document, mind and chart edits; hide/reopen Assistant input; fail and then complete a project switch; change preferences and restart with saved DockArea layout. Use `YSSBI_APP_DATA_DIR` with an absolute temporary directory to avoid touching normal application state. These scenarios do not establish real-provider streaming, signed plugin execution, physical IME behavior or untested platform support.

## Documentation

For book changes, use the [documentation build instructions](https://github.com/zhouyi207/YssBI/blob/main/docs/README.md). Build and inspect the book, check relative links and anchors, and run `git diff --check`. Documentation-only changes do not require a desktop rebuild.

## Report the evidence

Record the commands, affected scope and actual results. Keep new failures separate from unrun checks. Reuse fresh results while the relevant code and inputs are unchanged; do not rerun costly checks merely to produce another completion log.

## See also

- [Developing YssBI](../development.md)
- [Architecture](./architecture.md)
- [Performance](../performance.md)

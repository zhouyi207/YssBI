# Testing

Choose checks that exercise the changed behavior and its consumers. Compilation, behavior tests and interactive acceptance answer different questions; do not report one as proof of another.

## Select checks {#select-checks}

Run Cargo commands from the repository root. Select the affected package and target explicitly. A change to shared types, serialization or cross-module behavior also needs the relevant consumer checks; testing only the edited crate is not sufficient.

For the native desktop:

```sh
cargo check -p yss-desktop-gpui --bin yss-desktop-gpui
cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings
cargo fmt -p yss-desktop-gpui -- --check
```

For a library, use its package and the relevant test filter. For example, to run the Graph Runtime library tests:

```sh
cargo test -p yss-graph-runtime --lib
```

Use module READMEs for prerequisites and focused cases. A zero-match filter, a compilation-only check or failed environment initialization is not a passing behavior test. Do not run the entire workspace by default for a bounded local change. Broaden checks when a shared contract or unresolved failure requires it, and state what additional risk the broader check covers.

## Regression coverage

Keep tests tied to observable behavior: state transitions, boundaries, precedence, error outcomes and durable commits. Reuse existing coverage before adding another case for the same path. Do not assert source spelling, file layout or the disappearance of an old symbol as a substitute for behavior.

Review [state ownership](./architecture.md#state-ownership), dependency direction and public callers alongside the checks. A successful compiler run does not prove that ownership remains correct.

## Desktop acceptance

Exercise the changed path in the native application with isolated application data and a project copy. Include the relevant success, failure, cancellation and late-result behavior. Check focus, keyboard/IME, pointer input, resize, theme and platform behavior where the change affects them.

A component test or event-injected preview does not establish physical input, accessibility, repaint timing, multiple-window behavior or support on another operating system. Keep those acceptance items open until they are exercised. The [desktop README](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-desktop-gpui/README.md) retains detailed scenarios; [TODO.md](https://github.com/zhouyi207/YssBI/blob/main/TODO.md) tracks remaining work.

## Documentation and generated data

For book changes, use the [documentation build instructions](https://github.com/zhouyi207/YssBI/blob/main/docs/README.md). Build and inspect the book, check relative links and anchors, and run `git diff --check`. Documentation-only changes do not require a desktop rebuild.

The React reference dependency data has a separate check:

```sh
node scripts/generate-crate-dependencies.mjs --check
```

That command checks the existing `crateDependencies.json` against Cargo metadata. It does not generate book pages, validate architecture or require the React application build.

## Report the evidence

Record the commands, affected scope and actual results. Keep new failures separate from unrun checks. Reuse fresh results while the relevant code and inputs are unchanged; do not rerun costly checks merely to produce another completion log.

## See also

- [Developing YssBI](../development.md)
- [Architecture](./architecture.md)
- [Performance](../performance.md)

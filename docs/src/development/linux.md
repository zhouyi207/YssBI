# Building YssBI for Linux

Run the native desktop application from a source checkout. Linux is the current development platform; visual, input and performance acceptance still needs a real desktop session.

## Dependencies

Install [rustup](https://rustup.rs/) and the C/C++ build toolchain. Rustup uses the version in the repository's `rust-toolchain.toml`.

The existing Fedora build needs development libraries for fontconfig, libxkbcommon, Wayland/X11 and OpenSSL, including `libxkbcommon-x11-devel` for linking. Package names differ across distributions. Use a working graphics driver and a desktop session; a successful headless compile does not validate the GPU or input path.

See the [desktop module README](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-desktop-gpui/README.md) for the current platform notes. The application uses the platform features supplied by GPUI Kit; it does not require the old React build tools.

## Build and run

From the repository root:

```sh
cargo run
```

Open a project directly:

```sh
cargo run -- /absolute/path/to/project
```

Build an optimized binary:

```sh
cargo build --release
```

The binary is `target/release/yss-desktop-gpui`. Follow the [resource staging instructions](../development.md#build-and-run) before running a standalone copy outside the source checkout.

## Investigate a build or startup failure

- For a missing native library, install its development package rather than changing Rust dependencies to hide the linker error.
- For a window or rendering failure, record the desktop session, graphics driver, display scaling and exact launch command.
- Separate compilation, startup, redraw, keyboard/IME and pointer behavior when reporting results. A static screenshot is not evidence that later frames or interactions work.
- Use an isolated project copy for checks that edit data, save files or test recovery.

## See also

- [CLI reference](../reference/cli.md)
- [Testing](./testing.md)
- [Performance](../performance.md)

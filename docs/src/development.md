# Developing YssBI

Build the native desktop application from the repository root. YssBI uses a Rust Cargo workspace with `yss-desktop-gpui` as its default member.

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

Distribute the resulting `resources` folder with the binary. See the [sample asset guide](https://github.com/zhouyi207/YssBI/blob/gpui/resources/samples/README.md) for resource generation and validation. Use the native packaging entry below for distributions; automatic update delivery is not implemented.

## Packaging and releases

The implementation follows Zed's [release workflow](https://github.com/zed-industries/zed/blob/main/.github/workflows/release.yml)
and native [Linux](https://github.com/zed-industries/zed/blob/main/script/bundle-linux),
[macOS](https://github.com/zed-industries/zed/blob/main/script/bundle-mac) and
[Windows](https://github.com/zed-industries/zed/blob/main/script/bundle-windows.ps1) bundling:
native target runners, complete asset validation, then a draft release.
It does not copy Zed's private runners, signing services, branding, license, remote server or updater.

### Targets and local commands

Python 3.12 or newer and the repository Rust toolchain are required. Run from a native host
matching both the target OS and architecture; the driver rejects cross-packaging.
All automated Linux builds run directly on Ubuntu 24.04.

| OS / build baseline | Rust targets | Distribution | GitHub runners |
| --- | --- | --- | --- |
| Ubuntu 24.04 | `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` | `.tar.gz`, `.deb`, `.rpm` | `ubuntu-24.04`, `ubuntu-24.04-arm` |
| macOS | `x86_64-apple-darwin`, `aarch64-apple-darwin` | `.dmg` containing `YssBI.app` | `macos-15-intel`, `macos-15` |
| Windows | `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` | Inno Setup `.exe` and portable `.zip` | `windows-2022`, `windows-11-arm` |

```sh
# Default Linux output: relocatable archive.
python scripts/package.py --target x86_64-unknown-linux-gnu --output dist

# Ubuntu 24.04: all three Linux formats.
python scripts/package.py --target x86_64-unknown-linux-gnu --linux-format tar.gz --linux-format deb --linux-format rpm --output dist/ubuntu

# Fedora 44: local native RPM build, using rpm-build and native development packages.
python scripts/package.py --target x86_64-unknown-linux-gnu --linux-format rpm --output dist/fedora

# Repackage an existing target/<triple>/release build.
python scripts/package.py --target x86_64-unknown-linux-gnu --skip-build --output dist/repack

# Repackage a specified native binary for local validation, not an official release.
python scripts/package.py --target x86_64-unknown-linux-gnu --binary target/release/yss-desktop-gpui --output dist/local
```

The driver uses `cargo build --locked --release`, then runs the existing
`build_samples --check --stage` example with the same explicit native `--target` and
release profile, so Cargo can reuse matching dependency artifacts instead of building
a separate implicit-host copy. It verifies every Parquet size and SHA-256 against the
catalog and includes dataset provenance. `THIRD_PARTY_NOTICES.txt`
contains resolved Rust dependency metadata and available license/notice files. This is
not a legal compliance certification: the application has no declared distribution license,
and maintainers must review application, bundled native library, and dataset redistribution
terms before publishing. No application license or product icon is invented.
RPM's required license field is `LicenseRef-Unknown`; it is not a redistribution grant.

Assets use `yssbi-<version>-<os>-<architecture>.<extension>`, including
`yssbi-<version>-linux-<architecture>.rpm`. DEB uses
`yssbi-<version>-ubuntu24.04-<architecture>.deb`. Every asset has a `.sha256` sidecar.
The driver refuses to overwrite existing artifacts; use a fresh output directory.
Without `--linux-format`, Linux produces only the archive. DEB builds require Ubuntu 24.04;
RPM builds accept Ubuntu 24.04 or Fedora 44. Package versions use `~` for prereleases,
omit SemVer build metadata and follow dpkg/RPM ordering; both package revisions are `1`.

**An RPM extension does not imply compatibility with every RPM distribution.** Official
RPMs use Ubuntu 24.04's glibc baseline and SONAME/versioned ELF requirements, not Fedora
build provenance. Local Fedora builds can require newer libraries. Fedora, RHEL and
openSUSE installation compatibility must be checked on the intended release; missing
dependencies are not bypassed. DEB likewise does not imply support for every Debian derivative.

### Native prerequisites and installation layout

The shared `.github/actions/setup-build/action.yml` owns the hosted build prerequisites.
The Rust version comes from `rust-toolchain.toml`; system packages come from the selected
runner's distribution repositories.

- **Linux archive:** Clang/C++ tools, CMake, pkg-config and the fontconfig, FreeType, ALSA, GLib,
  Wayland, X11/XCB and xkbcommon development packages. Extract the archive and run
  `yssbi.app/bin/yssbi`; keep the whole directory together. Its relocatable launcher uses
  bundled non-system libraries, while libc, the GLib family, graphics drivers and ALSA stay
  on the host. `libexec/resources/samples` is beside the actual executable. The included
  `share/applications/com.zjy.yssbi.desktop` expects `yssbi` on the desktop session's PATH.
  A locally built archive is not proof of compatibility with older distributions.
- **Linux native packages:** Ubuntu requires `dpkg-dev`, `rpm`, `binutils` and `file`;
  Fedora local RPM builds require `rpm-build`. `dpkg-shlibdeps` and RPM's ELF scanner derive
  binary dependencies; explicit dependencies cover dynamically loaded font, keyboard,
  Wayland, EGL/Vulkan and desktop portal libraries. A GTK portal backend is recommended.
  Native packages do not bundle shared libraries or set `LD_LIBRARY_PATH`.
  Both install `/usr/libexec/yssbi/yss-desktop-gpui`, adjacent `resources/samples`,
  `/usr/bin/yssbi`, `/usr/share/applications/com.zjy.yssbi.desktop`, and notices under
  `/usr/share/doc/yssbi`. Install with `sudo apt install ./yssbi-<version>-ubuntu24.04-<architecture>.deb`
  or `sudo dnf install ./yssbi-<version>-linux-<architecture>.rpm`; launch `yssbi`.
  Removing/purging the package does not remove user projects, settings or application data.
  DEB installs notices as `/usr/share/doc/yssbi/copyright`; RPM marks
  `THIRD_PARTY_NOTICES.txt` as `%license`, preserving notices when ordinary documentation is excluded.
- **macOS:** Xcode command-line tools plus the Metal toolchain. Set
  `MACOSX_DEPLOYMENT_TARGET=10.15.7` for Intel or `11.0` for Apple Silicon when building;
  the bundle records any higher minimum embedded in the executable. Unresolved non-system
  dylibs fail packaging. Open the DMG and drag `YssBI.app` to Applications.
  The executable is `Contents/MacOS/yss-desktop-gpui`; resources are in `Contents/Resources`.
  The executable and app always receive certificate-free ad-hoc signatures, required for
  Apple Silicon execution. There is no Developer ID signing or notarization; Gatekeeper may block downloads.
- **Windows:** PowerShell 7, Visual Studio C++ build tools/target CRT redistributables,
  LLVM/libclang, Windows SDK `fxc.exe` and Inno Setup 6.3 or newer. Run inside the VS developer
  environment for the native target. `GPUI_FXC_PATH` is the full path to `fxc.exe`.
  Required MSVC runtime DLLs are architecture-checked and their existing Microsoft publisher
  signatures are verified before copying. This does not require a maintainer certificate.
  Unknown non-system DLL imports fail packaging. The unsigned installer is per-user,
  under `%LOCALAPPDATA%\Programs\YssBI`, with shortcuts and normal uninstall.
  Both EXE and ZIP keep `resources/samples` beside `yss-desktop-gpui.exe`.
  Uninstall preserves user projects and application data. SmartScreen may warn about downloads.

No developer certificates, signing credentials or signing-mode options are required or supported.
SHA-256 sidecars verify integrity, not publisher identity.

### GitHub setup and release procedure

CI runs on pull requests and pushes to `gpui`/`main`. `check-platform.yml` owns the
shared checks: workspace formatting and locked compilation on Linux/macOS/Windows,
focused portable unit tests, native resource discovery, and packaging regression tests.
Installed sample discovery belongs to the dependency-free `yss-installation-paths` crate;
`cargo test --locked -p yss-installation-paths --lib` covers executable-adjacent resources,
checkout fallback, and macOS bundle precedence without compiling the Workbench test binary.

Release preflight uses `scripts/release_ci.py` with read-only Actions access to find a
successful `ci.yml` **push** run for the exact release commit on this repository's
`gpui` or `main` branch. All three native check jobs must have completed successfully
in the same run attempt. PR/fork runs, other commits, and missing, skipped, failed or
cancelled checks cannot authorize reuse. The selected run URL and attempt are written
to the job summary. An API error fails preflight rather than bypassing checks.
If no qualifying run exists (including when CI is still running), each platform runs
fresh checks. Linux, macOS and Windows then start their own two architecture builds
independently; one platform does not wait for another platform's checks. Publication
still requires all three platform workflows and all six packages to succeed.

The shared build setup uses pinned `Swatinem/rust-cache` dependency caching, separated
by OS, architecture, native target, check/release profile and the action's Rust/environment
and manifest fingerprints. Workspace crate outputs and Cargo-installed binaries are
not saved. PRs restore caches but do not save them. GitHub ref scoping still applies:
default-branch caches are available to releases, but one release tag cannot restore
another tag's private cache, and check artifacts do not replace optimized release
artifacts. Cold release builds remain possible; same-tag retries can reuse their cache.
Compilation is capped at four jobs on Linux and two on macOS/Windows, bounded by the
available logical CPU count to retain memory headroom on hosted runners.

Actions are pinned to commit SHAs; Dependabot tracks Cargo and GitHub Actions weekly.
Checkout credentials are not persisted. Only the final draft publication job receives
`contents: write`, using GitHub's automatically supplied token.
No release environments or manually configured Secrets/variables are needed.
The repository's default workflow token permission remains read-only; PR approval by Actions is disabled.

After committing and pushing the workflow and application changes to `gpui`:

1. Set `[workspace.package].version` in `Cargo.toml` to the intended version and land that
   change on `gpui`. A release commit must be an ancestor of `origin/gpui`.
2. Create and push the matching tag, for example `app-v0.4.0`. A mismatched tag fails preflight.
   Alternatively, dispatch `release.yml` with an existing matching `app-v<version>` tag as its ref,
   not a branch. The tag must point to a commit containing this release workflow.
3. Allow preflight to reuse successful same-commit branch CI, or let each platform run
   its checks. To avoid duplicate checks, wait for branch CI to succeed before tagging.
   All six native build jobs must pass: two Linux, two macOS and two Windows jobs
   produce twelve bundles. Each target runs packaged `--help` checks.
   DEB is installed directly on its ephemeral Ubuntu build runner with normal APT dependency
   resolution; resource hashes, notices, launcher ownership, removal, reinstallation, purge
   and user-data preservation are checked. The build runner already has development
   dependencies, so this is not a minimal-system installation test.
   RPM is inspected without installation: metadata, declared dependencies, payload paths,
   modes, sample hashes and notices are checked, then the unpacked binary is launched.
   RPMs are not force-installed into Ubuntu's package database.
   Windows installs, launches and uninstalls its installer.
4. The final job checks the exact asset set and hashes, adds `SHA256SUMS`, uploads assets,
   verifies the uploaded set, and leaves a **draft** release for human review.
   An existing draft or public release is never overwritten. An interrupted upload can
   leave a partial draft: inspect and explicitly remove it before rerunning.
5. Review legal notices, target-distribution installation and native UI/resource behavior
   before manually publishing. The workflow does not auto-publish.

Every build's artifacts remain downloadable from its Actions run for 14 days.

### Verification boundaries

Local x86_64 validation repackaged an existing Ubuntu-built executable using Ubuntu 24.04's
actual RPM tools, extracted into a temporary tool directory on Fedora 44 without installing
them system-wide. RPM construction, declared dependencies, legal notices, five sample
hashes and relocated `--help` execution passed with both Ubuntu and Fedora RPM scanners.
The regenerated Linux archive also passed relocated execution and all sample hash checks.
An existing Ubuntu DEB passed the current metadata, payload and relocated launcher checks,
without installing it into the host package database.
Workflow syntax, PowerShell parsing and missing/corrupt release-asset rejection passed.
These checks do not replace a complete native Ubuntu hosted run.
Linux ARM64 and macOS/Windows packages still require their native hosted runners;
no release workflow or draft publication was triggered by local validation.

For read-only RPM validation, install Ubuntu's `rpm` package or Fedora's `rpm-build` package
(inspection also uses `rpmdeps`), along with the application's runtime libraries:

```sh
python scripts/linux_package_smoke.py rpm-contents path/to/yssbi-0.4.0-linux-x86_64.rpm
```

Native install/uninstall checks are available as `sudo python3 scripts/linux_package_smoke.py deb <asset>`
on Ubuntu 24.04 or `rpm <asset>` on Fedora 44. Run these only on disposable clean machines:
they modify the package database and install/remove packages. They refuse existing YssBI
installations or occupied installation paths. Keep the asset's `.sha256` sidecar beside it.
Read-only inspection uses `rpm2archive` and Python's filtered tar extraction, not a custom archive parser.

Ubuntu 24.04's `dpkg-shlibdeps` reports libc6 `/usr`-merge diversion warnings; these remain
visible, without missing-dependency bypasses or log filtering.
Headless `--help` checks do not establish GPU rendering, input, system dialogs,
Gatekeeper/SmartScreen behavior, or successful sample import. Exercise those on each
target before publishing the draft; graphical sessions require host drivers and a working portal backend.

## Bird's-eye view of YssBI

[Cargo.toml](https://github.com/zhouyi207/YssBI/blob/main/Cargo.toml) owns workspace membership and shared dependencies. These are useful starting points, not a second complete package inventory:

| Area                                        | Source entry                                                                                                                                                                                       |
| ------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Desktop entry and integration acceptance    | [yss-desktop-gpui](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-desktop-gpui/README.md)                                                                                                 |
| Window, project and Dock coordination       | [yss-workbench](https://github.com/zhouyi207/YssBI/tree/main/crates/yss-workbench/src)                                                                                                             |
| Shared presentation and background delivery | [yss-ui](https://github.com/zhouyi207/YssBI/tree/main/crates/yss-ui/src), [yss-desktop-services](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-desktop-services/src/desktop_services.rs) |
| Feature views and their retained owners     | [Desktop crate boundaries](./development/architecture.md#dependency-direction)                                                                                                                     |
| Typed use cases and composition             | [yss-application](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-application/README.md)                                                                                                   |
| Project resources and persistence           | [yss-project](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-project/README.md)                                                                                                           |
| Graph edits, execution and result delivery  | [Application graph](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-application/src/graph/README.md)                                                                                       |
| Graph-independent node invocation           | [yss-node-kernel](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-node-kernel/README.md)                                                                                                   |
| Dataset generations and commits             | [yss-database-store](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-database-store/README.md)                                                                                             |
| Statistical orchestration                   | [yss-sci-runtime](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-sci-runtime/README.md)                                                                                                   |
| Assistant runtime and tools                 | [yss-harness-core](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-harness-core/README.md)                                                                                                 |
| External plugin lifecycle                   | [yss-plugin-runtime](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-plugin-runtime/README.md)                                                                                             |
| Structured observations                     | [yss-logging](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-logging/README.md)                                                                                                           |

Read the [architecture guide](./development/architecture.md) for state ownership and dependency direction. Module READMEs describe the detailed interfaces and lifecycle contracts.

## Making a change

Use the repository's [coding rules](https://github.com/zhouyi207/YssBI/blob/main/.rules) and the applicable module rules. Prefer the existing owner of a behavior. Keep API, serialization and persistence changes consistent across callers; this unreleased project does not maintain old-contract migration layers.

Select checks for the affected modules and consumers using [Testing](./development/testing.md). Keep genuine missing capabilities and uncompleted acceptance in [TODO.md](https://github.com/zhouyi207/YssBI/blob/main/TODO.md), rather than treating a successful build as completed product work.

## See also

- [Getting started](./getting-started.md)
- [Performance](./performance.md)
- [Documentation build instructions](https://github.com/zhouyi207/YssBI/blob/main/docs/README.md)

#!/usr/bin/env python3
"""Build native YssBI distributions with the repository's pinned toolchain."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    f"{architecture}-{suffix}": (system, architecture)
    for system, suffix in (
        ("linux", "unknown-linux-gnu"),
        ("macos", "apple-darwin"),
        ("windows", "pc-windows-msvc"),
    )
    for architecture in ("x86_64", "aarch64")
}
# Keep host graphics drivers, libc and the GLib family together with the host.
SYSTEM_LIBRARIES = (
    "libc.so", "libm.so", "libpthread.so", "libdl.so", "librt.so", "libgcc_s.so",
    "libasound.so", "libglib-", "libgobject-", "libgio-", "libgmodule-",
    "libpcre", "libmount.so", "libblkid.so", "libselinux.so", "libffi.so",
    "libresolv.so", "libz.so", "libGL", "libEGL", "libvulkan.so", "libdrm",
    "libgbm.so", "libwayland-",
)


def run(arguments, **kwargs):
    print("+", " ".join(map(str, arguments)), flush=True)
    return subprocess.run(list(map(str, arguments)), check=True, cwd=ROOT, **kwargs)


def version():
    with (ROOT / "Cargo.toml").open("rb") as manifest:
        value = tomllib.load(manifest)["workspace"]["package"]["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?", value):
        raise ValueError(f"Invalid release version: {value}")
    return value


def verify_windows_linker(architecture):
    toolchain = os.environ.get("VCToolsInstallDir")
    linker = shutil.which("link.exe")
    if not toolchain or not linker:
        raise ValueError("Windows packaging requires the Visual Studio developer environment and MSVC link.exe")
    linker = Path(linker).resolve()
    target_architecture = {"x86_64": "x64", "aarch64": "arm64"}[architecture]
    candidates = {
        (Path(toolchain) / "bin" / host / target_architecture / "link.exe").resolve()
        for host in ("Hostx64", "Hostarm64", "Hostx86")
    }
    # Git Bash prepends its unrelated GNU link.exe ahead of the developer PATH.
    if not linker.is_file() or linker not in candidates:
        raise ValueError(
            f"Expected MSVC link.exe for {target_architecture}, but PATH resolves to {linker}. "
            "Run packaging from PowerShell in the matching Visual Studio developer environment, not Git Bash."
        )


def sha256(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def verify_samples(directory):
    catalog = json.loads((directory / "catalog.json").read_text(encoding="utf-8"))
    if catalog.get("formatVersion") != 1 or not catalog.get("datasets"):
        raise ValueError("Missing or unsupported sample catalog")
    for dataset in catalog["datasets"]:
        identifier = dataset["id"]
        revision = dataset["version"]
        if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", identifier) or type(revision) is not int or revision < 1:
            raise ValueError("Invalid sample identity")
        payload = directory / identifier / f"v{revision}" / "data.parquet"
        if payload.stat().st_size != dataset["byteSize"] or sha256(payload) != dataset["sha256"]:
            raise ValueError(f"Sample payload does not match catalog: {identifier}")
    for name in ("README.md", "sources.json"):
        if not (directory / name).is_file():
            raise ValueError(f"Missing sample provenance: {name}")


def verify_elf(binary, architecture):
    with binary.open("rb") as source:
        header = source.read(20)
    if len(header) != 20 or header[:6] != b"\x7fELF\x02\x01":
        raise ValueError(f"Not a 64-bit little-endian ELF executable: {binary}")
    machine = struct.unpack_from("<H", header, 18)[0]
    if machine != {"x86_64": 62, "aarch64": 183}[architecture]:
        raise ValueError(f"ELF architecture does not match {architecture}: {binary}")


def linux_libraries(binary):
    result = run(["ldd", binary], capture_output=True, text=True)
    libraries = {}
    for line in result.stdout.splitlines():
        if "not found" in line:
            raise ValueError(f"Unresolved native library: {line.strip()}")
        match = re.match(r"\s*(\S+) => (/[^\n]+?) \(0x[0-9a-f]+\)", line)
        if match:
            name, path = match.groups()
            if not name.startswith(SYSTEM_LIBRARIES):
                libraries[name] = Path(path)
    return libraries


def dependency_notices(target, destination):
    metadata = json.loads(run(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target],
        capture_output=True, text=True,
    ).stdout)
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [package["id"] for package in packages.values() if package["name"] == "yss-desktop-gpui"]
    included = set()
    while pending:
        identity = pending.pop()
        if identity in included:
            continue
        included.add(identity)
        pending.extend(
            dependency["pkg"] for dependency in nodes[identity]["deps"]
            if any(kind["kind"] != "dev" for kind in dependency["dep_kinds"])
        )
    with destination.open("w", encoding="utf-8") as output:
        output.write("YssBI third-party Rust dependency notices\n\n")
        output.write("Package metadata and license/notice files from the resolved sources follow.\n")
        output.write("Dataset provenance is distributed separately in resources/samples/README.md.\n\n")
        for package in sorted((packages[identity] for identity in included), key=lambda item: (item["name"], item["version"])):
            if package["id"] in metadata["workspace_members"]:
                continue
            output.write(f"\n{'=' * 72}\n{package['name']} {package['version']}\n")
            output.write(f"License expression: {package['license'] or 'not declared'}\n")
            output.write(f"Source: {package['repository'] or package['source']}\n")
            source = Path(package["manifest_path"]).parent
            candidates = set()
            if package.get("license_file"):
                candidates.add(source / package["license_file"])
            for pattern in ("LICENSE*", "LICENCE*", "COPYING*", "NOTICE*", "license*", "licence*"):
                candidates.update(source.glob(pattern))
            for license_path in sorted(path for path in candidates if path.is_file()):
                output.write(f"\n--- {license_path.name} ---\n")
                output.write(license_path.read_text(encoding="utf-8", errors="replace"))
                output.write("\n")


def package_linux(binary, samples, release_version, architecture, output, notices):
    verify_elf(binary, architecture)
    with tempfile.TemporaryDirectory(prefix="yssbi-linux-") as temporary:
        bundle = Path(temporary) / "yssbi.app"
        for directory in ("bin", "libexec", "lib", "share/applications"):
            (bundle / directory).mkdir(parents=True)
        packaged_binary = bundle / "libexec/yss-desktop-gpui"
        shutil.copy2(binary, packaged_binary)
        packaged_binary.chmod(0o755)
        shutil.copytree(samples, bundle / "libexec/resources/samples")
        shutil.copy2(notices, bundle / "THIRD_PARTY_NOTICES.txt")
        for name, source in linux_libraries(binary).items():
            shutil.copy2(source, bundle / "lib" / name)
        launcher = bundle / "bin/yssbi"
        launcher.write_text(
            '#!/bin/sh\nset -eu\n'
            'executable=$(readlink -f -- "$0")\n'
            'root=$(dirname -- "$(dirname -- "$executable")")\n'
            'export LD_LIBRARY_PATH="$root/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"\n'
            'exec "$root/libexec/yss-desktop-gpui" "$@"\n', encoding="utf-8",
        )
        launcher.chmod(0o755)
        (bundle / "share/applications/com.zjy.yssbi.desktop").write_text(
            '[Desktop Entry]\nType=Application\nName=YssBI\n'
            'Comment=Native data analysis workbench\nExec=yssbi %f\n'
            'Icon=applications-office\nTerminal=false\nCategories=Office;Science;\n'
            'StartupWMClass=com.zjy.yssbi\n', encoding="utf-8",
        )
        verify_samples(bundle / "libexec/resources/samples")
        run([launcher, "--help"], env={**os.environ, "LD_LIBRARY_PATH": ""})
        archive = output / f"yssbi-{release_version}-linux-{architecture}.tar.gz"
        with tarfile.open(archive, "w:gz") as compressed:
            compressed.add(bundle, arcname=bundle.name)
    return [archive]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    parser.add_argument("--skip-build", action="store_true", help="Package existing target/<triple>/release output")
    parser.add_argument("--binary", type=Path, help="Repackage an existing native binary instead of building")
    parser.add_argument(
        "--linux-format", action="append", choices=("tar.gz", "deb", "rpm"),
        help="Repeat to select Linux formats; defaults to tar.gz. DEB requires Ubuntu 24.04; RPM requires Ubuntu 24.04 or Fedora 44.",
    )
    arguments = parser.parse_args()
    system, architecture = TARGETS[arguments.target]
    host_system = {"Linux": "linux", "Darwin": "macos", "Windows": "windows"}.get(platform.system())
    if system != host_system:
        parser.error(f"{system} packaging must run on {system}")
    host_architecture = {"x86_64": "x86_64", "amd64": "x86_64", "arm64": "aarch64", "aarch64": "aarch64"}.get(platform.machine().lower())
    if architecture != host_architecture:
        parser.error("Use a native runner matching the target architecture")
    if system != "linux" and arguments.linux_format:
        parser.error("--linux-format is only available for Linux targets")
    linux_formats = list(dict.fromkeys(arguments.linux_format or ("tar.gz",)))
    if system == "linux":
        from package_linux_native import native_asset_name, package_native_linux, validate_distribution

        for package_format in linux_formats:
            if package_format != "tar.gz":
                validate_distribution(package_format)
    if system == "windows":
        verify_windows_linker(architecture)
    release_version = version()
    output = arguments.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    base = f"yssbi-{release_version}-{system}-{architecture}"
    if system == "linux":
        expected = [
            output / (
                f"{base}.tar.gz" if package_format == "tar.gz"
                else native_asset_name(release_version, architecture, package_format)
            )
            for package_format in linux_formats
        ]
    else:
        extensions = {"macos": ("dmg",), "windows": ("exe", "zip")}[system]
        expected = [output / f"{base}.{extension}" for extension in extensions]
    if any(path.exists() or path.with_name(path.name + ".sha256").exists() for path in expected):
        parser.error("Output already exists; choose an empty output directory")
    if not arguments.skip_build and arguments.binary is None:
        run(["cargo", "build", "--locked", "--release", "-p", "yss-desktop-gpui", "--bin", "yss-desktop-gpui", "--target", arguments.target])
    metadata = json.loads(run(["cargo", "metadata", "--no-deps", "--format-version", "1"], capture_output=True, text=True).stdout)
    suffix = ".exe" if system == "windows" else ""
    binary = (arguments.binary or Path(metadata["target_directory"]) / arguments.target / "release" / f"yss-desktop-gpui{suffix}").resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="yssbi-package-") as temporary:
        staging = Path(temporary)
        samples = staging / "samples"
        run(["cargo", "run", "--locked", "--release", "--target", arguments.target, "-p", "yss-application", "--example", "build_samples", "--", "--check", "--stage", samples])
        verify_samples(samples)
        notices = staging / "THIRD_PARTY_NOTICES.txt"
        dependency_notices(arguments.target, notices)
        if system == "linux":
            for package_format in linux_formats:
                if package_format == "tar.gz":
                    package_linux(binary, samples, release_version, architecture, output, notices)
                else:
                    package_native_linux(
                        binary, samples, release_version, architecture, output, notices, package_format,
                    )
        elif system == "macos":
            run([sys.executable, ROOT / "scripts/package_macos.py", "--binary", binary, "--samples", samples, "--version", release_version, "--target", arguments.target, "--output", output, "--notices", notices])
        else:
            run(["pwsh", "-NoProfile", "-File", ROOT / "scripts/package_windows.ps1", "-Binary", binary, "-Samples", samples, "-Version", release_version, "-Target", arguments.target, "-OutputDirectory", output, "-Notices", notices])
    for asset in expected:
        if not asset.is_file() or asset.stat().st_size == 0:
            raise ValueError(f"Packager did not produce {asset}")
        asset.with_name(asset.name + ".sha256").write_text(f"{sha256(asset)}  {asset.name}\n", encoding="utf-8")
        print(f"Packaged: {asset}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"Packaging failed: {error}", file=sys.stderr)
        sys.exit(1)

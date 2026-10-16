#!/usr/bin/env python3
"""Stage YssBI and build a native DMG using only Python and Apple tools.

The app is locally ad-hoc signed without certificates or Apple credentials.
The DMG contains the app and an Applications shortcut.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile


TARGETS = {
    "x86_64-apple-darwin": ("x86_64", "10.15.7"),
    "aarch64-apple-darwin": ("arm64", "11.0"),
}
# Apple Silicon itself requires macOS 11.0.
SEMVER = re.compile(
    r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-((?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
    r"(?:\.(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*))?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
)


def run(*arguments: str | Path) -> str:
    result = subprocess.run(
        [str(argument) for argument in arguments],
        text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    if result.returncode:
        raise RuntimeError(
            f"{Path(arguments[0]).name} failed (exit {result.returncode})"
            f"\n{result.stdout}\n{result.stderr}"
        )
    return result.stdout


def numeric_version(version: str) -> str:
    match = SEMVER.fullmatch(version)
    if match is None:
        raise ValueError(f"Not a semantic version: {version}")
    components = match.group(1, 2, 3)
    # CFBundleVersion's numeric fields are limited to four, two, two digits.
    if any(len(value) > limit for value, limit in zip(components, (4, 2, 2))):
        raise ValueError("Version exceeds Apple's CFBundleVersion field limits")
    return ".".join(components)


def version_tuple(version: str) -> tuple[int, int, int]:
    parts = tuple(int(part) for part in version.split("."))
    if not 1 <= len(parts) <= 3:
        raise ValueError(f"Invalid macOS version: {version}")
    return (parts + (0, 0))[:3]


def stage_app(
    binary: Path, samples: Path, version: str, target: str,
    destination: Path, minimum_os: str | None = None,
) -> Path:
    """Pure filesystem staging; destination must not already contain YssBI.app.

    Native binary validation is deliberately separate so tests on any host can
    exercise the complete bundle layout without Apple tools.
    """
    if target not in TARGETS:
        raise ValueError(f"Unsupported macOS target: {target}")
    bundle_version = numeric_version(version)
    baseline = TARGETS[target][1]
    minimum_os = max((baseline, minimum_os or baseline), key=version_tuple)
    if not binary.is_file():
        raise ValueError(f"Missing executable: {binary}")
    for name in ("README.md", "sources.json", "catalog.json"):
        if not (samples / name).is_file():
            raise ValueError(f"Missing staged sample resource: {name}")
    if not any(samples.rglob("*.parquet")):
        raise ValueError("Staged samples contain no Parquet files")
    # Staged resources must be self-contained, not references into a checkout.
    if samples.is_symlink() or any(path.is_symlink() for path in samples.rglob("*")):
        raise ValueError("Staged sample resources must not contain symlinks")
    app = destination / "YssBI.app"
    app.mkdir(parents=True, exist_ok=False)
    contents = app / "Contents"
    executable = contents / "MacOS" / "yss-desktop-gpui"
    executable.parent.mkdir(parents=True)
    shutil.copy2(binary, executable)
    executable.chmod(0o755)
    resources = contents / "Resources"
    resources.mkdir()
    shutil.copytree(samples, resources / "samples")
    info = {
        "CFBundleDevelopmentRegion": "en",
        "CFBundleDisplayName": "YssBI",
        "CFBundleName": "YssBI",
        "CFBundleExecutable": "yss-desktop-gpui",
        "CFBundleIdentifier": "com.zjy.yssbi",
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": bundle_version,
        "CFBundleVersion": bundle_version,
        # Keep prerelease/build metadata without invalid Apple version fields.
        "YssBIReleaseVersion": version,
        "LSMinimumSystemVersion": minimum_os,
        "NSHighResolutionCapable": True,
    }
    with (contents / "Info.plist").open("wb") as stream:
        plistlib.dump(info, stream, sort_keys=True)
    (contents / "PkgInfo").write_bytes(b"APPL????")
    return app


def validate_binary(binary: Path, target: str) -> str:
    expected, baseline = TARGETS[target]
    architectures = run("lipo", "-archs", binary).strip().split()
    if architectures != [expected]:
        raise ValueError(f"Expected {expected} Mach-O, found {architectures}")
    dependencies = run("otool", "-L", binary).splitlines()[1:]
    for line in dependencies:
        dependency = line.strip().split(" (", 1)[0]
        if not dependency.startswith(("/System/Library/", "/usr/lib/")):
            raise ValueError(
                f"Unbundled non-system dylib dependency: {dependency}. "
                "Build a self-contained executable before packaging."
            )
    load_commands = run("otool", "-l", binary)
    header = run("otool", "-hv", binary)
    if not re.search(r"\bfiletype\b", header):
        raise ValueError("Cannot inspect Mach-O executable header")
    if not re.search(r"\bEXECUTE\b", header):
        raise ValueError("Supplied Mach-O is not an executable")
    versions = []
    for command in re.split(r"Load command \d+", load_commands):
        if "LC_BUILD_VERSION" in command:
            match = re.search(r"^\s*minos\s+(\d+(?:\.\d+){0,2})\s*$", command, re.M)
        elif "LC_VERSION_MIN_MACOSX" in command:
            match = re.search(r"^\s*version\s+(\d+(?:\.\d+){0,2})\s*$", command, re.M)
        else:
            continue
        if match:
            versions.append(match.group(1))
    if not versions:
        raise ValueError("Mach-O contains no macOS deployment version")
    return max([baseline, *versions], key=version_tuple)


def sign_app(app: Path) -> None:
    options = ["--force", "--sign", "-", "--timestamp=none"]
    # There is one executable and no bundled frameworks; sign inside-out.
    # GPUI does not require JIT or disabled library validation entitlements.
    run("codesign", *options, app / "Contents/MacOS/yss-desktop-gpui")
    run("codesign", *options, app)
    run("codesign", "--verify", "--deep", "--strict", app)


def create_dmg(staging: Path, dmg: Path) -> None:
    app = staging / "YssBI.app"
    sign_app(app)
    (staging / "Applications").symlink_to("/Applications", target_is_directory=True)
    run("hdiutil", "create", "-volname", "YssBI", "-srcfolder", staging,
        "-format", "UDZO", "-fs", "HFS+", dmg)
    run("hdiutil", "verify", dmg)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--samples", type=Path, required=True)
    parser.add_argument("--notices", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    numeric_version(args.version)
    print("WARNING: the app is ad-hoc signed only; Gatekeeper may block it.",
          file=sys.stderr)
    if sys.platform != "darwin":
        raise RuntimeError("DMG packaging requires macOS and Xcode's command-line tools")
    minimum_os = validate_binary(args.binary.resolve(), args.target)
    args.output.mkdir(parents=True, exist_ok=True)
    architecture = args.target.split("-", 1)[0]
    name = f"yssbi-{args.version}-macos-{architecture}.dmg"
    # Work inside the output filesystem, then publish atomically only on success.
    with tempfile.TemporaryDirectory(prefix=".yssbi-macos-", dir=args.output.resolve()) as temporary:
        private = Path(temporary)
        staging = private / "image"
        app = stage_app(args.binary.resolve(), args.samples.resolve(), args.version,
                        args.target, staging, minimum_os)
        shutil.copy2(args.notices, app / "Contents/Resources/THIRD_PARTY_NOTICES.txt")
        dmg = private / name
        create_dmg(staging, dmg)
        dmg.replace(args.output / name)
    print(args.output / name)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError) as error:
        print(f"macOS packaging failed: {error}", file=sys.stderr)
        sys.exit(1)

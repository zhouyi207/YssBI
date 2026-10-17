import hashlib
import importlib.util
import json
import os
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


packaging = load("package")
macos = load("package_macos")


class PackagingTests(unittest.TestCase):
    def test_windows_accepts_msvc_linker_for_native_target(self):
        for architecture, target in (("x86_64", "x64"), ("aarch64", "arm64")):
            for host in ("Hostx64", "Hostarm64", "Hostx86"):
                with self.subTest(architecture=architecture, host=host), tempfile.TemporaryDirectory() as temporary:
                    toolchain = Path(temporary)
                    linker = toolchain / "bin" / host / target / "link.exe"
                    linker.parent.mkdir(parents=True)
                    linker.write_bytes(b"MSVC linker fixture")
                    with patch.dict(os.environ, {"VCToolsInstallDir": str(toolchain)}), \
                            patch.object(packaging.shutil, "which", return_value=str(linker)):
                        packaging.verify_windows_linker(architecture)

    def test_windows_rejects_git_linker_before_cargo_or_output_creation(self):
        for architecture, machine in (("x86_64", "AMD64"), ("aarch64", "ARM64")):
            with self.subTest(architecture=architecture), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                linker = root / "Git/usr/bin/link.exe"
                linker.parent.mkdir(parents=True)
                linker.write_bytes(b"GNU link fixture")
                output = root / "dist"
                with patch.dict(os.environ, {"VCToolsInstallDir": str(root / "MSVC")}), \
                        patch.object(packaging.shutil, "which", return_value=str(linker)), \
                        patch.object(packaging.platform, "system", return_value="Windows"), \
                        patch.object(packaging.platform, "machine", return_value=machine), \
                        patch.object(packaging.sys, "argv", ["package.py", "--target", f"{architecture}-pc-windows-msvc", "--output", str(output)]), \
                        patch.object(packaging, "run") as run:
                    with self.assertRaisesRegex(ValueError, "Expected MSVC link.exe"):
                        packaging.main()
                    run.assert_not_called()
                    self.assertFalse(output.exists())

    def test_windows_rejects_msvc_linker_for_wrong_target(self):
        with tempfile.TemporaryDirectory() as temporary:
            toolchain = Path(temporary)
            linker = toolchain / "bin/Hostx64/x64/link.exe"
            linker.parent.mkdir(parents=True)
            linker.write_bytes(b"MSVC linker fixture")
            with patch.dict(os.environ, {"VCToolsInstallDir": str(toolchain)}), \
                    patch.object(packaging.shutil, "which", return_value=str(linker)):
                with self.assertRaisesRegex(ValueError, "Expected MSVC link.exe for arm64"):
                    packaging.verify_windows_linker("aarch64")

    def test_windows_rejects_missing_developer_environment_or_linker(self):
        for environment, linker in (({}, "link.exe"), ({"VCToolsInstallDir": "MSVC"}, None)):
            with self.subTest(environment=environment, linker=linker), \
                    patch.dict(os.environ, environment, clear=True), \
                    patch.object(packaging.shutil, "which", return_value=linker):
                with self.assertRaisesRegex(ValueError, "requires the Visual Studio developer environment"):
                    packaging.verify_windows_linker("x86_64")

    def test_wrong_architecture_is_rejected_before_distribution(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "application"
            header = bytearray(20)
            header[:6] = b"\x7fELF\x02\x01"
            struct.pack_into("<H", header, 18, 183)
            binary.write_bytes(header)
            packaging.verify_elf(binary, "aarch64")
            with self.assertRaisesRegex(ValueError, "architecture"):
                packaging.verify_elf(binary, "x86_64")
            binary.write_bytes(b"not an executable")
            with self.assertRaisesRegex(ValueError, "ELF"):
                packaging.verify_elf(binary, "aarch64")

    def test_corrupt_sample_payload_blocks_packaging(self):
        with tempfile.TemporaryDirectory() as temporary:
            samples = Path(temporary)
            payload = samples / "iris/v1/data.parquet"
            payload.parent.mkdir(parents=True)
            original = b"PAR1sample fixturePAR1"
            payload.write_bytes(original)
            catalog = {
                "formatVersion": 1,
                "datasets": [{
                    "id": "iris", "version": 1, "byteSize": len(original),
                    "sha256": hashlib.sha256(original).hexdigest(),
                }],
            }
            (samples / "catalog.json").write_text(json.dumps(catalog))
            (samples / "README.md").write_text("Dataset attribution")
            (samples / "sources.json").write_text("{}")
            packaging.verify_samples(samples)
            payload.write_bytes(b"X" * len(original))
            with self.assertRaisesRegex(ValueError, "does not match"):
                packaging.verify_samples(samples)

    def test_sample_identity_cannot_escape_resource_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            samples = Path(temporary)
            (samples / "catalog.json").write_text(json.dumps({
                "formatVersion": 1,
                "datasets": [{"id": "../outside", "version": 1}],
            }))
            with self.assertRaisesRegex(ValueError, "identity"):
                packaging.verify_samples(samples)

    def test_prerelease_versions_have_valid_apple_numeric_version(self):
        self.assertEqual(macos.numeric_version("0.4.0-rc.2+build.15"), "0.4.0")
        for invalid in ("0.4", "01.4.0", "0.4.0-01", "0.100.0", "10000.0.0"):
            with self.subTest(version=invalid):
                with self.assertRaises(ValueError):
                    macos.numeric_version(invalid)


if __name__ == "__main__":
    unittest.main()

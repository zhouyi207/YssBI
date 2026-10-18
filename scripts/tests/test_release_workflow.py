from contextlib import chdir
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest
from unittest.mock import patch


WORKFLOW = (Path(__file__).resolve().parents[2] / ".github/workflows/release.yml").read_text()


def workflow_python(step_name):
    step = WORKFLOW.split(f"- name: {step_name}\n", 1)[1]
    source = step.split("python - <<'PY'\n", 1)[1].split("\n          PY", 1)[0]
    return compile(textwrap.dedent(source), f"release.yml: {step_name}", "exec")


PREFLIGHT = workflow_python("Validate release tag and version")
PUBLISH = workflow_python("Validate complete release and create draft")


class ReleaseWorkflowTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.enterContext(chdir(self.root))
        self.version = "0.4.0"
        self.set_version(self.version)
        self.output = self.root / "output"
        self.enterContext(patch.dict(os.environ, {
            "GITHUB_REF": "refs/tags/app-v0.4.0",
            "GITHUB_OUTPUT": str(self.output),
            "PACKAGE_VERSION": self.version,
            "GITHUB_API_URL": "https://api.github.test",
            "GITHUB_REPOSITORY": "test/repository",
            "GH_TOKEN": "test-token",
        }))
        self.git = self.enterContext(patch("subprocess.run"))
        self.network = self.enterContext(patch("urllib.request.urlopen"))

    def set_version(self, version):
        (self.root / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{version}"\n')

    def stage_assets(self):
        directory = self.root / "dist"
        directory.mkdir()
        for platform, extensions in {
            "linux": ("tar.gz", "rpm"), "ubuntu24.04": ("deb",),
            "macos": ("dmg",), "windows": ("exe", "zip"),
        }.items():
            for architecture in ("x86_64", "aarch64"):
                for extension in extensions:
                    name = f"yssbi-{self.version}-{platform}-{architecture}.{extension}"
                    payload = name.encode()
                    (directory / name).write_bytes(payload)
                    (directory / (name + ".sha256")).write_text(
                        f"{hashlib.sha256(payload).hexdigest()}  {name}\n"
                    )

    def test_preflight_accepts_matching_app_version_tags(self):
        for version in ("0.4.0", "0.5.0", "0.5.0-rc.1+build.2"):
            with self.subTest(version=version):
                self.set_version(version)
                os.environ["GITHUB_REF"] = f"refs/tags/app-v{version}"
                self.output.write_text("")
                exec(PREFLIGHT, {})
                self.assertEqual(self.output.read_text(), f"version={version}\n")
                self.git.assert_called_with(
                    ["git", "merge-base", "--is-ancestor", "HEAD", "origin/gpui"], check=True
                )

    def test_both_stages_reject_wrong_refs_before_side_effects(self):
        for reference in ("refs/tags/v0.4.0", "refs/tags/app-v0.4.1", "refs/heads/gpui",
                          "refs/heads/app-v0.4.0", "refs/tags/app-v0.4.0-rc.1"):
            for source in (PREFLIGHT, PUBLISH):
                with self.subTest(reference=reference, source=source.co_filename):
                    os.environ["GITHUB_REF"] = reference
                    with self.assertRaises(SystemExit):
                        exec(source, {})
        self.git.assert_not_called()
        self.network.assert_not_called()
        self.assertFalse(self.output.exists())

    def test_preflight_rejects_invalid_workspace_version(self):
        self.set_version("0.4")
        os.environ["GITHUB_REF"] = "refs/tags/app-v0.4"
        with self.assertRaisesRegex(SystemExit, "not a release version"):
            exec(PREFLIGHT, {})
        self.git.assert_not_called()

    def test_preflight_preserves_gpui_ancestry_guard(self):
        self.git.side_effect = subprocess.CalledProcessError(1, "git")
        with self.assertRaises(subprocess.CalledProcessError):
            exec(PREFLIGHT, {})
        self.assertFalse(self.output.exists())

    def test_publish_rejects_package_version_mismatch(self):
        os.environ["PACKAGE_VERSION"] = "0.4.1"
        with self.assertRaisesRegex(SystemExit, "disagree"):
            exec(PUBLISH, {})
        self.network.assert_not_called()

    def test_publish_refuses_existing_app_release(self):
        self.stage_assets()
        self.network.return_value = io.BytesIO(json.dumps([{"tag_name": "app-v0.4.0"}]).encode())
        with self.assertRaisesRegex(SystemExit, "already exists"):
            exec(PUBLISH, {})
        self.assertEqual(self.network.call_count, 1)
        self.assertEqual(self.network.call_args.args[0].get_method(), "GET")

    def test_draft_creation_uses_app_tag_and_preserves_versioned_assets(self):
        self.stage_assets()
        created = []

        def response(request, **kwargs):
            if request.get_method() == "POST":
                created.append(json.loads(request.data))
                raise RuntimeError("Stop before creating or uploading anything")
            return io.BytesIO(b'[{"tag_name": "v0.4.0"}]')

        self.network.side_effect = response
        with self.assertRaisesRegex(RuntimeError, "Stop before"):
            exec(PUBLISH, {})
        self.assertEqual(len(created), 1)
        self.assertEqual(created[0]["tag_name"], "app-v0.4.0")
        self.assertEqual(created[0]["name"], "YssBI 0.4.0")
        self.assertIs(created[0]["draft"], True)
        self.assertIs(created[0]["prerelease"], False)
        self.assertEqual(len((self.root / "dist/SHA256SUMS").read_text().splitlines()), 12)


if __name__ == "__main__":
    unittest.main()

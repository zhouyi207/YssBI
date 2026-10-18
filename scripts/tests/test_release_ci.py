import copy
import importlib.util
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location("release_ci", Path(__file__).resolve().parents[1] / "release_ci.py")
release_ci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release_ci)


class ReleaseCiTests(unittest.TestCase):
    def setUp(self):
        self.repository = "owner/project"
        self.commit = "a" * 40
        self.run = {
            "id": 42,
            "workflow_id": 7,
            "head_sha": self.commit,
            "event": "push",
            "head_branch": "gpui",
            "status": "completed",
            "conclusion": "success",
            "path": ".github/workflows/ci.yml",
            "repository": {"full_name": self.repository},
            "head_repository": {"full_name": self.repository},
            "run_attempt": 2,
        }
        self.jobs = [
            {"name": f"{target} / Check ({target})", "status": "completed", "conclusion": "success"}
            for target in release_ci.CHECK_TARGETS
        ]

    def find(self, runs, jobs=None):
        def request(path):
            if path == "actions/workflows/ci.yml":
                return {"id": 7}
            if path.startswith("actions/workflows/7/runs?"):
                return {"workflow_runs": runs}
            if path == "actions/runs/42/attempts/2/jobs?per_page=100&page=1":
                return {"jobs": self.jobs if jobs is None else jobs}
            raise AssertionError(f"Unexpected request: {path}")
        return release_ci.successful_ci_run(request, self.repository, self.commit)

    def test_reuses_successful_exact_commit_branch_ci_with_all_platforms(self):
        for branch in ("gpui", "main"):
            with self.subTest(branch=branch):
                self.run["head_branch"] = branch
                self.assertEqual(self.find([self.run])["id"], 42)

    def test_rejects_untrusted_or_incomplete_runs(self):
        for field, value in (
            ("head_sha", "b" * 40),
            ("event", "pull_request"),
            ("event", "workflow_dispatch"),
            ("head_branch", "feature"),
            ("status", "in_progress"),
            ("conclusion", "failure"),
            ("conclusion", "cancelled"),
            ("conclusion", "skipped"),
            ("workflow_id", 8),
            ("path", ".github/workflows/other.yml"),
            ("repository", {"full_name": "fork/project"}),
            ("head_repository", {"full_name": "fork/project"}),
        ):
            with self.subTest(field=field, value=value):
                candidate = self.run | {field: value}
                self.assertIsNone(self.find([candidate]))

    def test_missing_skipped_or_failed_platform_cannot_satisfy_gate(self):
        for index in range(len(self.jobs)):
            with self.subTest(index=index, state="missing"):
                self.assertIsNone(self.find([self.run], self.jobs[:index] + self.jobs[index + 1:]))
            for conclusion in ("skipped", "failure", "cancelled"):
                with self.subTest(index=index, state=conclusion):
                    jobs = copy.deepcopy(self.jobs)
                    jobs[index]["conclusion"] = conclusion
                    self.assertIsNone(self.find([self.run], jobs))

    def test_unrelated_job_name_does_not_satisfy_platform_gate(self):
        jobs = copy.deepcopy(self.jobs)
        jobs[0]["name"] += " optional"
        self.assertIsNone(self.find([self.run], jobs))

    def test_empty_history_requires_fresh_checks(self):
        self.assertIsNone(self.find([]))

    def test_run_and_attempt_job_pagination(self):
        def request(path):
            if path == "actions/workflows/ci.yml":
                return {"id": 7}
            if path.startswith("actions/workflows/7/runs?"):
                runs = [self.run | {"conclusion": "failure"}] * 100 if path.endswith("page=1") else [self.run]
                return {"workflow_runs": runs}
            if path.startswith("actions/runs/42/attempts/2/jobs?"):
                jobs = [{"name": "unrelated", "status": "completed", "conclusion": "success"}] * 100 if path.endswith("page=1") else self.jobs
                return {"jobs": jobs}
            raise AssertionError(f"Unexpected request: {path}")
        self.assertEqual(release_ci.successful_ci_run(request, self.repository, self.commit)["id"], 42)

    def test_api_error_cannot_authorize_reuse(self):
        def request(path):
            raise OSError("API unavailable")
        with self.assertRaises(OSError):
            release_ci.successful_ci_run(request, self.repository, self.commit)


if __name__ == "__main__":
    unittest.main()

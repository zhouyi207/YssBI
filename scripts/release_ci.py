#!/usr/bin/env python3
"""Find successful, same-commit branch CI that can satisfy release checks."""

import json
import os
from pathlib import Path
import sys
import urllib.parse
import urllib.request


CHECK_TARGETS = (
    "x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
)


def successful_ci_run(request, repository, commit):
    workflow = request("actions/workflows/ci.yml")
    query = urllib.parse.urlencode({"head_sha": commit, "event": "push", "per_page": 100})
    page = 1
    while True:
        runs = request(f"actions/workflows/{workflow['id']}/runs?{query}&page={page}")["workflow_runs"]
        for run in runs:
            if not (
                run["head_sha"] == commit
                and run["event"] == "push"
                and run["head_branch"] in ("gpui", "main")
                and run["status"] == "completed"
                and run["conclusion"] == "success"
                and run["workflow_id"] == workflow["id"]
                and run["path"] == ".github/workflows/ci.yml"
                and run["repository"]["full_name"] == repository
                and run["head_repository"]["full_name"] == repository
            ):
                continue
            # Pin the attempt so a rerun cannot mix old passing jobs with new results.
            jobs = []
            job_page = 1
            while True:
                batch = request(
                    f"actions/runs/{run['id']}/attempts/{run['run_attempt']}/jobs?per_page=100&page={job_page}"
                )["jobs"]
                jobs.extend(batch)
                if len(batch) < 100:
                    break
                job_page += 1
            if all(
                any(
                    job["name"].split(" / ")[-1] == f"Check ({target})"
                    and job["status"] == "completed"
                    and job["conclusion"] == "success"
                    for job in jobs
                )
                for target in CHECK_TARGETS
            ):
                return run
        if len(runs) < 100:
            return None
        page += 1


def main():
    repository = os.environ["GITHUB_REPOSITORY"]
    commit = os.environ["GITHUB_SHA"]
    api = os.environ["GITHUB_API_URL"].rstrip("/")
    token = os.environ["GH_TOKEN"]

    def request(path):
        request = urllib.request.Request(
            f"{api}/repos/{repository}/{path}",
            headers={
                "Authorization": f"Bearer {token}",
                "Accept": "application/vnd.github+json",
                "X-GitHub-Api-Version": "2022-11-28",
            },
        )
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)

    run = successful_ci_run(request, repository, commit)
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(f"reuse-ci={'true' if run else 'false'}\n")
    message = (
        f"Reusing successful CI for {commit}: {run['html_url']} (attempt {run['run_attempt']})."
        if run else f"No successful branch CI for {commit}; each platform will run its checks."
    )
    print(message)
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with Path(summary).open("a", encoding="utf-8") as output:
            output.write(message + "\n")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError) as error:
        print(f"CI reuse lookup failed: {error}", file=sys.stderr)
        sys.exit(1)

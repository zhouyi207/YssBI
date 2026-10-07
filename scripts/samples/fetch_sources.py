#!/usr/bin/env python3
"""Fetch the pinned CSV inputs; never run as part of application startup."""

import hashlib
import json
import pathlib
import tempfile
import urllib.request


def fetch_source(source, directory):
    name = source["sourceFile"]
    if pathlib.Path(name).name != name or not source["sourceUrl"].startswith("https://"):
        raise ValueError("Invalid sample source")
    target = directory / name
    expected = source["sourceSha256"]
    if target.is_file():
        with target.open("rb") as cached:
            if hashlib.file_digest(cached, "sha256").hexdigest() == expected:
                print(f"{name}: already verified")
                return

    request = urllib.request.Request(
        source["sourceUrl"], headers={"User-Agent": "YssBI-sample-preparation"}
    )
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=directory, delete=False) as output:
            temporary = pathlib.Path(output.name)
            digest = hashlib.sha256()
            size = 0
            with urllib.request.urlopen(request, timeout=60) as response:
                while chunk := response.read(1024 * 1024):
                    size += len(chunk)
                    if size > 64 * 1024 * 1024:
                        raise ValueError(f"{name}: source exceeds 64 MiB")
                    digest.update(chunk)
                    output.write(chunk)
        if digest.hexdigest() != expected:
            raise ValueError(f"{name}: upstream content does not match its pinned hash")
        temporary.replace(target)
        print(f"{name}: verified {size} bytes")
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def main():
    repository = pathlib.Path(__file__).resolve().parents[2]
    directory = repository / "resources/samples"
    directory.mkdir(parents=True, exist_ok=True)
    sources = json.loads((repository / "scripts/samples/sources.json").read_text())
    for source in sources:
        fetch_source(source, directory)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Read a public GitHub Actions job page without the rate-limited REST API.

Fetches https://github.com/<owner>/<repo>/actions/runs/<run>/job/<job> and pulls
the embedded JSON that backs the React view, printing step results and any
failure annotations.
"""
import json
import re
import sys
import urllib.request


def fetch(url: str) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": "bt-farm-ci-probe"})
    with urllib.request.urlopen(req, timeout=60) as response:
        return response.read().decode("utf-8", "replace")


def walk(node, hits, path=""):
    if isinstance(node, dict):
        for key, value in node.items():
            if key in ("conclusion", "status", "stepNumber", "message", "annotationLevel", "title") and isinstance(
                value, (str, int)
            ):
                hits.append((path + "/" + key, value))
            walk(value, hits, path + "/" + key)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            walk(value, hits, path + f"[{index}]")


def main() -> int:
    run_id, job_id = sys.argv[1], sys.argv[2]
    url = f"https://github.com/itriedcoding/BT-Farm/actions/runs/{run_id}/job/{job_id}"
    html = fetch(url)
    blocks = re.findall(r'<script type="application/json"[^>]*>(.*?)</script>', html, re.S)
    print(f"== {url}\nembedded json blocks: {len(blocks)}")
    for block in blocks:
        try:
            data = json.loads(block)
        except json.JSONDecodeError:
            continue
        text = json.dumps(data)
        if "conclusion" not in text and "steps" not in text:
            continue
        hits = []
        walk(data, hits)
        # Condense: only interesting values.
        seen = set()
        for path, value in hits:
            key = (path.split("/")[-1], str(value))
            if key in seen:
                continue
            seen.add(key)
            if value in ("success", "failure", "cancelled", "skipped", "completed", "queued", "in_progress"):
                print(f"  state {value:<12} at {path[-70:]}")
        for path, value in hits:
            if isinstance(value, str) and len(value) > 25 and path.endswith("message"):
                print(f"  message at {path[-60:]}:\n    {value[:2000]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

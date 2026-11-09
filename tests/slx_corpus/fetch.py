#!/usr/bin/env python3
"""Download the public SLX corpus listed in manifest.json."""

import hashlib
import json
from pathlib import Path
from urllib.parse import quote
from urllib.request import urlopen


ROOT = Path(__file__).resolve().parent
CACHE = ROOT / "cache"


def main():
    entries = json.loads((ROOT / "manifest.json").read_text(encoding="utf-8"))
    CACHE.mkdir(parents=True, exist_ok=True)
    for entry in entries:
        name = entry["repo"].replace("/", "__") + "--" + entry["path"].replace("/", "_")
        target = CACHE / name
        if target.exists():
            digest = hashlib.sha256(target.read_bytes()).hexdigest()
            if digest == entry["sha256"]:
                print(f"skip {name}")
                continue
        url = "https://raw.githubusercontent.com/{}/{}/{}".format(
            entry["repo"], entry["commit"], quote(entry["path"], safe="/")
        )
        try:
            with urlopen(url, timeout=60) as response:
                data = response.read()
        except Exception as exc:
            raise SystemExit(f"download failed for {name}: {exc}") from exc
        digest = hashlib.sha256(data).hexdigest()
        if digest != entry["sha256"]:
            target.unlink(missing_ok=True)
            raise SystemExit(f"SHA-256 mismatch for {name}: expected {entry['sha256']}, got {digest}")
        target.write_bytes(data)
        print(f"downloaded {name}")


if __name__ == "__main__":
    main()

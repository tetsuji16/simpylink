"""Shared helpers for testing generated simulation scripts."""
import csv
import json
import pathlib
import subprocess
import sys

import pytest

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
EXPECTED_DIR = REPO_ROOT / "tests" / "fixtures" / "expected"


class SimRun:
    """Result of running one generated script."""

    def __init__(self, returncode, stdout, stderr, out_dir):
        self.returncode = returncode
        self.stdout = stdout
        self.stderr = stderr
        self.out_dir = pathlib.Path(out_dir)

    def meta(self):
        with open(self.out_dir / "results.meta.json", encoding="utf-8") as f:
            return json.load(f)

    def table(self):
        """Return (header, rows) where rows are lists of floats."""
        with open(self.out_dir / "results.csv", encoding="utf-8", newline="") as f:
            reader = csv.reader(f)
            header = next(reader)
            rows = [[float(v) for v in row] for row in reader]
        return header, rows

    def column(self, name):
        header, rows = self.table()
        index = header.index(name)
        return [row[index] for row in rows]


@pytest.fixture
def run_script(tmp_path):
    """Run a script path with --out-dir and return a SimRun."""

    def _run(script_path, timeout=60):
        out_dir = tmp_path / "out"
        proc = subprocess.run(
            [sys.executable, "-u", str(script_path), "--out-dir", str(out_dir)],
            capture_output=True, text=True, timeout=timeout, cwd=tmp_path,
        )
        return SimRun(proc.returncode, proc.stdout, proc.stderr, out_dir)

    return _run

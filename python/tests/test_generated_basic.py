"""Golden generated scripts run standalone and produce the designed values (DES-03/DES-04)."""
import ast
import math
import py_compile

import pytest

from conftest import EXPECTED_DIR

CASES = {
    "gain_constant.py": ("Scope/1", [0.0, 0.5, 1.0], [6.0, 6.0, 6.0], 1e-12),
    "sum_constants.py": ("Out1", [0.0, 1.0], [2.5, 2.5], 1e-12),
    "step_gain_scope.py": ("Scope/1", [0.0, 0.5, 1.0, 1.5, 2.0], [0.0, 0.0, 6.0, 6.0, 6.0], 1e-12),
    "const_integrator.py": ("Scope/1", [0.0, 0.5, 1.0, 1.5, 2.0], [0.0, 0.5, 1.0, 1.5, 2.0], 1e-9),
    "accumulator.py": ("Scope/1", [0, 1, 2, 3, 4, 5], [0, 1, 2, 3, 4, 5], 1e-12),
}
ALLOWED_IMPORTS = {"argparse", "json", "math", "os", "sys", "traceback", "numpy", "scipy.integrate"}


def existing(name):
    path = EXPECTED_DIR / name
    if not path.exists():
        pytest.fail(f"missing golden file {path}")
    return path


@pytest.mark.parametrize("name", sorted(CASES))
def test_values(name, run_script):
    channel, times, values, tol = CASES[name]
    result = run_script(existing(name))
    assert result.returncode == 0, result.stderr
    header, rows = result.table()
    assert [r[0] for r in rows] == pytest.approx(times, abs=1e-12)
    got = result.column(channel)
    assert all(math.isclose(a, b, rel_tol=0, abs_tol=tol) for a, b in zip(got, values)), got
    meta = result.meta()
    assert meta["status"] == "ok" and meta["format_version"] == 1 and meta["num_samples"] == len(rows)
    assert f"SIMPYLINK:DONE samples={len(rows)}" in result.stdout


def test_first_order_lag_matches_analytic(run_script):
    result = run_script(existing("first_order_lag.py"))
    assert result.returncode == 0, result.stderr
    header, rows = result.table()
    for t, x in rows:
        assert abs(x - (1 - math.exp(-t))) <= 1e-4


@pytest.mark.parametrize("name", sorted(CASES) + ["first_order_lag.py"])
def test_compiles_and_is_standalone(name):
    path = existing(name)
    py_compile.compile(str(path), doraise=True)
    tree = ast.parse(path.read_text(encoding="utf-8"))
    imported = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            imported.update(a.name for a in node.names)
        elif isinstance(node, ast.ImportFrom):
            imported.add(node.module)
    assert imported <= ALLOWED_IMPORTS, imported


def test_error_is_reported(tmp_path, run_script):
    src = existing("step_gain_scope.py").read_text(encoding="utf-8")
    marker = '    """Compute every block output at time t in execution order."""\n'
    assert marker in src
    bad = tmp_path / "boom.py"
    bad.write_text(src.replace(marker, marker + '    raise RuntimeError("boom")\n', 1), encoding="utf-8")
    result = run_script(bad)
    assert result.returncode == 1
    meta = result.meta()
    assert meta["status"] == "error" and meta["error"] == "RuntimeError: boom" and meta["num_samples"] == 0
    assert "SIMPYLINK:ERROR RuntimeError: boom" in result.stderr

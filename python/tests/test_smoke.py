"""Smoke test: the helper runs a tiny script that writes the results contract."""
import textwrap


def test_run_script_reads_results(tmp_path, run_script):
    script = tmp_path / "tiny.py"
    script.write_text(textwrap.dedent('''
        import argparse, json, os
        p = argparse.ArgumentParser(); p.add_argument("--out-dir"); a = p.parse_args()
        os.makedirs(a.out_dir, exist_ok=True)
        with open(os.path.join(a.out_dir, "results.csv"), "w", encoding="utf-8", newline="\\n") as f:
            f.write("time,Scope/1\\n0.0,1.5\\n1.0,nan\\n")
        with open(os.path.join(a.out_dir, "results.meta.json"), "w", encoding="utf-8") as f:
            json.dump({"format": "simpylink-results", "status": "ok", "num_samples": 2}, f)
        print("SIMPYLINK:DONE samples=2")
    '''), encoding="utf-8")
    result = run_script(script)
    assert result.returncode == 0, result.stderr
    header, rows = result.table()
    assert header == ["time", "Scope/1"]
    assert rows[0] == [0.0, 1.5]
    assert result.column("Scope/1")[1] != result.column("Scope/1")[1]  # NaN
    assert result.meta()["status"] == "ok"
    assert "SIMPYLINK:DONE samples=2" in result.stdout

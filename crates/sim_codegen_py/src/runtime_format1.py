

# ===========================================================================
# Fixed runtime (identical in every generated file; generator format 1)
# ===========================================================================
class SimulationError(Exception):
    """Raised when the simulation cannot continue."""


def _time_tolerance():
    return 1e-9 * max(1.0, abs(START_TIME), abs(STOP_TIME))


def major_time_points():
    """Return a sorted list of (t, is_output, hit_rate_indices)."""
    tol = _time_tolerance()
    events = []
    k = 0
    while True:
        t = round(START_TIME + k * OUTPUT_STEP, 12)
        if t > STOP_TIME + tol:
            break
        events.append((t, "output", None))
        k += 1
    events.append((STOP_TIME, "output", None))
    for t in BREAKPOINTS:
        if START_TIME - tol <= t <= STOP_TIME + tol:
            events.append((t, "break", None))
    for index, (period, offset) in enumerate(DISCRETE_RATES):
        k = 0
        while True:
            t = round(offset + k * period, 12)
            if t > STOP_TIME + tol:
                break
            if t >= START_TIME - tol:
                events.append((t, "hit", index))
            k += 1
    events.sort(key=lambda e: e[0])
    points = []
    for t, kind, index in events:
        if points and abs(t - points[-1][0]) <= tol:
            t0, is_output, hits = points[-1]
        else:
            t0, is_output, hits = t, False, ()
            points.append((t0, is_output, hits))
        if kind == "output":
            is_output = True
        if kind == "hit" and index not in hits:
            hits = hits + (index,)
        points[-1] = (t0, is_output, hits)
    return points


def simulate():
    """Run the simulation and return (times, values) as NumPy arrays."""
    x = np.array(X0, dtype=float)
    d = np.array(D0, dtype=float)
    times = []
    rows = []
    points = major_time_points()
    for i, (t, is_output, hits) in enumerate(points):
        y = compute_outputs(t, x, d, t)
        if is_output:
            times.append(t)
            rows.append(log_values(y))
        if hits:
            d = update_discrete_states(t, x, d, y, hits)
        if i + 1 < len(points) and NUM_CONT_STATES > 0:
            t_next = points[i + 1][0]
            d_hold = d
            sol = solve_ivp(
                lambda tt, xx: compute_derivatives(tt, xx, d_hold, t),
                (t, t_next),
                x,
                method=SOLVER,
                rtol=REL_TOL,
                atol=ABS_TOL,
                max_step=MAX_STEP,
            )
            if not sol.success:
                raise SimulationError(
                    f"solver failed between t={t!r} and t={t_next!r}: {sol.message}"
                )
            x = sol.y[:, -1].copy()
    return np.array(times, dtype=float), np.array(rows, dtype=float).reshape(len(times), len(LOG_CHANNELS))


def _format_number(value):
    return repr(float(value))


def write_results(out_dir, times, values, status="ok", error=None):
    """Write results.csv and results.meta.json into out_dir."""
    os.makedirs(out_dir, exist_ok=True)
    csv_path = os.path.join(out_dir, "results.csv")
    with open(csv_path, "w", encoding="utf-8", newline="\n") as f:
        f.write(",".join(["time"] + [c["name"] for c in LOG_CHANNELS]) + "\n")
        for t, row in zip(times, values):
            f.write(",".join(_format_number(v) for v in [t, *row]) + "\n")
    meta = {
        "format": "simpylink-results",
        "format_version": 1,
        "model": MODEL_NAME,
        "status": status,
        "error": error,
        "num_samples": len(times),
        "channels": LOG_CHANNELS,
    }
    with open(os.path.join(out_dir, "results.meta.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump(meta, f, indent=2)
        f.write("\n")


def main(argv=None):
    parser = argparse.ArgumentParser(description=f"Simulate model {MODEL_NAME!r}.")
    parser.add_argument("--out-dir", default="results", help="directory for results.csv")
    args = parser.parse_args(argv)
    print("SIMPYLINK:START", flush=True)
    try:
        times, values = simulate()
    except Exception as exc:  # report every failure to the caller
        write_results(args.out_dir, [], [], status="error", error=f"{type(exc).__name__}: {exc}")
        print(f"SIMPYLINK:ERROR {type(exc).__name__}: {exc}", file=sys.stderr, flush=True)
        traceback.print_exc()
        return 1
    write_results(args.out_dir, times, values)
    print(f"SIMPYLINK:DONE samples={len(times)}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())

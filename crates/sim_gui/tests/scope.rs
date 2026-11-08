//! Scope series extraction and autoscale (Issue #68).
use sim_core::ids::BlockId;
use sim_gui::scope::{autoscale_bounds, scope_series, ScopeSeries};
use sim_runner::{Channel, SimResults};

fn results(time: Vec<f64>, name: &str, sid: u32, port: u32, values: Vec<f64>) -> SimResults {
    SimResults {
        model: "m".into(),
        time,
        channels: vec![Channel {
            name: name.into(),
            block_sid: sid,
            port,
            values,
        }],
    }
}

#[test]
fn scope_series_uses_channel_name_and_port_order() {
    let r = results(
        vec![0.0, 0.5, 1.0, 1.5, 2.0],
        "Scope/1",
        3,
        1,
        vec![0.0, 0.0, 6.0, 6.0, 6.0],
    );
    assert_eq!(
        scope_series(&r, BlockId(3)),
        vec![ScopeSeries {
            label: "Scope/1".into(),
            points: vec![[0.0, 0.0], [0.5, 0.0], [1.0, 6.0], [1.5, 6.0], [2.0, 6.0]],
        }]
    );
    assert!(scope_series(&r, BlockId(9)).is_empty());
}

#[test]
fn scope_series_sorts_multiple_ports() {
    let mut r = results(vec![0.0, 1.0], "Scope/2", 3, 2, vec![2.0, 3.0]);
    r.channels.push(Channel {
        name: "Scope/1".into(),
        block_sid: 3,
        port: 1,
        values: vec![0.0, 1.0],
    });
    let series = scope_series(&r, BlockId(3));
    assert_eq!(
        series.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(),
        vec!["Scope/1", "Scope/2"]
    );
}

#[test]
fn autoscale_adds_five_percent_y_margin() {
    let r = results(
        vec![0.0, 0.5, 1.0, 1.5, 2.0],
        "Scope/1",
        3,
        1,
        vec![0.0, 0.0, 6.0, 6.0, 6.0],
    );
    let (x0, x1, y0, y1) = autoscale_bounds(&scope_series(&r, BlockId(3))).expect("bounds");
    assert!((x0 - 0.0).abs() < 1e-12);
    assert!((x1 - 2.0).abs() < 1e-12);
    assert!((y0 - -0.3).abs() < 1e-12, "y0 = {y0}");
    assert!((y1 - 6.3).abs() < 1e-12, "y1 = {y1}");
}

#[test]
fn autoscale_constant_data_uses_plus_minus_one() {
    let r = results(vec![0.0, 0.5, 1.0], "Scope/1", 3, 1, vec![2.0, 2.0, 2.0]);
    let (x0, x1, y0, y1) = autoscale_bounds(&scope_series(&r, BlockId(3))).expect("bounds");
    assert!((x0 - 0.0).abs() < 1e-12);
    assert!((x1 - 1.0).abs() < 1e-12);
    assert!((y0 - 1.0).abs() < 1e-12);
    assert!((y1 - 3.0).abs() < 1e-12);
}

#[test]
fn scope_series_drops_non_finite_points() {
    let r = results(
        vec![0.0, 0.5, 1.0],
        "Scope/1",
        3,
        1,
        vec![0.0, f64::NAN, 1.0],
    );
    let series = scope_series(&r, BlockId(3));
    assert_eq!(
        series[0].points,
        vec![[0.0, 0.0], [1.0, 1.0]],
        "NaN points are skipped"
    );
}

#[test]
fn autoscale_without_points_is_none() {
    assert_eq!(autoscale_bounds(&[]), None);
    let empty = vec![ScopeSeries {
        label: "Scope/1".into(),
        points: vec![],
    }];
    assert_eq!(autoscale_bounds(&empty), None);
}

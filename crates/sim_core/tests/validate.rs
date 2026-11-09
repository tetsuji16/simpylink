//! Structural model validation tests (Issue #37, DES-02 §6).
use sim_core::diag::codes::*;
use sim_core::diag::Severity;
use sim_core::geom::Point;
use sim_core::ids::{BlockId, LineId};
use sim_core::json::load_from_path;
use sim_core::library::new_block;
use sim_core::model::*;
use sim_core::validate::validate_model;
use std::path::PathBuf;

fn fixture(name: &str) -> Model {
    load_from_path(&PathBuf::from(format!(
        "{}/../../tests/fixtures/models/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    )))
    .unwrap()
}

fn codes(model: &Model) -> Vec<String> {
    validate_model(model).into_iter().map(|d| d.code).collect()
}

fn pr(block: u32, port: u32) -> PortRef {
    PortRef {
        block: BlockId(block),
        port,
    }
}

fn line(id: u32, src: PortRef, dst: Option<PortRef>) -> Line {
    Line {
        id: LineId(id),
        src,
        points: vec![],
        dst,
        branches: vec![],
        name: String::new(),
    }
}

/// Constant(1) → Sum(2, "+-").1 and Sum(2) → Scope(3).1; Sum.2 is unconnected.
fn partially_connected_sum() -> Model {
    let mut model = Model::new("partial");
    model.config.stop_time = 1.0;
    let constant = new_block(&model.root, "Constant", BlockId(1), Point::new(0.0, 0.0)).unwrap();
    model.root.insert_block_sorted(constant);
    let mut sum = new_block(&model.root, "Sum", BlockId(2), Point::new(50.0, 0.0)).unwrap();
    sum.params.insert("Inputs".into(), "+-".into());
    model.root.insert_block_sorted(sum);
    let scope = new_block(&model.root, "Scope", BlockId(3), Point::new(100.0, 0.0)).unwrap();
    model.root.insert_block_sorted(scope);
    model.root.lines = vec![
        line(1, pr(1, 1), Some(pr(2, 1))),
        line(2, pr(2, 1), Some(pr(3, 1))),
    ];
    model
}

#[test]
fn fixture_is_clean() {
    assert!(validate_model(&fixture("step_gain_scope")).is_empty());
}

#[test]
fn unconnected_input_is_a_warning() {
    let diagnostics = validate_model(&partially_connected_sum());
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, W_UNCONNECTED_INPUT);
    assert_eq!(diagnostics[0].severity, Severity::Warning);
    assert_eq!(diagnostics[0].location.block, Some(BlockId(2)));
    assert_eq!(
        diagnostics[0].message,
        "input port 2 of 'Sum' is not connected; it will read 0"
    );
}

#[test]
fn branched_line_drives_both_destinations() {
    let mut model = fixture("step_gain_scope");
    let scope2 = new_block(&model.root, "Scope", BlockId(4), Point::new(300.0, 50.0)).unwrap();
    model.root.insert_block_sorted(scope2);
    model.root.lines[0].branches.push(Branch {
        tap: TapPoint { segment: 0, t: 0.5 },
        points: vec![],
        dst: Some(pr(3, 1)),
        branches: vec![],
    });
    model.root.lines[1].dst = Some(pr(4, 1));
    assert!(
        validate_model(&model).is_empty(),
        "{:?}",
        validate_model(&model)
    );
}

#[test]
fn invalid_config_values_are_all_reported() {
    let mut model = fixture("step_gain_scope");
    model.config.stop_time = 0.0;
    assert_eq!(codes(&model), vec![E_INVALID_CONFIG]);

    let mut model = fixture("step_gain_scope");
    model.config.output_step = -1.0;
    model.config.rel_tol = 0.0;
    let diagnostics = validate_model(&model);
    assert_eq!(codes(&model), vec![E_INVALID_CONFIG, E_INVALID_CONFIG]);
    assert_eq!(diagnostics[0].message, "output_step must be positive");
    assert_eq!(diagnostics[1].message, "rel_tol must be positive");
}

#[test]
fn duplicate_sid_is_reported_first() {
    let mut model = fixture("step_gain_scope");
    model.root.blocks[1].sid = BlockId(1);
    let diagnostics = validate_model(&model);
    assert_eq!(diagnostics[0].code, E_DUPLICATE_SID);
    assert_eq!(diagnostics[0].location.block, Some(BlockId(1)));
}

#[test]
fn duplicate_block_name_is_reported() {
    let mut model = fixture("step_gain_scope");
    model.root.blocks[1].name = "Step".into();
    assert!(codes(&model).contains(&E_DUPLICATE_BLOCK_NAME.to_string()));
}

#[test]
fn unknown_block_type_skips_parameter_checks() {
    let mut model = fixture("step_gain_scope");
    model.root.blocks[1].block_type = "Foo".into();
    let diagnostics = validate_model(&model);
    assert!(diagnostics.iter().any(|d| d.code == E_UNKNOWN_BLOCK_TYPE));
    assert!(!diagnostics
        .iter()
        .any(|d| d.code == E_INVALID_PARAM && d.location.block == Some(BlockId(2))));
}

#[test]
fn missing_and_invalid_parameters_are_reported() {
    let mut model = fixture("step_gain_scope");
    model.root.blocks[1].params.remove("Gain");
    let diagnostics = validate_model(&model);
    assert_eq!(diagnostics[0].code, E_INVALID_PARAM);
    assert_eq!(diagnostics[0].message, "parameter 'Gain' is missing");

    let mut model = fixture("step_gain_scope");
    model.root.blocks[1]
        .params
        .insert("Gain".into(), "[1 2]".into());
    let diagnostics = validate_model(&model);
    assert_eq!(diagnostics[0].code, E_INVALID_PARAM);
    assert_eq!(
        diagnostics[0].message,
        "parameter 'Gain': expected a scalar"
    );
}

#[test]
fn missing_source_block_is_reported() {
    let mut model = fixture("step_gain_scope");
    model.root.lines[1].src.block = BlockId(99);
    assert!(codes(&model).contains(&E_LINE_SRC_MISSING.to_string()));
}

#[test]
fn out_of_range_destination_port_leaves_input_unconnected() {
    let mut model = fixture("step_gain_scope");
    model.root.lines[0].dst = Some(pr(2, 2));
    let diagnostics = validate_model(&model);
    assert!(diagnostics.iter().any(|d| d.code == E_PORT_OUT_OF_RANGE));
    assert!(diagnostics.iter().any(|d| {
        d.code == W_UNCONNECTED_INPUT
            && d.location.block == Some(BlockId(2))
            && d.message == "input port 1 of 'Gain' is not connected; it will read 0"
    }));
}

#[test]
fn multiply_driven_input_is_reported() {
    let mut model = fixture("step_gain_scope");
    let mut duplicate = model.root.lines[0].clone();
    duplicate.id = LineId(3);
    model.root.lines.push(duplicate);
    assert!(codes(&model).contains(&E_INPUT_MULTIPLY_DRIVEN.to_string()));
}

#[test]
fn dangling_line_and_unconnected_scope_are_reported() {
    let mut model = fixture("step_gain_scope");
    model.root.lines[1].dst = None;
    let diagnostics = validate_model(&model);
    assert!(diagnostics.iter().any(|d| d.code == W_DANGLING_LINE));
    assert!(diagnostics
        .iter()
        .any(|d| { d.code == W_UNCONNECTED_INPUT && d.location.block == Some(BlockId(3)) }));
}

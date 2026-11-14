use sim_codegen_py::{generate, CodegenOptions};
use sim_core::diag::codes::{E_DUPLICATE_GOTO_TAG, E_INVALID_PARAM, E_UNMATCHED_FROM};
use sim_core::json::load_from_path;
use sim_core::validate::validate_model;
use sim_integration::e2e::{assert_close, simulate_model};
use std::path::PathBuf;

fn fixture() -> sim_core::model::Model {
    load_from_path(&PathBuf::from(format!(
        "{}/../fixtures/models/goto_from.json",
        env!("CARGO_MANIFEST_DIR")
    )))
    .unwrap()
}

#[test]
fn local_and_global_tags_route_signals() {
    let model = fixture();
    let result = simulate_model(&model).unwrap();
    assert_close(&result.time, &[0.0, 0.5, 1.0, 1.5, 2.0], 1e-12);
    assert_close(
        &result.channel("Scope/1").unwrap().values,
        &[0.0, 0.0, 6.0, 6.0, 6.0],
        1e-12,
    );
    assert_close(&result.channel("Scope/2").unwrap().values, &[5.0; 5], 1e-12);
    let compiled = sim_ir::compile(&model).unwrap();
    let generated = generate(&compiled.ir, &CodegenOptions::default()).unwrap();
    assert!(compiled
        .ir
        .blocks
        .iter()
        .all(|b| b.block_type != "Goto" && b.block_type != "From"));
    let source = generated.source.to_lowercase();
    for name in ["gotolocal", "fromlocal", "gotoglobal", "fromglobal"] {
        assert!(!source.contains(name));
    }
}

#[test]
fn invalid_tag_cases_are_diagnosed() {
    let mut model = fixture();
    model.root.blocks[4]
        .params
        .insert("GotoTag".into(), "missing".into());
    assert!(validate_model(&model)
        .iter()
        .any(|d| d.code == E_UNMATCHED_FROM));
    let mut model = fixture();
    let mut duplicate = model.root.blocks[3].clone();
    duplicate.sid = sim_core::ids::BlockId(11);
    duplicate.name = "GotoDuplicate".into();
    model.root.blocks.push(duplicate);
    assert!(validate_model(&model)
        .iter()
        .any(|d| d.code == E_DUPLICATE_GOTO_TAG));
    let mut model = fixture();
    let mut duplicate = model.root.blocks[7].clone();
    duplicate.sid = sim_core::ids::BlockId(11);
    duplicate.name = "GotoGlobalDuplicate".into();
    model.root.blocks.push(duplicate);
    assert!(validate_model(&model)
        .iter()
        .any(|d| d.code == E_DUPLICATE_GOTO_TAG));
    let mut model = fixture();
    model.root.blocks[3]
        .params
        .insert("TagVisibility".into(), "scoped".into());
    assert!(validate_model(&model)
        .iter()
        .any(|d| d.code == E_INVALID_PARAM));
    let mut model = fixture();
    let child = model.root.blocks[6].system.as_mut().unwrap();
    child.blocks[0].params.insert("GotoTag".into(), "u".into());
    assert!(validate_model(&model)
        .iter()
        .any(|d| d.code == E_UNMATCHED_FROM));
}

#[test]
fn scoped_tag_is_visible_below_its_marker() {
    let mut model = fixture();
    model.root.blocks[7]
        .params
        .insert("TagVisibility".into(), "scoped".into());
    let marker = sim_core::library::new_block(
        &model.root,
        "GotoTagVisibility",
        sim_core::ids::BlockId(11),
        sim_core::geom::Point::new(0.0, 0.0),
    )
    .unwrap();
    let mut marker = marker;
    marker.params.insert("GotoTag".into(), "g".into());
    model.root.blocks.push(marker);
    model.next_sid = 12;
    let result = simulate_model(&model).unwrap();
    assert_close(&result.channel("Scope/2").unwrap().values, &[5.0; 5], 1e-12);
    let compiled = sim_ir::compile(&model).unwrap();
    assert!(compiled
        .ir
        .blocks
        .iter()
        .all(|b| b.block_type != "GotoTagVisibility"));

    // An atomic subsystem is a visibility boundary.
    model.root.blocks[6]
        .params
        .insert("TreatAsAtomicUnit".into(), "on".into());
    assert!(validate_model(&model)
        .iter()
        .any(|d| d.code == E_UNMATCHED_FROM));

    model.root.blocks[6].params.clear();
    let mut duplicate = model.root.blocks[7].clone();
    duplicate.sid = sim_core::ids::BlockId(12);
    duplicate.name = "DuplicateScoped".into();
    model.root.blocks.push(duplicate);
    assert!(validate_model(&model)
        .iter()
        .any(|d| d.code == E_DUPLICATE_GOTO_TAG));
}

#[test]
fn scoped_marker_does_not_expose_tags_outside_its_subsystem() {
    let mut model = fixture();
    model.root.blocks[7]
        .params
        .insert("TagVisibility".into(), "scoped".into());
    let mut marker = sim_core::library::new_block(
        &model.root,
        "GotoTagVisibility",
        sim_core::ids::BlockId(11),
        sim_core::geom::Point::new(0.0, 0.0),
    )
    .unwrap();
    marker.params.insert("GotoTag".into(), "g".into());
    model.root.blocks[6]
        .system
        .as_mut()
        .unwrap()
        .blocks
        .push(marker);
    let diagnostics = validate_model(&model);
    assert!(diagnostics.iter().any(|d| d.code == E_INVALID_PARAM));
    assert!(diagnostics.iter().any(|d| d.code == E_UNMATCHED_FROM));
}

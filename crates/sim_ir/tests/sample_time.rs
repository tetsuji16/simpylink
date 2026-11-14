//! Sample-time resolution tests (Issue #72, DES-03 §6).
use sim_core::ids::BlockId;
use sim_ir::sample_time::{discrete_rates, rate_index, resolve_sample_times, StInput};
use sim_ir::{compile, SampleTime};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn input(sid: u32, declared: Option<SampleTime>, needs_discrete: bool, drivers: &[u32]) -> StInput {
    StInput {
        sid: BlockId(sid),
        name: format!("b{sid}"),
        declared,
        needs_discrete,
        drivers: drivers.iter().map(|d| BlockId(*d)).collect(),
    }
}

const D05: SampleTime = SampleTime::Discrete {
    period: 0.5,
    offset: 0.0,
};

#[test]
fn resolves_declared_inherited_and_constant_blocks() {
    let resolved = resolve_sample_times(&[
        input(1, Some(SampleTime::Constant), false, &[]),
        input(2, None, false, &[1]),
    ])
    .unwrap();
    assert_eq!(resolved[&BlockId(1)], SampleTime::Constant);
    assert_eq!(resolved[&BlockId(2)], SampleTime::Constant);

    let resolved = resolve_sample_times(&[
        input(1, Some(SampleTime::Continuous), false, &[]),
        input(2, None, false, &[1]),
        input(3, None, false, &[2]),
    ])
    .unwrap();
    assert_eq!(resolved[&BlockId(1)], SampleTime::Continuous);
    assert_eq!(resolved[&BlockId(2)], SampleTime::Continuous);
    assert_eq!(resolved[&BlockId(3)], SampleTime::Continuous);

    let resolved =
        resolve_sample_times(&[input(1, Some(D05), true, &[]), input(2, None, false, &[1])])
            .unwrap();
    assert_eq!(resolved[&BlockId(2)], D05);

    let resolved = resolve_sample_times(&[
        input(1, Some(SampleTime::Constant), false, &[]),
        input(2, Some(D05), true, &[]),
        input(3, None, false, &[1, 2]),
    ])
    .unwrap();
    assert_eq!(resolved[&BlockId(3)], D05);

    let resolved = resolve_sample_times(&[input(1, None, false, &[])]).unwrap();
    assert_eq!(resolved[&BlockId(1)], SampleTime::Constant);
}

#[test]
fn rejects_multirate_and_undetermined_discrete_blocks() {
    let error = resolve_sample_times(&[
        input(1, Some(D05), true, &[]),
        input(
            2,
            Some(SampleTime::Discrete {
                period: 0.2,
                offset: 0.0,
            }),
            true,
            &[],
        ),
        input(3, None, false, &[1, 2]),
    ])
    .unwrap_err();
    assert_eq!(
        error,
        vec![(BlockId(3), "multirate is not supported".to_string())]
    );

    let error = resolve_sample_times(&[input(1, None, true, &[])]).unwrap_err();
    assert_eq!(error[0].0, BlockId(1));
    assert!(error[0].1.contains("needs a discrete sample time"));

    let error = resolve_sample_times(&[
        input(1, Some(SampleTime::Constant), false, &[]),
        input(2, None, false, &[1, 3]),
        input(3, None, true, &[2]),
    ])
    .unwrap_err();
    assert_eq!(error[0].0, BlockId(3));
}

#[test]
fn discrete_rates_are_sorted_and_deduplicated() {
    let resolved = BTreeMap::from([
        (BlockId(1), D05),
        (
            BlockId(2),
            SampleTime::Discrete {
                period: 0.1,
                offset: 0.0,
            },
        ),
        (BlockId(3), D05),
        (BlockId(4), SampleTime::Continuous),
    ]);
    let rates = discrete_rates(&resolved);
    assert_eq!(rates, vec![(0.1, 0.0), (0.5, 0.0)]);
    assert_eq!(rate_index(&rates, D05), Some(1));
    assert_eq!(rate_index(&rates, SampleTime::Continuous), None);
}

#[test]
fn compile_uses_resolved_sample_times() {
    let path = PathBuf::from(format!(
        "{}/../../tests/fixtures/models/step_gain_scope.json",
        env!("CARGO_MANIFEST_DIR")
    ));
    let model = sim_core::json::load_from_path(&path).unwrap();
    let compiled = compile(&model).unwrap();
    assert!(compiled
        .ir
        .blocks
        .iter()
        .all(|b| b.sample_time == SampleTime::Continuous));
    assert!(compiled.ir.discrete_rates.is_empty());

    let path = PathBuf::from(format!(
        "{}/../../tests/fixtures/models/gain_constant.json",
        env!("CARGO_MANIFEST_DIR")
    ));
    let model = sim_core::json::load_from_path(&path).unwrap();
    let compiled = compile(&model).unwrap();
    assert_eq!(compiled.ir.blocks[1].sample_time, SampleTime::Constant);
}

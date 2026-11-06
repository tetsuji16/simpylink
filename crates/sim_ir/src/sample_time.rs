//! Sample time resolution (DES-03 §6).
use crate::types::SampleTime;
use sim_core::ids::BlockId;
use std::collections::BTreeMap;

/// Input for resolution.
#[derive(Debug, Clone)]
pub struct StInput {
    pub sid: BlockId,
    pub name: String,
    pub declared: Option<SampleTime>,
    pub needs_discrete: bool,
    pub drivers: Vec<BlockId>,
}

const EPS: f64 = 1e-12;

fn same(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() <= EPS && (a.1 - b.1).abs() <= EPS
}

fn combine(sts: &[SampleTime]) -> Result<SampleTime, String> {
    if sts.is_empty() {
        return Ok(SampleTime::Constant);
    }
    if sts.contains(&SampleTime::Continuous) {
        return Ok(SampleTime::Continuous);
    }
    let mut disc: Vec<(f64, f64)> = vec![];
    for s in sts {
        if let SampleTime::Discrete { period, offset } = s {
            if !disc.iter().any(|d| same(*d, (*period, *offset))) {
                disc.push((*period, *offset));
            }
        }
    }
    match disc.len() {
        0 => Ok(SampleTime::Constant),
        1 => Ok(SampleTime::Discrete {
            period: disc[0].0,
            offset: disc[0].1,
        }),
        _ => Err("multirate is not supported".into()),
    }
}

/// Resolves all sample times or returns (sid, message) errors.
pub fn resolve_sample_times(
    blocks: &[StInput],
) -> Result<BTreeMap<BlockId, SampleTime>, Vec<(BlockId, String)>> {
    let mut res: BTreeMap<BlockId, SampleTime> = BTreeMap::new();
    let mut errs = vec![];
    for b in blocks {
        if let Some(d) = b.declared {
            res.insert(b.sid, d);
        }
    }
    for _ in 0..=blocks.len() {
        let mut progress = false;
        for b in blocks {
            if res.contains_key(&b.sid) || errs.iter().any(|(s, _)| *s == b.sid) {
                continue;
            }
            if b.drivers.iter().all(|d| res.contains_key(d)) {
                let sts: Vec<SampleTime> = b.drivers.iter().map(|d| res[d]).collect();
                match combine(&sts) {
                    Ok(st) => {
                        res.insert(b.sid, st);
                    }
                    Err(e) => errs.push((b.sid, e)),
                }
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }
    let msg = |n: &str| format!("'{n}' needs a discrete sample time; set SampleTime");
    for b in blocks {
        if errs.iter().any(|(s, _)| *s == b.sid) {
            continue;
        }
        match res.get(&b.sid) {
            None if b.needs_discrete => errs.push((b.sid, msg(&b.name))),
            None => {
                res.insert(b.sid, SampleTime::Continuous);
            }
            Some(SampleTime::Discrete { .. }) => {}
            Some(_) if b.needs_discrete => errs.push((b.sid, msg(&b.name))),
            Some(_) => {}
        }
    }
    if errs.is_empty() {
        Ok(res)
    } else {
        errs.sort_by_key(|e| e.0);
        Err(errs)
    }
}

/// Distinct discrete rates sorted by period then offset.
pub fn discrete_rates(resolved: &BTreeMap<BlockId, SampleTime>) -> Vec<(f64, f64)> {
    let mut v: Vec<(f64, f64)> = vec![];
    for st in resolved.values() {
        if let SampleTime::Discrete { period, offset } = st {
            if !v.iter().any(|d| same(*d, (*period, *offset))) {
                v.push((*period, *offset));
            }
        }
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v
}

/// Index of a discrete rate.
pub fn rate_index(rates: &[(f64, f64)], st: SampleTime) -> Option<usize> {
    match st {
        SampleTime::Discrete { period, offset } => {
            rates.iter().position(|r| same(*r, (period, offset)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inp(sid: u32, declared: Option<SampleTime>, needs: bool, drivers: &[u32]) -> StInput {
        StInput {
            sid: BlockId(sid),
            name: format!("b{sid}"),
            declared,
            needs_discrete: needs,
            drivers: drivers.iter().map(|d| BlockId(*d)).collect(),
        }
    }
    const D05: SampleTime = SampleTime::Discrete {
        period: 0.5,
        offset: 0.0,
    };
    #[test]
    fn resolution() {
        let r = resolve_sample_times(&[
            inp(1, Some(SampleTime::Constant), false, &[]),
            inp(2, None, false, &[1]),
        ])
        .unwrap();
        assert_eq!(r[&BlockId(2)], SampleTime::Constant);
        let r = resolve_sample_times(&[
            inp(1, Some(SampleTime::Continuous), false, &[]),
            inp(2, None, false, &[1]),
            inp(3, None, false, &[2]),
        ])
        .unwrap();
        assert_eq!(r[&BlockId(3)], SampleTime::Continuous);
        let r = resolve_sample_times(&[inp(1, Some(D05), true, &[]), inp(2, None, false, &[1])])
            .unwrap();
        assert_eq!(r[&BlockId(2)], D05);
        let r = resolve_sample_times(&[
            inp(1, Some(SampleTime::Constant), false, &[]),
            inp(2, Some(D05), true, &[]),
            inp(3, None, false, &[1, 2]),
        ])
        .unwrap();
        assert_eq!(r[&BlockId(3)], D05);
        let e = resolve_sample_times(&[
            inp(1, Some(D05), true, &[]),
            inp(
                2,
                Some(SampleTime::Discrete {
                    period: 0.2,
                    offset: 0.0,
                }),
                true,
                &[],
            ),
            inp(3, None, false, &[1, 2]),
        ])
        .unwrap_err();
        assert_eq!(
            e,
            vec![(BlockId(3), "multirate is not supported".to_string())]
        );
        let e = resolve_sample_times(&[inp(1, None, true, &[])]).unwrap_err();
        assert!(e[0].1.contains("needs a discrete sample time"));
        let e = resolve_sample_times(&[
            inp(1, Some(SampleTime::Constant), false, &[]),
            inp(2, None, false, &[1, 3]),
            inp(3, None, true, &[2]),
        ])
        .unwrap_err();
        assert_eq!(e[0].0, BlockId(3));
        let r = resolve_sample_times(&[inp(1, None, false, &[])]).unwrap();
        assert_eq!(r[&BlockId(1)], SampleTime::Constant);
        let m = BTreeMap::from([
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
        let rates = discrete_rates(&m);
        assert_eq!(rates, vec![(0.1, 0.0), (0.5, 0.0)]);
        assert_eq!(rate_index(&rates, D05), Some(1));
    }
}

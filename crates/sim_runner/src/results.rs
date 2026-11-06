//! results.csv + results.meta.json loader.
use crate::types::*;
use serde::Deserialize;
use std::path::Path;

/// results.meta.json.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ResultsMeta {
    pub format: String,
    pub format_version: u32,
    pub model: String,
    pub status: String,
    pub error: Option<String>,
    pub num_samples: usize,
    pub channels: Vec<MetaChannel>,
}

/// Channel metadata.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MetaChannel {
    pub name: String,
    pub block_sid: u32,
    pub port: u32,
}

/// Parses a number written by Python repr().
pub fn parse_number(s: &str) -> Option<f64> {
    match s {
        "nan" => Some(f64::NAN),
        "inf" => Some(f64::INFINITY),
        "-inf" => Some(f64::NEG_INFINITY),
        _ => s.parse().ok(),
    }
}

/// Parses meta JSON.
pub fn parse_meta(text: &str) -> Result<ResultsMeta, ResultsError> {
    let m: ResultsMeta =
        serde_json::from_str(text).map_err(|e| ResultsError::Meta(e.to_string()))?;
    if m.format != "simpylink-results" || m.format_version != 1 {
        return Err(ResultsError::Meta("unsupported results format".into()));
    }
    Ok(m)
}

/// Parses the CSV into header and rows.
pub fn parse_csv(text: &str) -> Result<(Vec<String>, Vec<Vec<f64>>), ResultsError> {
    let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).collect();
    let n = if lines.last() == Some(&"") {
        lines.len() - 1
    } else {
        lines.len()
    };
    if n == 0 {
        return Err(ResultsError::Csv {
            line: 1,
            message: "empty file".into(),
        });
    }
    let header: Vec<String> = lines[0].split(',').map(str::to_string).collect();
    let mut rows = vec![];
    for (i, l) in lines[1..n].iter().enumerate() {
        let lineno = i + 2;
        if l.is_empty() {
            return Err(ResultsError::Csv {
                line: lineno,
                message: "empty line".into(),
            });
        }
        let cells: Vec<&str> = l.split(',').collect();
        if cells.len() != header.len() {
            return Err(ResultsError::Csv {
                line: lineno,
                message: format!("expected {} columns, found {}", header.len(), cells.len()),
            });
        }
        let mut row = vec![];
        for c in cells {
            row.push(parse_number(c).ok_or_else(|| ResultsError::Csv {
                line: lineno,
                message: format!("invalid number '{c}'"),
            })?);
        }
        rows.push(row);
    }
    Ok((header, rows))
}

/// Loads and validates the result files in `out_dir`.
pub fn load_results(out_dir: &Path) -> Result<SimResults, ResultsError> {
    let meta_text = std::fs::read_to_string(out_dir.join("results.meta.json"))
        .map_err(|_| ResultsError::Missing("results.meta.json".into()))?;
    let meta = parse_meta(&meta_text)?;
    if meta.status == "error" {
        return Err(ResultsError::Reported(
            meta.error.unwrap_or_else(|| "unknown error".into()),
        ));
    }
    if meta.status != "ok" {
        return Err(ResultsError::Meta("unknown status".into()));
    }
    let csv = std::fs::read_to_string(out_dir.join("results.csv"))
        .map_err(|_| ResultsError::Missing("results.csv".into()))?;
    let (header, rows) = parse_csv(&csv)?;
    if header.first().map(String::as_str) != Some("time") {
        return Err(ResultsError::Csv {
            line: 1,
            message: "first column must be 'time'".into(),
        });
    }
    let names: Vec<&str> = meta.channels.iter().map(|c| c.name.as_str()).collect();
    if header[1..].iter().map(String::as_str).collect::<Vec<_>>() != names {
        return Err(ResultsError::Csv {
            line: 1,
            message: "header does not match channels".into(),
        });
    }
    if rows.len() != meta.num_samples {
        return Err(ResultsError::Meta(format!(
            "num_samples is {} but CSV has {} rows",
            meta.num_samples,
            rows.len()
        )));
    }
    let time = rows.iter().map(|r| r[0]).collect();
    let channels = meta
        .channels
        .iter()
        .enumerate()
        .map(|(i, c)| Channel {
            name: c.name.clone(),
            block_sid: c.block_sid,
            port: c.port,
            values: rows.iter().map(|r| r[i + 1]).collect(),
        })
        .collect();
    Ok(SimResults {
        model: meta.model,
        time,
        channels,
    })
}

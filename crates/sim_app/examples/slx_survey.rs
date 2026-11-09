use serde_json::{json, Value};
use sim_core::validate::validate_model;
use sim_ir::compile::compile;
use sim_slx::opc::read_package;
use sim_slx::xml::parse_xml;
use sim_slx::{read_slx_bytes, write_slx_bytes};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

fn package_p_count(bytes: &[u8]) -> usize {
    read_package(bytes)
        .ok()
        .map(|pkg| {
            pkg.parts
                .iter()
                .filter(|p| {
                    p.name.starts_with("simulink/systems/") || p.name == "simulink/blockdiagram.xml"
                })
                .map(|p| String::from_utf8_lossy(&p.data).matches("<P ").count())
                .sum()
        })
        .unwrap_or(0)
}

fn version(path: &Path) -> String {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| read_package(&bytes).ok())
        .and_then(|imp| {
            imp.part("metadata/coreProperties.xml")
                .and_then(|p| parse_xml("metadata/coreProperties.xml", &p.data).ok())
                .and_then(|d| {
                    fn find(e: &sim_slx::xml::XmlElement) -> Option<String> {
                        if e.name.ends_with("version") {
                            return Some(e.text());
                        }
                        e.elements().find_map(find)
                    }
                    d.root().and_then(find)
                })
        })
        .unwrap_or_else(|| "?".into())
}

fn read_error(file: &str, version: &str, message: String) -> Value {
    json!({"file":file,"version":version,"read":message,"blocks":0,"lines":0,"unsupported":0,"unsupported_types":[],"dropped":0,"warnings":0,"validate":{},"compile":"skipped","compile_errors":[],"roundtrip":{"parts_changed":0,"parts_lost":0,"P_before":0,"P_after":0,"reread_equal":false}})
}

fn survey(path: &Path) -> Value {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let version = version(path);
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return read_error(&name, &version, format!("ERR {e}")),
    };
    let result = catch_unwind(AssertUnwindSafe(|| read_slx_bytes(&bytes, &name)));
    let imported = match result {
        Ok(Ok(i)) => i,
        Ok(Err(e)) => return read_error(&name, &version, format!("ERR {e}")),
        Err(_) => return read_error(&name, &version, "PANIC".into()),
    };
    let mut blocks = 0;
    fn count(s: &sim_core::model::System, n: &mut usize) {
        *n += s.blocks.len();
        for b in &s.blocks {
            if let Some(c) = &b.system {
                count(c, n);
            }
        }
    }
    fn line_count(s: &sim_core::model::System) -> usize {
        s.lines.len()
            + s.blocks
                .iter()
                .filter_map(|b| b.system.as_ref())
                .map(line_count)
                .sum::<usize>()
    }
    count(&imported.model.root, &mut blocks);
    let diagnostics = validate_model(&imported.model);
    let mut validate: BTreeMap<String, usize> = BTreeMap::new();
    for d in diagnostics {
        *validate
            .entry(format!("{:?}:{}", d.severity, d.code))
            .or_default() += 1;
    }
    let unsupported_types: Vec<String> = imported
        .report
        .unsupported_blocks
        .iter()
        .map(|(_, t)| t.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let (compile_result, compile_errors) = if imported.report.unsupported_blocks.is_empty() {
        match compile(&imported.model) {
            Ok(_) => ("ok", Vec::new()),
            Err(e) => (
                "fail",
                e.diagnostics
                    .into_iter()
                    .map(|d| format!("{}: {}", d.code, d.message))
                    .collect(),
            ),
        }
    } else {
        ("skipped", Vec::new())
    };
    let mut roundtrip = json!({"parts_changed":0,"parts_lost":0,"P_before":package_p_count(&bytes),"P_after":package_p_count(&bytes),"reread_equal":false});
    if let Ok(out) = write_slx_bytes(&imported.model, Some(&imported.preserved)) {
        if let (Ok(before), Ok(after)) = (read_package(&bytes), read_package(&out)) {
            let before_names: std::collections::BTreeSet<_> =
                before.parts.iter().map(|p| p.name.as_str()).collect();
            let after_names: std::collections::BTreeSet<_> =
                after.parts.iter().map(|p| p.name.as_str()).collect();
            let changed = before
                .parts
                .iter()
                .filter(|p| after.part(&p.name).is_some_and(|q| q.data != p.data))
                .count();
            let lost = before_names.difference(&after_names).count();
            let reread_equal = read_slx_bytes(&out, &name).is_ok_and(|i| i.model == imported.model);
            roundtrip = json!({"parts_changed":changed,"parts_lost":lost,"P_before":package_p_count(&bytes),"P_after":package_p_count(&out),"reread_equal":reread_equal});
        }
    }
    json!({"file":name,"version":version,"read":"ok","blocks":blocks,"lines":line_count(&imported.model.root),"unsupported":imported.report.unsupported_blocks.len(),"unsupported_types":unsupported_types,"dropped":imported.report.dropped.len(),"warnings":imported.report.warnings.len(),"validate":validate,"compile":compile_result,"compile_errors":compile_errors,"roundtrip":roundtrip})
}

fn main() {
    let Some(dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: slx_survey <directory>");
        std::process::exit(2);
    };
    let mut files: Vec<_> = match std::fs::read_dir(&dir) {
        Ok(r) => r
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "slx"))
            .collect(),
        Err(e) => {
            eprintln!("cannot read directory: {e}");
            std::process::exit(1);
        }
    };
    files.sort();
    let rows: Vec<_> = files.iter().map(|p| survey(p)).collect();
    let mut stats = BTreeMap::new();
    stats.insert("files", rows.len());
    stats.insert("read_ok", rows.iter().filter(|r| r["read"] == "ok").count());
    stats.insert(
        "unsupported_zero",
        rows.iter().filter(|r| r["unsupported"] == 0).count(),
    );
    stats.insert(
        "compile_ok",
        rows.iter().filter(|r| r["compile"] == "ok").count(),
    );
    stats.insert(
        "P_before",
        rows.iter()
            .map(|r| r["roundtrip"]["P_before"].as_u64().unwrap_or(0) as usize)
            .sum(),
    );
    stats.insert(
        "P_after",
        rows.iter()
            .map(|r| r["roundtrip"]["P_after"].as_u64().unwrap_or(0) as usize)
            .sum(),
    );
    eprintln!(
        "summary: {}",
        serde_json::to_string(&stats).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&rows).unwrap_or_else(|e| format!("[] /* {e} */"))
    );
}

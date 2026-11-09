//! SLX ⇔ Model mapping (best effort, not verified against Simulink R2026b).
use crate::error::SlxError;
use crate::opc::{read_package, write_package, OpcPackage};
use crate::xml::{parse_xml, write_xml, XmlDocument, XmlElement, XmlNode};
use sim_core::geom::Rect;
use sim_core::ids::{BlockId, LineId};
use sim_core::library::{fill_default_params, spec};
use sim_core::model::*;
use std::collections::BTreeMap;
use std::path::Path;

/// What happened during import.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportReport {
    pub info: Vec<String>,
    pub warnings: Vec<String>,
    pub unsupported_blocks: Vec<(String, String)>,
    pub dropped: Vec<String>,
}

impl ImportReport {
    /// Human-readable lines.
    pub fn lines(&self) -> Vec<String> {
        let mut v = vec![
            "SLX import is experimental and has not been verified against Simulink R2026b."
                .to_string(),
        ];
        v.extend(self.info.iter().map(|s| format!("[Info] {s}")));
        v.extend(self.warnings.iter().map(|s| format!("[Warning] {s}")));
        v.extend(
            self.unsupported_blocks
                .iter()
                .map(|(p, t)| format!("[Unsupported] {p} (BlockType {t})")),
        );
        v.extend(self.dropped.iter().map(|s| format!("[Dropped] {s}")));
        v
    }
}

/// Original package, kept for export.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PreservedData {
    pub package: OpcPackage,
    pub split_systems: bool,
}

/// Import result.
#[derive(Debug, Clone, PartialEq)]
pub struct SlxImport {
    pub model: Model,
    pub report: ImportReport,
    pub preserved: PreservedData,
}

const BD: &str = "simulink/blockdiagram.xml";
const ROOT_SYS: &str = "simulink/systems/system_root.xml";

fn p_map(e: &XmlElement) -> BTreeMap<String, String> {
    e.children_named("P")
        .filter_map(|p| Some((p.attr("Name")?, p.text())))
        .collect()
}

fn parse_num_list(s: &str) -> Vec<f64> {
    s.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split([',', ';', ' '])
        .filter(|x| !x.trim().is_empty())
        .filter_map(|x| x.trim().parse().ok())
        .collect()
}

fn parse_port_ref(s: &str, want_in: bool) -> Option<PortRef> {
    let (sid, rest) = s.trim().split_once('#')?;
    let (kind, n) = rest.split_once(':')?;
    if (want_in && kind != "in") || (!want_in && kind != "out") {
        return None;
    }
    let sid: u32 = sid.rsplit(':').next()?.parse().ok()?;
    Some(PortRef {
        block: BlockId(sid),
        port: n.parse().ok()?,
    })
}

struct Reader<'a> {
    pkg: &'a OpcPackage,
    report: ImportReport,
    max_line: u32,
}

impl Reader<'_> {
    fn load_part(&self, name: &str) -> Option<XmlDocument> {
        parse_xml(name, &self.pkg.part(name)?.data).ok()
    }

    fn system(&mut self, sys_el: &XmlElement, path: &str) -> System {
        let mut sys = System::default();
        for b in sys_el.children_named("Block") {
            if let Some(blk) = self.block(b, path) {
                sys.insert_block_sorted(blk);
            }
        }
        for l in sys_el.children_named("Line") {
            if let Some(line) = self.line(l, path) {
                sys.insert_line_sorted(line);
            }
        }
        sys
    }

    fn block(&mut self, b: &XmlElement, path: &str) -> Option<Block> {
        let ps = p_map(b);
        let mut bt = b.attr("BlockType").unwrap_or_default();
        let name = b.attr("Name").unwrap_or_default();
        let full = format!("{path}/{name}");
        let Some(sid) = b
            .attr("SID")
            .and_then(|s| s.rsplit(':').next().and_then(|x| x.parse::<u32>().ok()))
        else {
            self.report
                .dropped
                .push(format!("{full}: block without numeric SID"));
            return None;
        };
        let pos = parse_num_list(
            ps.get("Position")
                .map(String::as_str)
                .unwrap_or("[0, 0, 30, 30]"),
        );
        let position = if pos.len() == 4 {
            Rect::new(pos[0], pos[1], pos[2], pos[3])
        } else {
            Rect::new(0.0, 0.0, 30.0, 30.0)
        };
        let orientation = match ps.get("Orientation").map(String::as_str) {
            Some("left") => Orientation::Left,
            Some("up") => Orientation::Up,
            Some("down") => Orientation::Down,
            _ => Orientation::Right,
        };
        let mut params = BTreeMap::new();
        let mut system = None;
        let source = ps.get("SourceBlock").cloned().unwrap_or_default();
        if bt == "SubSystem" && source.contains("PID Controller") {
            bt = "PIDController".into();
            let inst: BTreeMap<String, String> =
                b.children_named("InstanceData").flat_map(p_map).collect();
            let all: BTreeMap<String, String> = ps.clone().into_iter().chain(inst).collect();
            for k in ["P", "I", "D", "N", "Controller", "Form", "TimeDomain"] {
                if let Some(v) = all.get(k) {
                    params.insert(k.to_string(), v.clone());
                }
            }
            self.report.info.push(format!(
                "{full}: PID Controller library block mapped to PIDController"
            ));
        } else if bt == "SubSystem" {
            let inline = b.children_named("System").next();
            let child = match inline {
                Some(s) if s.attr("Ref").is_none() => Some(self.system(s, &full)),
                Some(s) => {
                    let r = s.attr("Ref").unwrap_or_default();
                    let part = format!("simulink/systems/{r}.xml");
                    match self.load_part(&part) {
                        Some(doc) => doc.root().map(|root| self.system(root, &full)),
                        None => {
                            self.report
                                .warnings
                                .push(format!("{full}: referenced system part {part} not found"));
                            None
                        }
                    }
                }
                None => None,
            };
            system = Some(child.unwrap_or_default());
        } else if let Some(s) = spec(&bt) {
            for p in s.params {
                if let Some(v) = ps.get(p.name) {
                    params.insert(p.name.to_string(), v.clone());
                }
            }
        } else {
            self.report
                .unsupported_blocks
                .push((full.clone(), bt.clone()));
        }
        let mut blk = Block {
            sid: BlockId(sid),
            block_type: bt,
            name,
            position,
            orientation,
            params,
            system,
        };
        fill_default_params(&mut blk);
        Some(blk)
    }

    fn branches(&mut self, e: &XmlElement, path: &str) -> (Option<PortRef>, Vec<Branch>) {
        let ps = p_map(e);
        let dst = ps.get("Dst").and_then(|d| {
            let r = parse_port_ref(d, true);
            if r.is_none() {
                self.report
                    .dropped
                    .push(format!("{path}: connection to '{d}' is not supported"));
            }
            r
        });
        let mut brs = vec![];
        for b in e.children_named("Branch") {
            let (d, sub) = self.branches(b, path);
            brs.push(Branch {
                tap: TapPoint { segment: 0, t: 0.5 },
                points: vec![],
                dst: d,
                branches: sub,
            });
        }
        (dst, brs)
    }

    fn line(&mut self, l: &XmlElement, path: &str) -> Option<Line> {
        let ps = p_map(l);
        let Some(src) = ps.get("Src").and_then(|s| parse_port_ref(s, false)) else {
            self.report
                .dropped
                .push(format!("{path}: line without a supported source"));
            return None;
        };
        let (dst, branches) = self.branches(l, path);
        self.max_line += 1;
        Some(Line {
            id: LineId(self.max_line),
            src,
            points: vec![],
            dst,
            branches,
            name: ps.get("Name").cloned().unwrap_or_default(),
        })
    }
}

fn find_p(e: &XmlElement, name: &str, out: &mut Option<String>) {
    if out.is_some() {
        return;
    }
    if e.name == "P" && e.attr("Name").as_deref() == Some(name) {
        *out = Some(e.text());
        return;
    }
    for c in e.elements() {
        find_p(c, name, out);
    }
}

fn config_value(pkg: &OpcPackage, name: &str) -> Option<String> {
    for part in &pkg.parts {
        if !part.name.starts_with("simulink/")
            || !part.name.ends_with(".xml")
            || part.name.starts_with("simulink/systems/")
        {
            continue;
        }
        let Ok(doc) = parse_xml(&part.name, &part.data) else {
            continue;
        };
        let mut out = None;
        if let Some(r) = doc.root() {
            find_p(r, name, &mut out);
        }
        if out.is_some() {
            return out;
        }
    }
    None
}

/// Reads an SLX from bytes.
pub fn read_slx_bytes(bytes: &[u8], model_name: &str) -> Result<SlxImport, SlxError> {
    let pkg = read_package(bytes)?;
    let mut r = Reader {
        pkg: &pkg,
        report: ImportReport::default(),
        max_line: 0,
    };
    let split = pkg.part(ROOT_SYS).is_some();
    let root = if split {
        let doc = r
            .load_part(ROOT_SYS)
            .ok_or_else(|| SlxError::Unsupported("cannot parse system_root.xml".into()))?;
        let el = doc
            .root()
            .cloned()
            .ok_or_else(|| SlxError::Unsupported("empty system_root.xml".into()))?;
        r.system(&el, model_name)
    } else {
        let doc = r.load_part(BD).ok_or_else(|| {
            SlxError::Unsupported("no simulink/blockdiagram.xml or systems/system_root.xml".into())
        })?;
        let root = doc
            .root()
            .ok_or_else(|| SlxError::Unsupported("empty blockdiagram.xml".into()))?;
        let model_el = root
            .children_named("Model")
            .next()
            .or(if root.name == "Model" {
                Some(root)
            } else {
                None
            });
        let sys_el = model_el
            .and_then(|m| m.children_named("System").next())
            .cloned();
        match sys_el {
            Some(s) => r.system(&s, model_name),
            None => return Err(SlxError::Unsupported("no root System element found".into())),
        }
    };
    let mut model = Model::new(model_name);
    model.root = root;
    let num = |k: &str| config_value(&pkg, k).and_then(|v| v.trim().parse::<f64>().ok());
    if let Some(v) = num("StartTime") {
        model.config.start_time = v;
    }
    if let Some(v) = num("StopTime") {
        model.config.stop_time = v;
    }
    if let Some(v) = num("RelTol") {
        model.config.rel_tol = v;
    }
    if let Some(v) = num("AbsTol") {
        model.config.abs_tol = v;
    }
    if let Some(v) = num("MaxStep") {
        model.config.max_step = Some(v);
    }
    match config_value(&pkg, "Solver").as_deref() {
        Some("ode45") => model.config.solver = SolverKind::Ode45,
        Some("ode23") => model.config.solver = SolverKind::Ode23,
        Some("ode15s") => model.config.solver = SolverKind::Ode15s,
        Some(s) if !s.is_empty() && s != "VariableStepAuto" => r
            .report
            .warnings
            .push(format!("solver '{s}' is not supported; using auto (RK45)")),
        _ => {}
    }
    model.config.output_step =
        ((model.config.stop_time - model.config.start_time) / 500.0).max(1e-6);
    r.report.info.push(format!(
        "output_step set to {} (Simulink logs at solver steps)",
        model.config.output_step
    ));
    r.report
        .info
        .push("line routing points are not imported (lines are auto-routed)".into());
    model.next_sid = model.max_sid() + 1;
    model.next_line_id = model.max_line_id() + 1;
    let report = r.report;
    Ok(SlxImport {
        model,
        report,
        preserved: PreservedData {
            package: pkg.clone(),
            split_systems: split,
        },
    })
}

/// Reads an SLX file.
pub fn read_slx(path: &Path) -> Result<SlxImport, SlxError> {
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "model".into());
    read_slx_bytes(&std::fs::read(path)?, &name)
}

// ------------------------------------------------------------------ writing

fn p_el(name: &str, value: &str) -> XmlElement {
    let mut e = XmlElement::new("P");
    e.set_attr("Name", name);
    e.set_text(value);
    e
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn branch_el(dst: Option<PortRef>, brs: &[Branch]) -> Vec<XmlElement> {
    let mut v = vec![];
    if let Some(d) = dst {
        v.push(p_el("Dst", &format!("{}#in:{}", d.block.0, d.port)));
    }
    for b in brs {
        let mut e = XmlElement::new("Branch");
        for c in branch_el(b.dst, &b.branches) {
            e.push(c);
        }
        v.push(e);
    }
    v
}

fn system_el(sys: &System, split: bool, parts: &mut Vec<(String, XmlElement)>) -> XmlElement {
    let mut s = XmlElement::new("System");
    for b in &sys.blocks {
        let mut e = XmlElement::new("Block");
        let (bt, extra) = if b.block_type == "PIDController" {
            ("SubSystem", true)
        } else {
            (b.block_type.as_str(), false)
        };
        e.set_attr("BlockType", bt);
        e.set_attr("Name", &b.name);
        e.set_attr("SID", &b.sid.0.to_string());
        let r = b.position;
        e.push(p_el(
            "Position",
            &format!(
                "[{}, {}, {}, {}]",
                fmt_num(r.left),
                fmt_num(r.top),
                fmt_num(r.right),
                fmt_num(r.bottom)
            ),
        ));
        if b.orientation != Orientation::Right {
            let o = match b.orientation {
                Orientation::Left => "left",
                Orientation::Up => "up",
                _ => "down",
            };
            e.push(p_el("Orientation", o));
        }
        if extra {
            e.push(p_el("SourceBlock", "simulink/Continuous/PID Controller"));
            let mut inst = XmlElement::new("InstanceData");
            for (k, v) in &b.params {
                inst.push(p_el(k, v));
            }
            e.push(inst);
        } else {
            for (k, v) in &b.params {
                e.push(p_el(k, v));
            }
        }
        if let Some(c) = &b.system {
            let child = system_el(c, split, parts);
            if split {
                let r = format!("system_{}", b.sid.0);
                parts.push((format!("simulink/systems/{r}.xml"), child));
                let mut refe = XmlElement::new("System");
                refe.set_attr("Ref", &r);
                refe.self_closing = true;
                e.push(refe);
            } else {
                e.push(child);
            }
        }
        s.push(e);
    }
    for l in &sys.lines {
        let mut e = XmlElement::new("Line");
        if !l.name.is_empty() {
            e.push(p_el("Name", &l.name));
        }
        e.push(p_el(
            "Src",
            &format!("{}#out:{}", l.src.block.0, l.src.port),
        ));
        for c in branch_el(l.dst, &l.branches) {
            e.push(c);
        }
        s.push(e);
    }
    s
}

fn doc(root: XmlElement) -> Vec<u8> {
    let d = XmlDocument {
        nodes: vec![
            XmlNode::Decl("xml version=\"1.0\" encoding=\"utf-8\"".into()),
            XmlNode::Text("\n".into()),
            XmlNode::Element(root),
        ],
    };
    write_xml(&d)
}

/// Serializes a model to SLX bytes. With `preserved`, unknown parts of the original package are kept.
pub fn write_slx_bytes(
    model: &Model,
    preserved: Option<&PreservedData>,
) -> Result<Vec<u8>, SlxError> {
    let split = preserved.map(|p| p.split_systems).unwrap_or(false);
    let mut extra = vec![];
    let root_sys = system_el(&model.root, split, &mut extra);
    let mut pkg = preserved.map(|p| p.package.clone()).unwrap_or_default();
    if pkg.part("[Content_Types].xml").is_none() {
        pkg.set_part(
            "[Content_Types].xml",
            br#"<?xml version="1.0" encoding="utf-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/vnd.mathworks.simulink.model+xml"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/></Types>"#
                .to_vec(),
        );
        pkg.set_part(
            "_rels/.rels",
            br#"<?xml version="1.0" encoding="utf-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="blockDiagram" Target="simulink/blockdiagram.xml" Type="http://schemas.mathworks.com/simulink/2010/relationships/blockDiagram"/></Relationships>"#
                .to_vec(),
        );
    }
    if split {
        pkg.parts
            .retain(|p| !p.name.starts_with("simulink/systems/"));
        pkg.set_part(ROOT_SYS, doc(root_sys));
        for (name, el) in extra {
            pkg.set_part(&name, doc(el));
        }
    } else {
        let mut info = XmlElement::new("ModelInformation");
        info.set_attr("Version", "1.0");
        let mut m = XmlElement::new("Model");
        m.set_attr("Name", &model.name);
        let c = &model.config;
        m.push(p_el("StartTime", &fmt_num(c.start_time)));
        m.push(p_el("StopTime", &fmt_num(c.stop_time)));
        m.push(p_el("RelTol", &format!("{}", c.rel_tol)));
        m.push(p_el("AbsTol", &format!("{}", c.abs_tol)));
        let solver = c.solver.simulink_name();
        m.push(p_el("Solver", solver));
        m.push(root_sys);
        info.push(m);
        pkg.set_part(BD, doc(info));
    }
    write_package(&pkg)
}

/// Writes an SLX file (never overwrites an existing file).
pub fn write_slx(
    model: &Model,
    preserved: Option<&PreservedData>,
    path: &Path,
) -> Result<(), SlxError> {
    if path.exists() {
        return Err(SlxError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "{} exists; simpylink never overwrites SLX files",
                path.display()
            ),
        )));
    }
    std::fs::write(path, write_slx_bytes(model, preserved)?)?;
    Ok(())
}

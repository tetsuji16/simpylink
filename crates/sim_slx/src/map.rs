//! SLX ⇔ Model mapping (best effort, not verified against Simulink R2026b).
use crate::error::SlxError;
use crate::opc::{read_package, write_package, OpcPackage};
use crate::xml::{parse_xml, write_xml, XmlDocument, XmlElement, XmlNode};
use sim_core::geom::Rect;
use sim_core::ids::{BlockId, LineId};
use sim_core::library::{fill_default_params, spec, OPAQUE_INPUTS, OPAQUE_OUTPUTS};
use sim_core::model::*;
use std::collections::{BTreeMap, BTreeSet};
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
        let types: BTreeSet<_> = self.unsupported_blocks.iter().map(|(_, t)| t).collect();
        v.push(format!("[Summary] {} blocks not supported ({} types); they are kept as opaque blocks and cannot be simulated.", self.unsupported_blocks.len(), types.len()));
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
#[derive(Debug, Clone, PartialEq)]
pub struct PreservedData {
    pub package: OpcPackage,
    pub split_systems: bool,
    /// simpylink BlockId to original SLX SID string.
    pub sid_map: BTreeMap<BlockId, String>,
    /// Model exactly as imported; used to find edits before writing.
    pub baseline: Model,
}

impl Default for PreservedData {
    fn default() -> Self {
        Self {
            package: OpcPackage::default(),
            split_systems: false,
            sid_map: BTreeMap::new(),
            baseline: Model::new(""),
        }
    }
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

/// Loads a part that the model cannot be read without.
///
/// A part that is present but is not well-formed XML is reported as `SlxError::Xml`
/// with that part name, so a truncated file is not read as a complete one.
fn required_part(pkg: &OpcPackage, name: &str, missing: &str) -> Result<XmlDocument, SlxError> {
    let part = pkg
        .part(name)
        .ok_or_else(|| SlxError::Unsupported(missing.into()))?;
    parse_xml(name, &part.data)
}

fn collect_sids(pkg: &OpcPackage, split: bool) -> Result<Vec<String>, SlxError> {
    fn walk(
        pkg: &OpcPackage,
        sys: &XmlElement,
        seen: &mut BTreeSet<String>,
        out: &mut Vec<String>,
    ) -> Result<(), SlxError> {
        for block in sys.children_named("Block") {
            if let Some(sid) = block.attr("SID") {
                out.push(sid);
            }
            if let Some(child) = block.children_named("System").next() {
                if let Some(reference) = child.attr("Ref") {
                    let part = format!("simulink/systems/{reference}.xml");
                    if seen.insert(part.clone()) {
                        // A missing part is reported later as "not found"; one that exists
                        // but cannot be parsed breaks the read.
                        if let Some(p) = pkg.part(&part) {
                            let doc = parse_xml(&part, &p.data)?;
                            if let Some(root) = doc.root() {
                                walk(pkg, root, seen, out)?;
                            }
                        }
                    }
                } else {
                    walk(pkg, child, seen, out)?;
                }
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let root = if split {
        pkg.part(ROOT_SYS)
            .map(|p| parse_xml(ROOT_SYS, &p.data))
            .transpose()?
    } else {
        pkg.part(BD).map(|p| parse_xml(BD, &p.data)).transpose()?
    };
    if let Some(doc) = root {
        if split {
            if let Some(sys) = doc.root() {
                walk(pkg, sys, &mut seen, &mut out)?;
            }
        } else if let Some(sys) = doc
            .root()
            .and_then(|r| {
                r.children_named("Model")
                    .next()
                    .or(if r.name == "Model" { Some(r) } else { None })
            })
            .and_then(|m| m.children_named("System").next())
        {
            walk(pkg, sys, &mut seen, &mut out)?;
        }
    }
    Ok(out)
}

struct SidAssignment {
    lookup: BTreeMap<String, BlockId>,
    reverse: BTreeMap<BlockId, String>,
    occurrences: BTreeMap<String, Vec<BlockId>>,
    renumbered: usize,
}

fn assign_sids(raw: &[String]) -> SidAssignment {
    let mut used = BTreeSet::new();
    let mut by_text = BTreeMap::new();
    let mut reverse = BTreeMap::new();
    let mut assignments: BTreeMap<String, Vec<BlockId>> = BTreeMap::new();
    let mut counts = BTreeMap::<String, usize>::new();
    for sid in raw {
        *counts.entry(sid.clone()).or_default() += 1;
    }
    let mut max = 0u32;
    for sid in raw {
        if counts.get(sid) == Some(&1) {
            if let Ok(n) = sid.parse::<u32>() {
                used.insert(n);
                max = max.max(n);
            }
        }
    }
    let mut next = max.saturating_add(1);
    let mut renumbered = 0;
    let mut seen = BTreeMap::<String, usize>::new();
    for sid in raw {
        let occurrence = seen.entry(sid.clone()).or_default();
        let id = if *occurrence == 0 && counts.get(sid) == Some(&1) && sid.parse::<u32>().is_ok() {
            sid.parse::<u32>().map(BlockId).unwrap_or(BlockId(0))
        } else {
            while used.contains(&next) {
                next = next.saturating_add(1);
            }
            let id = BlockId(next);
            used.insert(next);
            next = next.saturating_add(1);
            renumbered += 1;
            id
        };
        *occurrence += 1;
        assignments.entry(sid.clone()).or_default().push(id);
        by_text.entry(sid.clone()).or_insert(id);
        reverse.insert(id, sid.clone());
    }
    SidAssignment {
        lookup: by_text,
        reverse,
        occurrences: assignments,
        renumbered,
    }
}

fn p_map(e: &XmlElement) -> BTreeMap<String, String> {
    e.children_named("P")
        .filter_map(|p| Some((p.attr("Name")?, p.text())))
        .collect()
}

fn orientation_from_slx(
    rotation: Option<&str>,
    mirror: Option<&str>,
    legacy: Option<&str>,
) -> (Orientation, bool) {
    if rotation.is_none() && mirror.is_none() {
        return (
            match legacy {
                Some("left") => Orientation::Left,
                Some("up") => Orientation::Up,
                Some("down") => Orientation::Down,
                _ => Orientation::Right,
            },
            false,
        );
    }
    let turns = match rotation.unwrap_or("0") {
        "0" => 0,
        "90" => 1,
        "180" => 2,
        "270" => 3,
        _ => return (Orientation::Right, false),
    };
    let mut orientation = Orientation::Right;
    for _ in 0..turns {
        orientation = orientation.rotated_cw();
    }
    if mirror == Some("on") {
        orientation = orientation.flipped();
    }
    (orientation, turns == 2)
}

fn orientation_to_slx(orientation: Orientation) -> (Option<&'static str>, bool) {
    match orientation {
        Orientation::Right => (None, false),
        Orientation::Left => (None, true),
        Orientation::Down => (Some("90"), false),
        Orientation::Up => (Some("270"), false),
    }
}

fn set_orientation_xml(block: &mut XmlElement, orientation: Orientation) {
    block.children.retain(|node| match node {
        XmlNode::Element(element) if element.name == "P" => !matches!(
            element.attr("Name").as_deref(),
            Some("BlockRotation" | "BlockMirror" | "Orientation")
        ),
        _ => true,
    });
    let (rotation, mirror) = orientation_to_slx(orientation);
    let at = block
        .children
        .iter()
        .position(|node| {
            matches!(node, XmlNode::Element(element) if element.name == "P" && element.attr("Name").as_deref() == Some("Position"))
        })
        .map(|index| index + 1)
        .unwrap_or(block.children.len());
    let mut values = Vec::new();
    if let Some(rotation) = rotation {
        values.push(p_el("BlockRotation", rotation));
    }
    if mirror {
        values.push(p_el("BlockMirror", "on"));
    }
    for (offset, value) in values.into_iter().enumerate() {
        block.children.insert(at + offset, XmlNode::Element(value));
    }
}

fn patch_orientations_in_system(
    xml: &mut XmlElement,
    before: &System,
    after: &System,
    sid_map: &BTreeMap<BlockId, String>,
    split: bool,
    pkg: &mut OpcPackage,
) -> Result<bool, SlxError> {
    let mut changed = false;
    for node in &mut xml.children {
        let XmlNode::Element(block_xml) = node else {
            continue;
        };
        if block_xml.name != "Block" {
            continue;
        }
        let Some(sid_text) = block_xml.attr("SID") else {
            continue;
        };
        let id = sid_map
            .iter()
            .find_map(|(id, sid)| (sid == &sid_text).then_some(*id))
            .or_else(|| sid_text.parse::<u32>().ok().map(BlockId));
        let Some(id) = id else {
            continue;
        };
        let old = before.blocks.iter().find(|block| block.sid == id);
        let new = after.blocks.iter().find(|block| block.sid == id);
        if let Some(new) = new {
            if old.is_none_or(|old| old.orientation != new.orientation) {
                set_orientation_xml(block_xml, new.orientation);
                changed = true;
            }
            if let Some(new_child) = &new.system {
                if split {
                    let name = sid_map
                        .get(&new.sid)
                        .cloned()
                        .unwrap_or_else(|| new.sid.0.to_string());
                    let part = format!("simulink/systems/system_{name}.xml");
                    if let Some(original) = pkg.part(&part).map(|p| p.data.clone()) {
                        let mut doc = parse_xml(&part, &original)?;
                        let updated = if let Some(root) = doc.root_mut() {
                            patch_orientations_in_system(
                                root,
                                old.and_then(|block| block.system.as_ref())
                                    .unwrap_or(&System::default()),
                                new_child,
                                sid_map,
                                true,
                                pkg,
                            )?
                        } else {
                            false
                        };
                        if updated {
                            pkg.set_part(&part, write_xml(&doc));
                        }
                    }
                } else if let Some(child_xml) =
                    block_xml.children.iter_mut().find_map(|node| match node {
                        XmlNode::Element(element)
                            if element.name == "System" && element.attr("Ref").is_none() =>
                        {
                            Some(element)
                        }
                        _ => None,
                    })
                {
                    changed |= patch_orientations_in_system(
                        child_xml,
                        old.and_then(|block| block.system.as_ref())
                            .unwrap_or(&System::default()),
                        new_child,
                        sid_map,
                        false,
                        pkg,
                    )?;
                }
            }
        }
    }
    Ok(changed)
}

fn patch_orientations(
    pkg: &mut OpcPackage,
    model: &Model,
    preserved: &PreservedData,
) -> Result<(), SlxError> {
    let part = if preserved.split_systems {
        ROOT_SYS
    } else {
        BD
    };
    if model == &preserved.baseline {
        return Ok(());
    }
    let Some(original) = pkg.part(part).map(|p| p.data.clone()) else {
        return Ok(());
    };
    let mut doc = parse_xml(part, &original)?;
    let Some(root) = doc.root_mut() else {
        return Ok(());
    };
    let system = if preserved.split_systems {
        Some(root)
    } else {
        root.children.iter_mut().find_map(|node| match node {
            XmlNode::Element(model) if model.name == "Model" => {
                model.children.iter_mut().find_map(|node| match node {
                    XmlNode::Element(system) if system.name == "System" => Some(system),
                    _ => None,
                })
            }
            _ => None,
        })
    };
    let changed = if let Some(system) = system {
        patch_orientations_in_system(
            system,
            &preserved.baseline.root,
            &model.root,
            &preserved.sid_map,
            preserved.split_systems,
            pkg,
        )?
    } else {
        false
    };
    if changed {
        pkg.set_part(part, write_xml(&doc));
    }
    Ok(())
}

/// Splits a Simulink number list.
///
/// Returns the values that could be read and, if any element was not a number, the
/// first offending element. A caller must treat the whole list as unreadable then,
/// rather than silently working with the remaining numbers.
fn parse_num_list(s: &str) -> (Vec<f64>, Option<String>) {
    let mut values = vec![];
    let mut bad = None;
    for x in s
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split([',', ';', ' '])
    {
        let x = x.trim();
        if x.is_empty() {
            continue;
        }
        match x.parse::<f64>() {
            Ok(v) => values.push(v),
            Err(_) => {
                if bad.is_none() {
                    bad = Some(x.to_string());
                }
            }
        }
    }
    (values, bad)
}

fn parse_port_ref(s: &str, want_in: bool, sid_map: &BTreeMap<String, BlockId>) -> Option<PortRef> {
    let (sid, rest) = s.trim().split_once('#')?;
    let (kind, n) = rest.split_once(':')?;
    if (want_in && kind != "in") || (!want_in && kind != "out") {
        return None;
    }
    let sid = *sid_map.get(sid)?;
    Some(PortRef {
        block: sid,
        port: n.parse().ok()?,
    })
}

struct Reader<'a> {
    pkg: &'a OpcPackage,
    report: ImportReport,
    max_line: u32,
    sid_map: BTreeMap<String, BlockId>,
    sid_assignments: BTreeMap<String, Vec<BlockId>>,
    sid_occurrences: BTreeMap<String, usize>,
    physical: usize,
    orientation_approximated: usize,
}

impl Reader<'_> {
    /// Loads an optional part; one that exists but cannot be parsed becomes a warning.
    fn load_part(&mut self, name: &str) -> Option<XmlDocument> {
        // Copy the reference out so the borrow of `self` ends before the warning is pushed.
        let pkg = self.pkg;
        let part = pkg.part(name)?;
        match parse_xml(name, &part.data) {
            Ok(doc) => Some(doc),
            Err(e) => {
                self.report
                    .warnings
                    .push(format!("{name}: could not be parsed ({e}); ignored"));
                None
            }
        }
    }

    fn system(&mut self, sys_el: &XmlElement, path: &str) -> System {
        let mut sys = System::default();
        let physical_before = self.physical;
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
        let physical = self.physical - physical_before;
        if physical > 0 {
            self.report.dropped.push(format!(
                "{path}: {physical} physical (Simscape) connections are not supported"
            ));
        }
        sys
    }

    fn block(&mut self, b: &XmlElement, path: &str) -> Option<Block> {
        let ps = p_map(b);
        // `Name` is needed to build the block path, so it comes first.
        let name = b.attr("Name").unwrap_or_default();
        let full = format!("{path}/{name}");
        if name.is_empty() {
            self.report
                .warnings
                .push(format!("{path}: block has no Name"));
        }
        // A block without a BlockType cannot be mapped to a known block; keep it opaque
        // instead of pretending it is a valid empty type.
        let mut bt = match b.attr("BlockType") {
            Some(v) => v,
            None => {
                self.report.warnings.push(format!(
                    "{full}: block has no BlockType; kept as an opaque block"
                ));
                String::new()
            }
        };
        let Some(sid_text) = b.attr("SID") else {
            self.report
                .dropped
                .push(format!("{full}: block without numeric SID"));
            return None;
        };
        let occurrence = self.sid_occurrences.entry(sid_text.clone()).or_default();
        let assigned = self
            .sid_assignments
            .get(&sid_text)
            .and_then(|ids| ids.get(*occurrence))
            .copied();
        *occurrence += 1;
        let Some(sid) = assigned.or_else(|| self.sid_map.get(&sid_text).copied()) else {
            self.report
                .dropped
                .push(format!("{full}: SID '{sid_text}' was not assigned"));
            return None;
        };
        let raw_position = ps
            .get("Position")
            .map(String::as_str)
            .unwrap_or("[0, 0, 30, 30]");
        let (pos, bad_position) = parse_num_list(raw_position);
        let position = match bad_position {
            Some(raw) => {
                self.report.warnings.push(format!(
                    "{full}: Position has a value that is not a number: '{raw}'; using the default position"
                ));
                Rect::new(0.0, 0.0, 30.0, 30.0)
            }
            None if pos.len() == 4 => Rect::new(pos[0], pos[1], pos[2], pos[3]),
            None => Rect::new(0.0, 0.0, 30.0, 30.0),
        };
        let rotation = ps.get("BlockRotation").map(String::as_str);
        if let Some(value) = rotation.filter(|value| !matches!(*value, "0" | "90" | "180" | "270"))
        {
            self.report.warnings.push(format!(
                "{full}: invalid BlockRotation '{value}'; using right"
            ));
        }
        let (orientation, approximated) = orientation_from_slx(
            rotation,
            ps.get("BlockMirror").map(String::as_str),
            ps.get("Orientation").map(String::as_str),
        );
        if approximated {
            self.orientation_approximated += 1;
        }
        let mut params = BTreeMap::new();
        let mut system = None;
        // Optional: only library-linked blocks carry a SourceBlock.
        let source = ps.get("SourceBlock").map(String::as_str).unwrap_or("");
        if (bt == "SubSystem" && source.contains("PID Controller"))
            || (bt == "Reference" && source.contains("PID Controller"))
        {
            bt = "PIDController".into();
            let inst: BTreeMap<String, String> =
                b.children_named("InstanceData").flat_map(p_map).collect();
            let all: BTreeMap<String, String> =
                ps.clone().into_iter().chain(inst.clone()).collect();
            for k in ["P", "I", "D", "N", "Controller", "Form", "TimeDomain"] {
                if let Some(v) = all.get(k) {
                    params.insert(k.to_string(), v.clone());
                }
            }
            if !params.contains_key("TimeDomain")
                && ps
                    .get("LibrarySourceBlock")
                    .is_some_and(|s| s.starts_with("simulink/Discrete/"))
            {
                params.insert("TimeDomain".into(), "Discrete-time".into());
            }
            if inst.get("UseFilter").is_some_and(|s| s == "off") {
                self.report.warnings.push(format!(
                    "{full}: PID without derivative filter is approximated with N"
                ));
            }
            if ["InitialConditionForIntegrator", "InitialConditionForFilter"]
                .iter()
                .any(|k| inst.get(*k).is_some_and(|v| v != "0"))
            {
                self.report
                    .warnings
                    .push(format!("{full}: PID initial conditions are ignored"));
            }
            self.report.info.push(format!(
                "{full}: PID Controller library block mapped to PIDController"
            ));
        } else if bt == "SubSystem" {
            if let Some(value) = ps.get("TreatAsAtomicUnit") {
                params.insert("TreatAsAtomicUnit".into(), value.clone());
            }
            let inline = b.children_named("System").next();
            let child = match inline {
                Some(s) if s.attr("Ref").is_none() => Some(self.system(s, &full)),
                Some(s) => match s.attr("Ref") {
                    Some(r) => {
                        let part = format!("simulink/systems/{r}.xml");
                        match self.load_part(&part) {
                            Some(doc) => doc.root().map(|root| self.system(root, &full)),
                            None => {
                                self.report.warnings.push(format!(
                                    "{full}: referenced system part {part} not found"
                                ));
                                None
                            }
                        }
                    }
                    // Unreachable: the arm above handles a System without a Ref.
                    None => None,
                },
                None => None,
            };
            // A SubSystem without a readable body keeps an empty system.
            system = Some(child.unwrap_or_else(System::default));
        } else if let Some(s) = spec(&bt) {
            for p in s.params {
                if let Some(v) = ps.get(p.name) {
                    params.insert(p.name.to_string(), v.clone());
                }
            }
        } else {
            if let Some(pc) = b.children_named("PortCounts").next() {
                let input = pc.attr("in");
                let output = pc.attr("out");
                let parsed_in = input.as_deref().map(str::parse::<u32>);
                let parsed_out = output.as_deref().map(str::parse::<u32>);
                if parsed_in.as_ref().is_some_and(Result::is_err)
                    || parsed_out.as_ref().is_some_and(Result::is_err)
                {
                    self.report
                        .warnings
                        .push(format!("{full}: cannot parse PortCounts; using zero ports"));
                } else {
                    if let Some(Ok(n)) = parsed_in {
                        if n > 0 {
                            params.insert(OPAQUE_INPUTS.into(), n.to_string());
                        }
                    }
                    if let Some(Ok(n)) = parsed_out {
                        if n > 0 {
                            params.insert(OPAQUE_OUTPUTS.into(), n.to_string());
                        }
                    }
                }
            }
            self.report.unsupported_blocks.push((
                full.clone(),
                if bt == "Reference" {
                    format!("Reference:{}", source.replace('\n', " "))
                } else {
                    bt.clone()
                },
            ));
        }
        let mut blk = Block {
            sid,
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
            if d.contains("#lconn:") || d.contains("#rconn:") {
                return None;
            }
            let r = parse_port_ref(d, true, &self.sid_map);
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
        fn physical(e: &XmlElement) -> bool {
            let props = p_map(e);
            props
                .values()
                .any(|v| v.contains("#lconn:") || v.contains("#rconn:"))
                || e.children_named("Branch").any(physical)
        }
        if physical(l) {
            self.physical += 1;
        }
        if ps
            .get("Src")
            .is_some_and(|s| s.contains("#lconn:") || s.contains("#rconn:"))
        {
            return None;
        }
        let Some(src) = ps
            .get("Src")
            .and_then(|s| parse_port_ref(s, false, &self.sid_map))
        else {
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
            // A branch without a Name is still a branch; keep the label empty.
            name: ps.get("Name").cloned().unwrap_or_else(String::new),
        })
    }
}

/// Every `P` value found in the optional `simulink/*.xml` parts (first occurrence wins).
///
/// A part that exists but cannot be parsed is reported through `warnings` instead of
/// being skipped without a trace.
fn config_values(pkg: &OpcPackage, warnings: &mut Vec<String>) -> BTreeMap<String, String> {
    fn collect(e: &XmlElement, out: &mut BTreeMap<String, String>) {
        for p in e.children_named("P") {
            if let Some(name) = p.attr("Name") {
                out.entry(name).or_insert_with(|| p.text());
            }
        }
        for c in e.elements() {
            collect(c, out);
        }
    }
    let mut out = BTreeMap::new();
    for part in &pkg.parts {
        if !part.name.starts_with("simulink/")
            || !part.name.ends_with(".xml")
            || part.name.starts_with("simulink/systems/")
        {
            continue;
        }
        match parse_xml(&part.name, &part.data) {
            Ok(doc) => {
                if let Some(r) = doc.root() {
                    collect(r, &mut out);
                }
            }
            Err(e) => warnings.push(format!("{}: could not be parsed ({e}); ignored", part.name)),
        }
    }
    out
}

/// Active config-set solver properties as name to string value.
fn solver_settings(pkg: &OpcPackage, warnings: &mut Vec<String>) -> BTreeMap<String, String> {
    let json_path = pkg
        .part("simulink/configSetInfo.json")
        .and_then(|p| serde_json::from_slice::<serde_json::Value>(&p.data).ok())
        .and_then(|v| {
            v.get("ConfigSetInfo")?
                .as_array()?
                .iter()
                .find(|x| x.get("Active").and_then(|a| a.as_bool()) == Some(true))?
                .get("PartName")?
                .as_str()
                .map(|s| s.trim_start_matches('/').to_string())
        })
        .filter(|s| pkg.part(s).is_some())
        .or_else(|| {
            pkg.part("simulink/configSet0.json")
                .map(|_| "simulink/configSet0.json".to_string())
        });
    if let Some(path) = json_path {
        let Some(part) = pkg.part(&path) else {
            return BTreeMap::new();
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&part.data) else {
            return BTreeMap::new();
        };
        let mut out = BTreeMap::new();
        if let Some(items) = value
            .pointer("/_properties/Components/_elements")
            .and_then(|v| v.as_array())
        {
            for item in items {
                if item.get("_object_class").and_then(|v| v.as_str()) != Some("Simulink.SolverCC") {
                    continue;
                }
                if let Some(props) = item.pointer("/_properties").and_then(|v| v.as_object()) {
                    for (key, value) in props {
                        if let Some(s) = value.as_str() {
                            out.insert(key.clone(), s.to_string());
                        } else if value.is_number() || value.is_boolean() {
                            out.insert(key.clone(), value.to_string());
                        }
                    }
                }
            }
        }
        if !out.is_empty() {
            return out;
        }
    }
    for (info, fallback) in [
        ("simulink/configSetInfo.xml", "simulink/configSet0.xml"),
        ("", "simulink/configSet0.xml"),
    ] {
        let target = if !info.is_empty() {
            let doc = match pkg.part(info) {
                Some(p) => match parse_xml(info, &p.data) {
                    Ok(d) => Some(d),
                    Err(e) => {
                        warnings.push(format!("{info}: could not be parsed ({e}); ignored"));
                        None
                    }
                },
                None => None,
            };
            doc.and_then(|d| {
                d.root()
                    .and_then(|r| {
                        r.children_named("ConfigSet")
                            .find(|e| e.attr("Active").as_deref() == Some("true"))
                    })
                    .and_then(|e| e.attr("PartName"))
            })
            .map(|p| p.trim_start_matches('/').to_string())
            .unwrap_or_else(|| fallback.to_string())
        } else {
            fallback.to_string()
        };
        if let Some(part) = pkg.part(&target) {
            let doc = match parse_xml(&target, &part.data) {
                Ok(d) => d,
                Err(e) => {
                    warnings.push(format!("{target}: could not be parsed ({e}); ignored"));
                    continue;
                }
            };
            if let Some(root) = doc.root() {
                let mut solver = None;
                fn find_solver(
                    e: &XmlElement,
                    inside: bool,
                    out: &mut Option<BTreeMap<String, String>>,
                ) {
                    let is_solver = inside
                        || (e.name == "Object"
                            && e.attr("ClassName").as_deref() == Some("Simulink.SolverCC"));
                    if is_solver {
                        let values: BTreeMap<_, _> = e
                            .children_named("P")
                            .filter_map(|p| Some((p.attr("Name")?, p.text())))
                            .collect();
                        if !values.is_empty() {
                            *out = Some(values);
                        }
                    }
                    for c in e.elements() {
                        find_solver(c, is_solver, out);
                    }
                }
                find_solver(root, false, &mut solver);
                if let Some(values) = solver {
                    return values;
                }
            }
        }
    }
    BTreeMap::new()
}

/// Reads an SLX from bytes.
pub fn read_slx_bytes(bytes: &[u8], model_name: &str) -> Result<SlxImport, SlxError> {
    let pkg = read_package(bytes)?;
    let split = pkg.part(ROOT_SYS).is_some();
    if let Some(part) = pkg.part("simulink/blockDiagram.json") {
        let value: serde_json::Value = serde_json::from_slice(&part.data)
            .map_err(|_| SlxError::Unsupported("cannot parse blockDiagram.json".into()))?;
        if value.get("BlockDiagram").is_none() {
            let root_key = value
                .as_object()
                .and_then(|o| o.keys().next())
                .map(String::as_str)
                .unwrap_or("?");
            return Err(SlxError::Unsupported(format!(
                "unsupported blockDiagram.json root '{root_key}'"
            )));
        }
    }
    // `blockdiagram.xml` is a required part: one that exists but cannot be parsed is an
    // error, not a model that happens to look empty.
    if pkg.part(BD).is_some() {
        let doc = required_part(
            &pkg,
            BD,
            "no simulink/blockdiagram.xml or systems/system_root.xml",
        )?;
        if doc
            .root()
            .is_some_and(|r| r.children_named("Library").next().is_some())
        {
            return Err(SlxError::Unsupported(
                "this SLX is a block library, not a model".into(),
            ));
        }
    }
    let raw_sids = collect_sids(&pkg, split)?;
    let assigned = assign_sids(&raw_sids);
    let mut r = Reader {
        pkg: &pkg,
        report: ImportReport::default(),
        max_line: 0,
        sid_map: assigned.lookup,
        sid_assignments: assigned.occurrences,
        sid_occurrences: BTreeMap::new(),
        physical: 0,
        orientation_approximated: 0,
    };
    if assigned.renumbered > 0 {
        r.report.info.push(format!(
            "{} block SIDs were renumbered (non-numeric or duplicate SID)",
            assigned.renumbered
        ));
    }
    let root = if split {
        let doc = required_part(&pkg, ROOT_SYS, "cannot parse system_root.xml")?;
        let el = doc
            .root()
            .cloned()
            .ok_or_else(|| SlxError::Unsupported("empty system_root.xml".into()))?;
        r.system(&el, model_name)
    } else {
        let doc = required_part(
            &pkg,
            BD,
            "no simulink/blockdiagram.xml or systems/system_root.xml",
        )?;
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
    if r.orientation_approximated > 0 {
        r.report.info.push(format!(
            "{} blocks rotated by 180 degrees were mapped to a flip; port order may differ",
            r.orientation_approximated
        ));
    }
    // Optional parts that cannot be parsed are collected here and reported at the end.
    let mut part_warnings: Vec<String> = Vec::new();
    let settings = solver_settings(&pkg, &mut part_warnings);
    let config = config_values(&pkg, &mut part_warnings);
    if pkg
        .part("simulink/configSet0.json")
        .is_some_and(|p| serde_json::from_slice::<serde_json::Value>(&p.data).is_err())
    {
        r.report
            .warnings
            .push("cannot parse configSet0.json".into());
    }
    let value = |key: &str| {
        settings
            .get(key)
            .cloned()
            .or_else(|| config.get(key).cloned())
    };
    let mut num = |k: &str, default: f64| {
        let raw = value(k)?;
        if raw == "auto" && ["MaxStep", "RelTol", "AbsTol"].contains(&k) {
            return None;
        }
        if raw.eq_ignore_ascii_case("inf") && k == "StopTime" {
            r.report
                .warnings
                .push("StopTime inf is not supported; using 10".into());
            return Some(default);
        }
        match raw.trim().parse::<f64>() {
            Ok(v) if v.is_finite() => Some(v),
            _ => {
                r.report
                    .warnings
                    .push(format!("{k} '{raw}' is not a number; using {default}"));
                Some(default)
            }
        }
    };
    if let Some(v) = num("StartTime", model.config.start_time) {
        model.config.start_time = v;
    }
    if let Some(v) = num("StopTime", model.config.stop_time) {
        model.config.stop_time = v;
    }
    if let Some(v) = num("RelTol", model.config.rel_tol) {
        model.config.rel_tol = v;
    }
    if let Some(v) = num("AbsTol", model.config.abs_tol) {
        model.config.abs_tol = v;
    }
    if let Some(raw) = value("MaxStep") {
        if raw != "auto" {
            if let Ok(v) = raw.trim().parse::<f64>() {
                if v.is_finite() {
                    model.config.max_step = Some(v);
                } else {
                    r.report
                        .warnings
                        .push(format!("MaxStep '{raw}' is not a number; using default"));
                }
            } else {
                r.report
                    .warnings
                    .push(format!("MaxStep '{raw}' is not a number; using default"));
            }
        }
    }
    if let Some(v) = settings.get("FixedStep").filter(|v| *v != "auto") {
        r.report.warnings.push(format!(
            "fixed solver step '{v}' is not supported; using auto (RK45)"
        ));
    }
    match settings
        .get("SolverName")
        .or_else(|| settings.get("Solver"))
        .cloned()
        .or_else(|| config.get("SolverName").cloned())
        .or_else(|| config.get("Solver").cloned())
        .as_deref()
    {
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
    let baseline = model.clone();
    // Parts the importer never reads are checked too: one that exists but is not
    // well-formed XML means the package is damaged, and the user should hear about it.
    for part in &pkg.parts {
        if !part.name.ends_with(".xml") {
            continue;
        }
        if let Err(e) = parse_xml(&part.name, &part.data) {
            let prefix = format!("{}: could not be parsed (", part.name);
            let already_reported = part_warnings
                .iter()
                .chain(r.report.warnings.iter())
                .any(|warning| warning.starts_with(&prefix));
            if !already_reported {
                part_warnings.push(format!("{}: could not be parsed ({e}); ignored", part.name));
            }
        }
    }
    r.report.warnings.extend(part_warnings);
    let report = r.report;
    Ok(SlxImport {
        model,
        report,
        preserved: PreservedData {
            package: pkg.clone(),
            split_systems: split,
            sid_map: assigned.reverse,
            baseline,
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

fn sid_text(id: BlockId, sid_map: Option<&BTreeMap<BlockId, String>>) -> String {
    sid_map
        .and_then(|m| m.get(&id))
        .cloned()
        .unwrap_or_else(|| id.0.to_string())
}

fn branch_el(
    dst: Option<PortRef>,
    brs: &[Branch],
    sid_map: Option<&BTreeMap<BlockId, String>>,
) -> Vec<XmlElement> {
    let mut v = vec![];
    if let Some(d) = dst {
        v.push(p_el(
            "Dst",
            &format!("{}#in:{}", sid_text(d.block, sid_map), d.port),
        ));
    }
    for b in brs {
        let mut e = XmlElement::new("Branch");
        for c in branch_el(b.dst, &b.branches, sid_map) {
            e.push(c);
        }
        v.push(e);
    }
    v
}

fn system_el(
    sys: &System,
    split: bool,
    parts: &mut Vec<(String, XmlElement)>,
    sid_map: Option<&BTreeMap<BlockId, String>>,
) -> XmlElement {
    let mut s = XmlElement::new("System");
    for b in &sys.blocks {
        let mut e = XmlElement::new("Block");
        let (bt, extra) = if b.block_type == "PIDController" {
            ("Reference", true)
        } else {
            (b.block_type.as_str(), false)
        };
        e.set_attr("BlockType", bt);
        e.set_attr("Name", &b.name);
        e.set_attr("SID", &sid_text(b.sid, sid_map));
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
        if b.params.contains_key(OPAQUE_INPUTS) || b.params.contains_key(OPAQUE_OUTPUTS) {
            let mut pc = XmlElement::new("PortCounts");
            if let Some(n) = b.params.get(OPAQUE_INPUTS) {
                pc.set_attr("in", n);
            }
            if let Some(n) = b.params.get(OPAQUE_OUTPUTS) {
                pc.set_attr("out", n);
            }
            pc.self_closing = true;
            e.push(pc);
        }
        let (rotation, mirror) = orientation_to_slx(b.orientation);
        if let Some(rotation) = rotation {
            e.push(p_el("BlockRotation", rotation));
        }
        if mirror {
            e.push(p_el("BlockMirror", "on"));
        }
        if extra {
            e.push(p_el(
                "LibrarySourceBlock",
                "simulink/Continuous/PID Controller",
            ));
            e.push(p_el("SourceBlock", "slpidlib/PID Controller"));
            e.push(p_el("SourceType", "PID 1dof"));
            let mut inst = XmlElement::new("InstanceData");
            for k in ["P", "I", "D", "N", "Controller", "Form", "TimeDomain"] {
                if let Some(v) = b.params.get(k) {
                    inst.push(p_el(k, v));
                }
            }
            e.push(inst);
        } else {
            for (k, v) in &b.params {
                if !k.starts_with("__") {
                    e.push(p_el(k, v));
                }
            }
        }
        if let Some(c) = &b.system {
            let child = system_el(c, split, parts, sid_map);
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
            &format!("{}#out:{}", sid_text(l.src.block, sid_map), l.src.port),
        ));
        for c in branch_el(l.dst, &l.branches, sid_map) {
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
    if let Some(preserved) = preserved {
        let mut package = crate::patch::patch_package(model, preserved)?;
        patch_orientations(&mut package, model, preserved)?;
        return write_package(&package);
    }
    let split = preserved.map(|p| p.split_systems).unwrap_or(false);
    let mut extra = vec![];
    let root_sys = system_el(
        &model.root,
        split,
        &mut extra,
        preserved.map(|p| &p.sid_map),
    );
    // Exporting a model that was not imported has no original package to patch.
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

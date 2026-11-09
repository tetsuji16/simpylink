//! Minimal, loss preserving updates to imported system XML.
use crate::map::PreservedData;
use crate::opc::OpcPackage;
use crate::xml::{parse_xml, write_xml, XmlElement, XmlNode};
use sim_core::ids::BlockId;
use sim_core::model::{Block, Line, Model, System};
use std::collections::BTreeMap;

fn set_p(block: &mut XmlElement, key: &str, value: &str) {
    for node in &mut block.children {
        if let XmlNode::Element(p) = node {
            if p.name == "P" && p.attr("Name").as_deref() == Some(key) {
                p.set_text(value);
                return;
            }
        }
    }
    let mut p = XmlElement::new("P");
    p.set_attr("Name", key);
    p.set_text(value);
    let at = block
        .children
        .iter()
        .position(|n| matches!(n, XmlNode::Element(e) if e.name == "Position"))
        .map(|i| i + 1)
        .unwrap_or(block.children.len());
    block.children.insert(at, XmlNode::Element(p));
}

fn block_map(s: &System) -> BTreeMap<u32, &Block> {
    s.blocks.iter().map(|b| (b.sid.0, b)).collect()
}

fn p_el(name: &str, value: &str) -> XmlElement {
    let mut p = XmlElement::new("P");
    p.set_attr("Name", name);
    p.set_text(value);
    p
}

fn block_element(block: &Block, sid: &str, split: bool) -> XmlElement {
    let mut e = XmlElement::new("Block");
    let pid = block.block_type == "PIDController";
    e.set_attr(
        "BlockType",
        if pid { "Reference" } else { &block.block_type },
    );
    e.set_attr("Name", &block.name);
    e.set_attr("SID", sid);
    e.push(p_el(
        "Position",
        &format!(
            "[{}, {}, {}, {}]",
            block.position.left, block.position.top, block.position.right, block.position.bottom
        ),
    ));
    if pid {
        let mut counts = XmlElement::new("PortCounts");
        counts.set_attr("in", "1");
        counts.set_attr("out", "1");
        counts.self_closing = true;
        e.push(counts);
        e.push(p_el(
            "LibrarySourceBlock",
            "simulink/Continuous/PID Controller",
        ));
        e.push(p_el("SourceBlock", "slpidlib/PID Controller"));
        e.push(p_el("SourceType", "PID 1dof"));
        let mut instance = XmlElement::new("InstanceData");
        for key in ["P", "I", "D", "N", "Controller", "Form", "TimeDomain"] {
            if let Some(value) = block.params.get(key) {
                instance.push(p_el(key, value));
            }
        }
        e.push(instance);
    }
    if block.params.contains_key("__opaque_inputs") || block.params.contains_key("__opaque_outputs")
    {
        let mut counts = XmlElement::new("PortCounts");
        if let Some(n) = block.params.get("__opaque_inputs") {
            counts.set_attr("in", n);
        }
        if let Some(n) = block.params.get("__opaque_outputs") {
            counts.set_attr("out", n);
        }
        counts.self_closing = true;
        e.push(counts);
    }
    for (key, value) in &block.params {
        if pid {
            continue;
        }
        if !key.starts_with("__") {
            e.push(p_el(key, value));
        }
    }
    if split && block.system.is_some() {
        let mut child = XmlElement::new("System");
        child.set_attr("Ref", &format!("system_{sid}"));
        child.self_closing = true;
        e.push(child);
    }
    e
}

fn line_elements(line: &Line, sid_map: &BTreeMap<BlockId, String>) -> Vec<XmlElement> {
    let sid = |id: BlockId| {
        sid_map
            .get(&id)
            .cloned()
            .unwrap_or_else(|| id.0.to_string())
    };
    fn branches(
        dst: Option<sim_core::model::PortRef>,
        bs: &[sim_core::model::Branch],
        sid: &dyn Fn(BlockId) -> String,
    ) -> Vec<XmlElement> {
        let mut out = Vec::new();
        if let Some(d) = dst {
            out.push(p_el("Dst", &format!("{}#in:{}", sid(d.block), d.port)));
        }
        for b in bs {
            let mut e = XmlElement::new("Branch");
            for child in branches(b.dst, &b.branches, sid) {
                e.push(child);
            }
            out.push(e);
        }
        out
    }
    let mut e = XmlElement::new("Line");
    if !line.name.is_empty() {
        e.push(p_el("Name", &line.name));
    }
    e.push(p_el(
        "Src",
        &format!("{}#out:{}", sid(line.src.block), line.src.port),
    ));
    for child in branches(line.dst, &line.branches, &sid) {
        e.push(child);
    }
    vec![e]
}

fn build_system(
    sys: &System,
    split: bool,
    sid_map: &BTreeMap<BlockId, String>,
    parts: &mut Vec<(String, XmlElement)>,
) -> XmlElement {
    let mut root = XmlElement::new("System");
    for block in &sys.blocks {
        let sid = sid_map
            .get(&block.sid)
            .cloned()
            .unwrap_or_else(|| block.sid.0.to_string());
        let mut element = block_element(block, &sid, split);
        if let Some(child) = &block.system {
            let child_xml = build_system(child, split, sid_map, parts);
            if split {
                parts.push((format!("simulink/systems/system_{sid}.xml"), child_xml));
            } else {
                element
                    .children
                    .retain(|n| !matches!(n, XmlNode::Element(e) if e.name == "System"));
                element.push(child_xml);
            }
        }
        root.push(element);
    }
    for line in &sys.lines {
        for element in line_elements(line, sid_map) {
            root.push(element);
        }
    }
    root
}

fn patch_system(
    xml: &mut XmlElement,
    before: &System,
    after: &System,
    sid_map: &BTreeMap<sim_core::ids::BlockId, String>,
    split: bool,
) -> Result<(), crate::error::SlxError> {
    let old = block_map(before);
    let new = block_map(after);
    let original: BTreeMap<String, u32> = sid_map
        .iter()
        .map(|(id, sid)| (sid.clone(), id.0))
        .collect();
    let mut present = std::collections::BTreeSet::new();
    for node in &mut xml.children {
        let XmlNode::Element(block) = node else {
            continue;
        };
        if block.name != "Block" {
            continue;
        }
        let Some(id) = block
            .attr("SID")
            .and_then(|sid| original.get(&sid).copied().or_else(|| sid.parse().ok()))
        else {
            continue;
        };
        present.insert(id);
        let Some(old) = old.get(&id) else { continue };
        let Some(new) = new.get(&id) else { continue };
        if old.name != new.name {
            block.set_attr("Name", &new.name);
        }
        if old.position != new.position {
            set_p(
                block,
                "Position",
                &format!(
                    "[{}, {}, {}, {}]",
                    new.position.left, new.position.top, new.position.right, new.position.bottom
                ),
            );
        }
        for (key, value) in &new.params {
            if key.starts_with("__") {
                continue;
            }
            if old.params.get(key) != Some(value) {
                set_p(block, key, value);
            }
        }
        if let (Some(old_child), Some(new_child)) = (&old.system, &new.system) {
            if let Some(child_xml) = block.children.iter_mut().find_map(|n| match n {
                XmlNode::Element(e) if e.name == "System" && e.attr("Ref").is_none() => Some(e),
                _ => None,
            }) {
                patch_system(child_xml, old_child, new_child, sid_map, split)?;
            }
        }
    }
    if let Some((missing, _)) = old
        .iter()
        .find(|(id, _)| !present.contains(id) && new.contains_key(id))
    {
        return Err(crate::error::SlxError::Unsupported(format!(
            "block SID {} not found in original XML",
            missing
        )));
    }
    xml.children.retain(|n| match n {
        XmlNode::Element(e) if e.name == "Block" => e
            .attr("SID")
            .and_then(|s| original.get(&s).copied().or_else(|| s.parse().ok()))
            .is_none_or(|id| new.contains_key(&id)),
        _ => true,
    });
    let original_line_set = &before.lines;
    if original_line_set != &after.lines {
        xml.children
            .retain(|n| !matches!(n, XmlNode::Element(e) if e.name == "Line"));
        for line in &after.lines {
            for e in line_elements(line, sid_map) {
                xml.push(e);
            }
        }
    }
    let insertion = xml
        .children
        .iter()
        .position(|n| matches!(n, XmlNode::Element(e) if e.name == "Line"))
        .unwrap_or(xml.children.len());
    let additions: Vec<_> = new
        .iter()
        .filter(|(id, _)| !old.contains_key(id))
        .map(|(_, b)| {
            let sid = sid_map
                .get(&b.sid)
                .cloned()
                .unwrap_or_else(|| b.sid.0.to_string());
            let mut element = block_element(b, &sid, split);
            if !split {
                if let Some(child) = &b.system {
                    element.push(build_system(child, false, sid_map, &mut Vec::new()));
                }
            }
            element
        })
        .collect();
    for (offset, block) in additions.into_iter().enumerate() {
        xml.children
            .insert(insertion + offset, XmlNode::Element(block));
    }
    Ok(())
}

/// Applies model edits to preserved XML without rebuilding unrelated package parts.
pub(crate) fn patch_package(
    model: &Model,
    preserved: &PreservedData,
) -> Result<OpcPackage, crate::error::SlxError> {
    if model == &preserved.baseline {
        return Ok(preserved.package.clone());
    }
    let mut pkg = preserved.package.clone();
    let part = if preserved.split_systems {
        "simulink/systems/system_root.xml"
    } else {
        "simulink/blockdiagram.xml"
    };
    if let Some(original) = pkg.part(part).map(|p| p.data.clone()) {
        let mut doc = parse_xml(part, &original)?;
        if let Some(root) = doc.root_mut() {
            let system = if preserved.split_systems {
                Some(root)
            } else {
                root.children.iter_mut().find_map(|n| match n {
                    XmlNode::Element(e) if e.name == "Model" => {
                        e.children.iter_mut().find_map(|n| match n {
                            XmlNode::Element(s) if s.name == "System" => Some(s),
                            _ => None,
                        })
                    }
                    _ => None,
                })
            };
            if let Some(system) = system {
                patch_system(
                    system,
                    &preserved.baseline.root,
                    &model.root,
                    &preserved.sid_map,
                    preserved.split_systems,
                )?;
            }
        }
        pkg.set_part(part, write_xml(&doc));
    }
    if preserved.split_systems {
        fn children(
            before: &System,
            after: &System,
            pkg: &mut OpcPackage,
            p: &PreservedData,
        ) -> Result<(), crate::error::SlxError> {
            fn remove_subtree(sys: &System, pkg: &mut OpcPackage, p: &PreservedData) {
                for block in &sys.blocks {
                    if let Some(child) = &block.system {
                        let name = p
                            .sid_map
                            .get(&block.sid)
                            .cloned()
                            .unwrap_or_else(|| block.sid.0.to_string());
                        let path = format!("simulink/systems/system_{name}.xml");
                        pkg.parts.retain(|part| {
                            part.name != path
                                && part.name
                                    != format!("simulink/systems/_rels/system_{name}.xml.rels")
                        });
                        remove_subtree(child, pkg, p);
                    }
                }
            }
            for old in &before.blocks {
                let Some(new) = after.blocks.iter().find(|b| b.sid == old.sid) else {
                    if old.system.is_some() {
                        let ref_name = p
                            .sid_map
                            .get(&old.sid)
                            .cloned()
                            .unwrap_or_else(|| old.sid.0.to_string());
                        let path = format!("simulink/systems/system_{ref_name}.xml");
                        pkg.parts.retain(|part| {
                            part.name != path
                                && part.name
                                    != format!("simulink/systems/_rels/system_{ref_name}.xml.rels")
                        });
                        if let Some(child) = &old.system {
                            remove_subtree(child, pkg, p);
                        }
                    }
                    continue;
                };
                if let (Some(old_sys), Some(new_sys)) = (&old.system, &new.system) {
                    let ref_name = p
                        .sid_map
                        .get(&old.sid)
                        .cloned()
                        .unwrap_or_else(|| old.sid.0.to_string());
                    let path = format!("simulink/systems/system_{ref_name}.xml");
                    if let Some(original) = pkg.part(&path).map(|part| part.data.clone()) {
                        let mut doc = parse_xml(&path, &original)?;
                        if let Some(root) = doc.root_mut() {
                            patch_system(root, old_sys, new_sys, &p.sid_map, true)?;
                        }
                        pkg.set_part(&path, write_xml(&doc));
                    } else if old_sys != new_sys {
                        return Err(crate::error::SlxError::Unsupported(format!(
                            "system part {path} not found in original package"
                        )));
                    }
                    children(old_sys, new_sys, pkg, p)?;
                }
            }
            for new in after
                .blocks
                .iter()
                .filter(|b| !before.blocks.iter().any(|old| old.sid == b.sid))
            {
                if let Some(child_sys) = &new.system {
                    let ref_name = p
                        .sid_map
                        .get(&new.sid)
                        .cloned()
                        .unwrap_or_else(|| new.sid.0.to_string());
                    let mut extra = Vec::new();
                    let root = build_system(child_sys, true, &p.sid_map, &mut extra);
                    pkg.set_part(
                        &format!("simulink/systems/system_{ref_name}.xml"),
                        write_xml(&crate::xml::XmlDocument {
                            nodes: vec![XmlNode::Element(root)],
                        }),
                    );
                    for (name, element) in extra {
                        pkg.set_part(
                            &name,
                            write_xml(&crate::xml::XmlDocument {
                                nodes: vec![XmlNode::Element(element)],
                            }),
                        );
                    }
                    children(&System::default(), child_sys, pkg, p)?;
                }
            }
            Ok(())
        }
        children(&preserved.baseline.root, &model.root, &mut pkg, preserved)?;
    }
    if pkg
        .parts
        .iter()
        .any(|part| part.name.starts_with("simulink/systems/") && part.name.ends_with(".xml"))
    {
        if let Some(content_types) = pkg.part("[Content_Types].xml").map(|p| p.data.clone()) {
            if let Ok(mut doc) = parse_xml("[Content_Types].xml", &content_types) {
                if let Some(root) = doc.root_mut() {
                    let has_xml_default = root
                        .children_named("Default")
                        .any(|e| e.attr("Extension").as_deref() == Some("xml"));
                    if !has_xml_default {
                        let mut default = XmlElement::new("Default");
                        default.set_attr("Extension", "xml");
                        default.set_attr(
                            "ContentType",
                            "application/vnd.mathworks.simulink.model+xml",
                        );
                        default.self_closing = true;
                        root.push(default);
                        pkg.set_part("[Content_Types].xml", write_xml(&doc));
                    }
                }
            }
        }
    }
    Ok(pkg)
}

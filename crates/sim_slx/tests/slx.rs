//! SLX layer tests. Fixtures here are synthetic; they are NOT files saved by Simulink.
use sim_core::json::load_from_path;
use sim_slx::opc::*;
use sim_slx::xml::*;
use sim_slx::*;
use std::collections::BTreeSet;
use std::io::Write;

fn fx(name: &str) -> sim_core::model::Model {
    load_from_path(&std::path::PathBuf::from(format!(
        "{}/../../tests/fixtures/models/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    )))
    .unwrap()
}

fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let o = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (n, d) in entries {
            w.start_file(*n, o).unwrap();
            w.write_all(d).unwrap();
        }
        w.finish().unwrap();
    }
    buf.into_inner()
}

const CT: &[u8] = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/a/b.xml" ContentType="application/x-test"/></Types>"#;

#[test]
fn opc_lossless() {
    let z = make_zip(&[
        ("[Content_Types].xml", CT),
        ("_rels/.rels", b"<Relationships/>"),
        ("a/b.xml", b"<b/>"),
        ("c.xml", b"<c/>"),
        ("bin/data.bin", &[0, 1, 2, 255]),
    ]);
    let p = read_package(&z).unwrap();
    assert_eq!(
        p.parts.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
        vec![
            "[Content_Types].xml",
            "_rels/.rels",
            "a/b.xml",
            "c.xml",
            "bin/data.bin"
        ]
    );
    assert_eq!(
        p.content_type("a/b.xml").as_deref(),
        Some("application/x-test")
    );
    assert_eq!(p.content_type("c.xml").as_deref(), Some("application/xml"));
    assert_eq!(p.content_type("bin/data.bin"), None);
    assert_eq!(read_package(&write_package(&p).unwrap()).unwrap(), p);
    assert!(read_package(&[]).is_err());
    assert!(
        matches!(read_package(&make_zip(&[("x.xml", b"<x/>")])), Err(SlxError::Package(m)) if m.contains("Content_Types"))
    );
}

#[test]
fn xml_lossless() {
    let x = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ModelInformation Version=\"1.0\">\n  <Model Name=\"m\">\n    <P Name=\"Gain\">3 &amp; 4</P>\n    <!-- c -->\n    <![CDATA[x<y]]>\n    <Empty/>\n    <N>増幅</N>\n  </Model>\n</ModelInformation>\n";
    let d = parse_xml("t", x.as_bytes()).unwrap();
    assert_eq!(String::from_utf8(write_xml(&d)).unwrap(), x);
    let model = d.root().unwrap().children_named("Model").next().unwrap();
    let p = model.children_named("P").next().unwrap();
    assert_eq!(
        (p.attr("Name").as_deref(), p.text().as_str()),
        (Some("Gain"), "3 & 4")
    );
    let mut a = parse_xml("t", b"<A b=\"1\" a=\"2\"/>").unwrap();
    a.root_mut().unwrap().set_attr("a", "x&y");
    assert_eq!(
        String::from_utf8(write_xml(&a)).unwrap(),
        "<A b=\"1\" a=\"x&amp;y\"/>"
    );
    let mut t = parse_xml("t", b"<P>v</P>").unwrap();
    t.root_mut().unwrap().set_text("1<2");
    assert_eq!(String::from_utf8(write_xml(&t)).unwrap(), "<P>1&lt;2</P>");
    assert_eq!(unescape("&#65;&#x42;&lt;"), "AB<");
    assert!(parse_xml("t", b"<A><B></A>").is_err());
    assert!(parse_xml("t", b"<A>").is_err());
    assert!(parse_xml("t", b"").unwrap().root().is_none());
}

fn connections(m: &sim_core::model::Model) -> BTreeSet<(u32, u32, u32, u32)> {
    fn rec(s: &sim_core::model::System, out: &mut BTreeSet<(u32, u32, u32, u32)>) {
        for l in &s.lines {
            for d in l.destinations() {
                out.insert((l.src.block.0, l.src.port, d.block.0, d.port));
            }
        }
        for b in &s.blocks {
            if let Some(c) = &b.system {
                rec(c, out);
            }
        }
    }
    let mut out = BTreeSet::new();
    rec(&m.root, &mut out);
    out
}

#[test]
fn roundtrip_fixtures() {
    for name in [
        "step_gain_scope",
        "first_order_lag",
        "pi_loop",
        "math_blocks",
        "subsystem_gain",
        "accumulator",
    ] {
        let m = fx(name);
        let bytes = write_slx_bytes(&m, None).unwrap();
        let back = read_slx_bytes(&bytes, name).unwrap();
        assert!(
            back.report.unsupported_blocks.is_empty(),
            "{name}: {:?}",
            back.report
        );
        assert_eq!(connections(&back.model), connections(&m), "{name}");
        let strip = |s: &sim_core::model::System| {
            s.blocks
                .iter()
                .map(|b| {
                    (
                        b.sid,
                        b.block_type.clone(),
                        b.name.clone(),
                        b.position,
                        b.params.clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(strip(&back.model.root), strip(&m.root), "{name}");
        assert_eq!(back.model.config.stop_time, m.config.stop_time);
        // second round trip through the preserved package
        let again = read_slx_bytes(
            &write_slx_bytes(&back.model, Some(&back.preserved)).unwrap(),
            name,
        )
        .unwrap();
        assert_eq!(connections(&again.model), connections(&m));
    }
}

#[test]
fn split_system_layout_and_unsupported() {
    let root = br#"<?xml version="1.0" encoding="utf-8"?>
<System>
  <Block BlockType="Constant" Name="Constant" SID="1"><P Name="Position">[20, 20, 50, 50]</P><P Name="Value">2</P></Block>
  <Block BlockType="Gain" Name="Gain" SID="2"><P Name="Position">[100, 20, 130, 50]</P><P Name="Gain">3</P></Block>
  <Block BlockType="Mux" Name="Mux" SID="3"><P Name="Position">[180, 20, 185, 60]</P></Block>
  <Block BlockType="Scope" Name="Scope" SID="4"><P Name="Position">[240, 20, 270, 52]</P></Block>
  <Line><P Name="Src">1#out:1</P><P Name="Dst">2#in:1</P></Line>
  <Line><P Name="Src">2#out:1</P><Branch><P Name="Dst">4#in:1</P></Branch><Branch><P Name="Dst">3#in:1</P></Branch></Line>
</System>"#;
    let cfg = br#"<ConfigSet><Object><P Name="StopTime">5.0</P><P Name="Solver">ode23</P></Object></ConfigSet>"#;
    let z = make_zip(&[
        ("[Content_Types].xml", CT),
        ("simulink/systems/system_root.xml", root),
        ("simulink/configSet0.xml", cfg),
        ("metadata/thumbnail.png", &[1, 2, 3]),
    ]);
    let imp = read_slx_bytes(&z, "synthetic").unwrap();
    assert_eq!(imp.model.root.blocks.len(), 4);
    assert_eq!(imp.model.config.stop_time, 5.0);
    assert_eq!(imp.model.config.solver, sim_core::model::SolverKind::Ode23);
    assert_eq!(
        imp.report.unsupported_blocks,
        vec![("synthetic/Mux".to_string(), "Mux".to_string())]
    );
    assert_eq!(imp.model.root.blocks[1].params["Gain"], "3");
    assert_eq!(imp.model.root.blocks[1].params["SampleTime"], "-1");
    let out = write_slx_bytes(&imp.model, Some(&imp.preserved)).unwrap();
    let pkg = read_package(&out).unwrap();
    assert!(
        pkg.part("metadata/thumbnail.png").is_some()
            && pkg.part("simulink/configSet0.xml").is_some()
    );
    assert_eq!(
        connections(&read_slx_bytes(&out, "s").unwrap().model),
        connections(&imp.model)
    );
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("a.slx");
    std::fs::write(&p, b"x").unwrap();
    assert!(write_slx(&imp.model, None, &p).is_err());
}

#[test]
fn composite_sids_do_not_collide_and_are_restored() {
    let root = br#"<System><Block BlockType="Constant" Name="C" SID="25"><P Name="Value">2</P></Block><Block BlockType="SubSystem" Name="S" SID="36"><System Ref="system_36"/></Block><Line><P Name="Src">25#out:1</P><P Name="Dst">36#in:1</P></Line></System>"#;
    let child = br#"<System><Block BlockType="Inport" Name="In1" SID="36::1"/><Block BlockType="Gain" Name="G" SID="36::25"><P Name="Gain">3</P></Block><Block BlockType="Outport" Name="Out1" SID="36::2"/><Line><P Name="Src">36::1#out:1</P><P Name="Dst">36::25#in:1</P></Line></System>"#;
    let z = make_zip(&[
        ("[Content_Types].xml", CT),
        ("simulink/systems/system_root.xml", root),
        ("simulink/systems/system_36.xml", child),
    ]);
    let imp = read_slx_bytes(&z, "m").unwrap();
    assert_eq!(imp.model.root.blocks[0].sid.0, 25);
    assert_eq!(imp.model.root.blocks[1].sid.0, 36);
    let child = imp.model.root.blocks[1].system.as_ref().unwrap();
    assert_eq!(
        child.blocks.iter().map(|b| b.sid.0).collect::<Vec<_>>(),
        vec![37, 38, 39]
    );
    assert_eq!(child.lines[0].src.block.0, 37);
    assert_eq!(child.lines[0].dst.unwrap().block.0, 38);
    assert_eq!(imp.preserved.sid_map[&sim_core::ids::BlockId(38)], "36::25");
    let output = write_slx_bytes(&imp.model, Some(&imp.preserved)).unwrap();
    assert_eq!(read_slx_bytes(&output, "m").unwrap().model, imp.model);
    let outpkg = read_package(&output).unwrap();
    let xml = String::from_utf8_lossy(&outpkg.part("simulink/systems/system_36.xml").unwrap().data);
    assert!(xml.contains("SID=\"36::25\""));
    assert!(xml.contains("36::1#out:1"));
}

#[test]
fn unchanged_import_export_keeps_every_original_part() {
    let root = br#"<System><Block BlockType="Gain" Name="G" SID="1"><P Name="Position">[10, 10, 40, 40]</P><P Name="Gain">3</P><P Name="ZOrder">2</P></Block></System>"#;
    let z = make_zip(&[
        ("[Content_Types].xml", CT),
        ("simulink/systems/system_root.xml", root),
        (
            "simulink/systems/_rels/system_root.xml.rels",
            b"<Relationships/>",
        ),
        ("simulink/stateflow/machine.xml", b"<Machine/>"),
    ]);
    let imp = read_slx_bytes(&z, "m").unwrap();
    let expected = write_package(&imp.preserved.package).unwrap();
    assert_eq!(
        write_slx_bytes(&imp.model, Some(&imp.preserved)).unwrap(),
        expected
    );
}

#[test]
fn changed_parameter_patches_only_its_original_value() {
    let root = br#"<System><Block BlockType="Gain" Name="G" SID="1"><P Name="Position">[10, 10, 40, 40]</P><P Name="ZOrder">2</P><P Name="Gain">3</P><P Name="GraphicalSettings">keep</P></Block></System>"#;
    let z = make_zip(&[
        ("[Content_Types].xml", CT),
        ("simulink/systems/system_root.xml", root),
        ("simulink/stateflow/machine.xml", b"<Machine/>"),
    ]);
    let mut imp = read_slx_bytes(&z, "m").unwrap();
    imp.model.root.blocks[0]
        .params
        .insert("Gain".into(), "5".into());
    let out = read_package(&write_slx_bytes(&imp.model, Some(&imp.preserved)).unwrap()).unwrap();
    let patched =
        String::from_utf8_lossy(&out.part("simulink/systems/system_root.xml").unwrap().data);
    assert!(patched.contains("<P Name=\"Gain\">5</P>"));
    assert!(patched.contains("<P Name=\"GraphicalSettings\">keep</P>"));
    assert_eq!(
        out.part("simulink/stateflow/machine.xml").unwrap().data,
        b"<Machine/>"
    );
    assert_eq!(
        read_slx_bytes(&write_package(&out).unwrap(), "m")
            .unwrap()
            .model,
        imp.model
    );
}

#[test]
fn r2026b_json_solver_settings_and_infinity_fallback() {
    let root = br#"<System><Block BlockType="Constant" Name="C" SID="1"/></System>"#;
    let info = br#"{"ConfigSetInfo":[{"Active":true,"PartName":"/simulink/configSet0.json"}]}"#;
    let cfg = br#"{"_object_class":"Simulink.ConfigSet","_properties":{"Components":{"_elements":[{"_object_class":"Simulink.SolverCC","_properties":{"StopTime":"5","SolverName":"ode23","MaxStep":"1e-3"}}]}}}"#;
    let z = make_zip(&[
        ("[Content_Types].xml", CT),
        ("simulink/systems/system_root.xml", root),
        ("simulink/configSetInfo.json", info),
        ("simulink/configSet0.json", cfg),
    ]);
    let imp = read_slx_bytes(&z, "m").unwrap();
    assert_eq!(imp.model.config.stop_time, 5.0);
    assert_eq!(imp.model.config.solver, sim_core::model::SolverKind::Ode23);
    assert_eq!(imp.model.config.max_step, Some(1e-3));
    assert!(imp.report.warnings.is_empty());

    let inf = br#"{"_properties":{"Components":{"_elements":[{"_object_class":"Simulink.SolverCC","_properties":{"StopTime":"inf"}}]}}}"#;
    let z = make_zip(&[
        ("[Content_Types].xml", CT),
        ("simulink/systems/system_root.xml", root),
        ("simulink/configSet0.json", inf),
    ]);
    let imp = read_slx_bytes(&z, "m").unwrap();
    assert_eq!(imp.model.config.stop_time, 10.0);
    assert!(imp
        .report
        .warnings
        .iter()
        .any(|w| w.contains("StopTime inf")));
    assert!(!sim_core::validate::validate_model(&imp.model)
        .iter()
        .any(|d| d.code == "E_INVALID_CONFIG"));
}

#[test]
fn block_library_root_is_reported_as_unsupported() {
    let bd = br#"<ModelInformation><Library><System/></Library></ModelInformation>"#;
    let z = make_zip(&[
        ("[Content_Types].xml", CT),
        ("simulink/blockdiagram.xml", bd),
    ]);
    assert!(
        matches!(read_slx_bytes(&z, "lib"), Err(SlxError::Unsupported(message)) if message.contains("block library"))
    );
}

#[test]
fn reference_pid_controller_uses_instance_data() {
    let root = br#"<System><Block BlockType="Reference" Name="PID" SID="2"><PortCounts in="1" out="1"/><P Name="LibrarySourceBlock">simulink/Continuous/PID Controller</P><P Name="SourceBlock">slpidlib/PID Controller</P><InstanceData><P Name="P">2</P><P Name="I">3</P><P Name="D">0</P><P Name="N">100</P><P Name="Controller">PID</P><P Name="Form">Parallel</P></InstanceData></Block></System>"#;
    let z = make_zip(&[
        ("[Content_Types].xml", CT),
        ("simulink/systems/system_root.xml", root),
    ]);
    let imp = read_slx_bytes(&z, "m").unwrap();
    let pid = &imp.model.root.blocks[0];
    assert_eq!(pid.block_type, "PIDController");
    assert_eq!(pid.params["P"], "2");
    assert_eq!(pid.params["I"], "3");
    assert!(imp.report.unsupported_blocks.is_empty());
    let rerun = read_slx_bytes(&write_slx_bytes(&imp.model, None).unwrap(), "m").unwrap();
    assert_eq!(rerun.model.root.blocks[0].block_type, "PIDController");
}

#[test]
fn opaque_port_counts_preserve_lines_and_aggregate_physical_connections() {
    let root = br#"<System>
<Block BlockType="Constant" Name="C" SID="1"/><Block BlockType="Terminator" Name="T" SID="2"><PortCounts in="1"/></Block>
<Block BlockType="Constant" Name="C1" SID="3"/><Block BlockType="Demux" Name="D" SID="4"><PortCounts in="1" out="2"/></Block><Block BlockType="Scope" Name="S" SID="5"><P Name="NumInputPorts">2</P></Block>
<Block BlockType="SimscapeBlock" Name="P1" SID="6"/><Block BlockType="SimscapeBlock" Name="P2" SID="7"/>
<Line><P Name="Src">1#out:1</P><P Name="Dst">2#in:1</P></Line><Line><P Name="Src">3#out:1</P><P Name="Dst">4#in:1</P></Line>
<Line><P Name="Src">4#out:1</P><P Name="Dst">5#in:1</P></Line><Line><P Name="Src">4#out:2</P><P Name="Dst">5#in:2</P></Line>
<Line><P Name="Src">6#lconn:1</P><P Name="Dst">7#rconn:1</P></Line><Line><P Name="Src">6#lconn:1</P><P Name="Dst">7#rconn:1</P></Line><Line><P Name="Src">6#lconn:1</P><P Name="Dst">7#rconn:1</P></Line>
</System>"#;
    let z = make_zip(&[
        ("[Content_Types].xml", CT),
        ("simulink/systems/system_root.xml", root),
    ]);
    let imp = read_slx_bytes(&z, "m").unwrap();
    assert_eq!(imp.model.root.lines.len(), 4);
    assert!(!sim_core::validate::validate_model(&imp.model)
        .iter()
        .any(|d| d.code == "E_PORT_OUT_OF_RANGE"));
    assert!(imp
        .report
        .dropped
        .iter()
        .any(|d| d.contains("3 physical (Simscape) connections")));
    let out = write_slx_bytes(&imp.model, None).unwrap();
    let package = read_package(&out).unwrap();
    let xml = String::from_utf8_lossy(
        package
            .part("simulink/blockdiagram.xml")
            .map(|p| p.data.as_slice())
            .unwrap_or_default(),
    );
    assert!(!xml.contains("__opaque"));
    assert!(xml.contains("<PortCounts in=\"1\" out=\"2\"/>"));

    let invalid = br#"<System><Block BlockType="Unlisted" Name="X" SID="1"><PortCounts in="abc"/></Block></System>"#;
    let z = make_zip(&[
        ("[Content_Types].xml", CT),
        ("simulink/systems/system_root.xml", invalid),
    ]);
    let imp = read_slx_bytes(&z, "m").unwrap();
    assert!(!imp.model.root.blocks[0]
        .params
        .contains_key("__opaque_inputs"));
    assert_eq!(imp.report.warnings.len(), 1);
}

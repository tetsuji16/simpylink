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

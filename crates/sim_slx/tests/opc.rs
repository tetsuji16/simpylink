//! OPC package byte preservation and overwrite refusal (Issue #83 §H/§J).
use sim_slx::opc::*;
use sim_slx::SlxError;
use std::io::Write;

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
fn opc_keeps_bytes_and_refuses_to_overwrite_files() {
    let z = make_zip(&[
        ("[Content_Types].xml", CT),
        ("bin/data.bin", &[0, 1, 2, 255]),
    ]);
    let p = read_package(&z).unwrap();
    assert_eq!(p.part("bin/data.bin").unwrap().data, vec![0, 1, 2, 255]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.slx");
    write_package_file(&p, &path).unwrap();
    assert!(matches!(
        write_package_file(&p, &path),
        Err(SlxError::Io(_))
    ));
    assert_eq!(read_package_file(&path).unwrap(), p);
}

#[test]
fn invalid_content_types_xml_has_no_content_type() {
    let z = make_zip(&[
        ("[Content_Types].xml", b"<Types><Default"),
        ("c.xml", b"<c/>"),
    ]);
    let p = read_package(&z).unwrap();
    assert_eq!(p.content_type("c.xml"), None);
}

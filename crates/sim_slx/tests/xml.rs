//! Lossless XML attribute order and quote normalisation (Issue #84 §H).
use sim_slx::xml::*;

#[test]
fn xml_attr_order_and_single_quote_normalisation() {
    let d = parse_xml("t", b"<A b=\"1\" a=\"2\"/>").unwrap();
    assert_eq!(
        d.root().unwrap().attrs,
        vec![("b".into(), "1".into()), ("a".into(), "2".into())]
    );
    assert_eq!(write_xml(&d), b"<A b=\"1\" a=\"2\"/>");

    let d = parse_xml("t", b"<A x='q\"'/>").unwrap();
    assert_eq!(d.root().unwrap().attr("x").as_deref(), Some("q\""));
    assert_eq!(write_xml(&d), b"<A x=\"q&quot;\"/>");
}

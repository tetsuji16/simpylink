//! Lossless XML tree (attribute order, raw text, entity references, comments, CDATA).
use crate::error::SlxError;
use quick_xml::events::Event;
use quick_xml::Reader;

/// XML node.
#[derive(Debug, Clone, PartialEq)]
pub enum XmlNode {
    Element(XmlElement),
    Text(String),
    EntityRef(String),
    CData(String),
    Comment(String),
    Decl(String),
    PI(String),
    DocType(String),
}

/// Element with raw (escaped) attribute values.
#[derive(Debug, Clone, PartialEq)]
pub struct XmlElement {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<XmlNode>,
    pub self_closing: bool,
}

/// Document.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct XmlDocument {
    pub nodes: Vec<XmlNode>,
}

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// Parses bytes into a lossless tree.
pub fn parse_xml(part: &str, bytes: &[u8]) -> Result<XmlDocument, SlxError> {
    let err = |m: String| SlxError::Xml {
        part: part.into(),
        message: m,
    };
    let mut r = Reader::from_reader(bytes);
    let mut buf = vec![];
    let mut stack: Vec<XmlElement> = vec![];
    let mut top: Vec<XmlNode> = vec![];
    fn push(stack: &mut [XmlElement], top: &mut Vec<XmlNode>, n: XmlNode) {
        match stack.last_mut() {
            Some(e) => e.children.push(n),
            None => top.push(n),
        }
    }
    loop {
        let ev = r
            .read_event_into(&mut buf)
            .map_err(|e| err(e.to_string()))?;
        match ev {
            Event::Start(e) => {
                let attrs = e
                    .attributes()
                    .map(|a| {
                        a.map(|a| (s(a.key.as_ref()), s(&a.value)))
                            .map_err(|x| err(x.to_string()))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                stack.push(XmlElement {
                    name: s(e.name().as_ref()),
                    attrs,
                    children: vec![],
                    self_closing: false,
                });
            }
            Event::Empty(e) => {
                let attrs = e
                    .attributes()
                    .map(|a| {
                        a.map(|a| (s(a.key.as_ref()), s(&a.value)))
                            .map_err(|x| err(x.to_string()))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let el = XmlElement {
                    name: s(e.name().as_ref()),
                    attrs,
                    children: vec![],
                    self_closing: true,
                };
                push(&mut stack, &mut top, XmlNode::Element(el));
            }
            Event::End(e) => {
                let name = s(e.name().as_ref());
                let el = stack
                    .pop()
                    .ok_or_else(|| err("unexpected end tag".into()))?;
                if el.name != name {
                    return Err(err("mismatched end tag".into()));
                }
                push(&mut stack, &mut top, XmlNode::Element(el));
            }
            Event::Text(t) => push(&mut stack, &mut top, XmlNode::Text(s(t.as_ref()))),
            Event::GeneralRef(g) => push(&mut stack, &mut top, XmlNode::EntityRef(s(g.as_ref()))),
            Event::CData(c) => push(&mut stack, &mut top, XmlNode::CData(s(c.as_ref()))),
            Event::Comment(c) => push(&mut stack, &mut top, XmlNode::Comment(s(c.as_ref()))),
            Event::Decl(d) => push(&mut stack, &mut top, XmlNode::Decl(s(d.as_ref()))),
            Event::PI(p) => push(&mut stack, &mut top, XmlNode::PI(s(p.as_ref()))),
            Event::DocType(d) => push(&mut stack, &mut top, XmlNode::DocType(s(d.as_ref()))),
            Event::Eof => break,
        }
        buf.clear();
    }
    if !stack.is_empty() {
        return Err(err("unexpected end of file".into()));
    }
    Ok(XmlDocument { nodes: top })
}

fn write_node(n: &XmlNode, out: &mut String) {
    match n {
        XmlNode::Element(e) => {
            out.push('<');
            out.push_str(&e.name);
            for (k, v) in &e.attrs {
                out.push_str(&format!(" {k}=\"{}\"", v.replace('"', "&quot;")));
            }
            if e.self_closing && e.children.is_empty() {
                out.push_str("/>");
            } else {
                out.push('>');
                for c in &e.children {
                    write_node(c, out);
                }
                out.push_str(&format!("</{}>", e.name));
            }
        }
        XmlNode::Text(t) => out.push_str(t),
        XmlNode::EntityRef(r) => out.push_str(&format!("&{r};")),
        XmlNode::CData(c) => out.push_str(&format!("<![CDATA[{c}]]>")),
        XmlNode::Comment(c) => out.push_str(&format!("<!--{c}-->")),
        XmlNode::Decl(d) | XmlNode::PI(d) => out.push_str(&format!("<?{d}?>")),
        XmlNode::DocType(d) => out.push_str(&format!("<!DOCTYPE {d}>")),
    }
}

/// Serializes the tree.
pub fn write_xml(doc: &XmlDocument) -> Vec<u8> {
    let mut out = String::new();
    for n in &doc.nodes {
        write_node(n, &mut out);
    }
    out.into_bytes()
}

/// Escapes text content.
pub fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
/// Escapes an attribute value.
pub fn escape_attr(s: &str) -> String {
    escape_text(s).replace('"', "&quot;")
}
/// Unescapes entities.
pub fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        if let Some(j) = after.find(';') {
            out.push_str(&entity(&after[..j]).unwrap_or_else(|| format!("&{};", &after[..j])));
            rest = &after[j + 1..];
        } else {
            out.push('&');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}
fn entity(name: &str) -> Option<String> {
    Some(match name {
        "amp" => "&".into(),
        "lt" => "<".into(),
        "gt" => ">".into(),
        "quot" => "\"".into(),
        "apos" => "'".into(),
        _ if name.starts_with("#x") || name.starts_with("#X") => {
            char::from_u32(u32::from_str_radix(&name[2..], 16).ok()?)?.to_string()
        }
        _ if name.starts_with('#') => char::from_u32(name[1..].parse().ok()?)?.to_string(),
        _ => return None,
    })
}

impl XmlDocument {
    /// Root element.
    pub fn root(&self) -> Option<&XmlElement> {
        self.nodes.iter().find_map(|n| {
            if let XmlNode::Element(e) = n {
                Some(e)
            } else {
                None
            }
        })
    }
    /// Mutable root element.
    pub fn root_mut(&mut self) -> Option<&mut XmlElement> {
        self.nodes.iter_mut().find_map(|n| {
            if let XmlNode::Element(e) = n {
                Some(e)
            } else {
                None
            }
        })
    }
}

impl XmlElement {
    /// New element.
    pub fn new(name: &str) -> Self {
        XmlElement {
            name: name.into(),
            attrs: vec![],
            children: vec![],
            self_closing: false,
        }
    }
    /// Decoded attribute.
    pub fn attr(&self, name: &str) -> Option<String> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| unescape(v))
    }
    /// Sets an attribute (escaped), keeping its position.
    pub fn set_attr(&mut self, name: &str, value: &str) {
        let v = escape_attr(value);
        match self.attrs.iter_mut().find(|(k, _)| k == name) {
            Some(a) => a.1 = v,
            None => self.attrs.push((name.into(), v)),
        }
    }
    /// Child elements with a name.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a XmlElement> {
        self.children.iter().filter_map(move |n| match n {
            XmlNode::Element(e) if e.name == name => Some(e),
            _ => None,
        })
    }
    /// All child elements.
    pub fn elements(&self) -> impl Iterator<Item = &XmlElement> {
        self.children.iter().filter_map(|n| {
            if let XmlNode::Element(e) = n {
                Some(e)
            } else {
                None
            }
        })
    }
    /// Decoded text content (direct children).
    pub fn text(&self) -> String {
        let mut s = String::new();
        for c in &self.children {
            match c {
                XmlNode::Text(t) => s.push_str(&unescape(t)),
                XmlNode::EntityRef(r) => s.push_str(&entity(r).unwrap_or_default()),
                XmlNode::CData(c) => s.push_str(c),
                _ => {}
            }
        }
        s
    }
    /// Replaces children with one text node.
    pub fn set_text(&mut self, value: &str) {
        self.children = vec![XmlNode::Text(escape_text(value))];
        self.self_closing = false;
    }
    /// Adds a child element.
    pub fn push(&mut self, e: XmlElement) {
        self.children.push(XmlNode::Element(e));
    }
}

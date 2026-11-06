//! Open Packaging Conventions (ZIP) container, lossless.
use crate::error::SlxError;
use std::io::{Cursor, Read, Write};
use std::path::Path;

/// One part.
#[derive(Debug, Clone, PartialEq)]
pub struct OpcPart {
    pub name: String,
    pub data: Vec<u8>,
}

/// A package (parts in ZIP order).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OpcPackage {
    pub parts: Vec<OpcPart>,
}

impl OpcPackage {
    /// Part by name.
    pub fn part(&self, name: &str) -> Option<&OpcPart> {
        self.parts.iter().find(|p| p.name == name)
    }
    /// Mutable part by name.
    pub fn part_mut(&mut self, name: &str) -> Option<&mut OpcPart> {
        self.parts.iter_mut().find(|p| p.name == name)
    }
    /// Inserts or replaces a part (new parts are appended).
    pub fn set_part(&mut self, name: &str, data: Vec<u8>) {
        match self.part_mut(name) {
            Some(p) => p.data = data,
            None => self.parts.push(OpcPart {
                name: name.into(),
                data,
            }),
        }
    }
    /// Content type from [Content_Types].xml (Override first, then Default).
    pub fn content_type(&self, name: &str) -> Option<String> {
        let ct = self.part("[Content_Types].xml")?;
        let doc = crate::xml::parse_xml("[Content_Types].xml", &ct.data).ok()?;
        let root = doc.root()?;
        for o in root.children_named("Override") {
            if o.attr("PartName")
                .map(|p| p.trim_start_matches('/').eq_ignore_ascii_case(name))
                .unwrap_or(false)
            {
                return o.attr("ContentType");
            }
        }
        let ext = name.rsplit('.').next()?.to_ascii_lowercase();
        if !name.contains('.') {
            return None;
        }
        for d in root.children_named("Default") {
            if d.attr("Extension")
                .map(|e| e.eq_ignore_ascii_case(&ext))
                .unwrap_or(false)
            {
                return d.attr("ContentType");
            }
        }
        None
    }
}

/// Reads a package from bytes.
pub fn read_package(bytes: &[u8]) -> Result<OpcPackage, SlxError> {
    let mut ar =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| SlxError::Package(e.to_string()))?;
    let mut pkg = OpcPackage::default();
    for i in 0..ar.len() {
        let mut f = ar
            .by_index(i)
            .map_err(|e| SlxError::Package(e.to_string()))?;
        let name = f.name().to_string();
        if name.ends_with('/') {
            continue;
        }
        let mut data = vec![];
        f.read_to_end(&mut data)?;
        pkg.parts.push(OpcPart { name, data });
    }
    if pkg.part("[Content_Types].xml").is_none() {
        return Err(SlxError::Package("missing [Content_Types].xml".into()));
    }
    Ok(pkg)
}

/// Writes a package to bytes.
pub fn write_package(pkg: &OpcPackage) -> Result<Vec<u8>, SlxError> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for p in &pkg.parts {
            w.start_file(p.name.as_str(), opts)
                .map_err(|e| SlxError::Package(e.to_string()))?;
            w.write_all(&p.data)?;
        }
        w.finish().map_err(|e| SlxError::Package(e.to_string()))?;
    }
    Ok(buf.into_inner())
}

/// Reads a package file.
pub fn read_package_file(path: &Path) -> Result<OpcPackage, SlxError> {
    read_package(&std::fs::read(path)?)
}

/// Writes a package file; refuses to overwrite.
pub fn write_package_file(pkg: &OpcPackage, path: &Path) -> Result<(), SlxError> {
    if path.exists() {
        return Err(SlxError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "{} exists; simpylink never overwrites SLX files",
                path.display()
            ),
        )));
    }
    std::fs::write(path, write_package(pkg)?)?;
    Ok(())
}

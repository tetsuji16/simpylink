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

/// Size limits applied while reading a package (protection against ZIP bombs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcLimits {
    /// Largest number of entries in the archive.
    pub max_parts: usize,
    /// Largest uncompressed size of one part, in bytes.
    pub max_part_bytes: u64,
    /// Largest uncompressed size of all parts together, in bytes.
    pub max_total_bytes: u64,
}

impl Default for OpcLimits {
    /// 10 000 parts, 256 MiB per part, 1 GiB in total.
    fn default() -> Self {
        OpcLimits {
            max_parts: 10_000,
            max_part_bytes: 256 * 1024 * 1024,
            max_total_bytes: 1024 * 1024 * 1024,
        }
    }
}

/// Reads a package from bytes with the default [`OpcLimits`].
pub fn read_package(bytes: &[u8]) -> Result<OpcPackage, SlxError> {
    read_package_with_limits(bytes, &OpcLimits::default())
}

/// Reads a package from bytes; fails with [`SlxError::Package`] when a limit is exceeded.
///
/// The limits are checked against the bytes actually decompressed, not against the
/// sizes declared in the ZIP headers (which an attacker controls).
pub fn read_package_with_limits(bytes: &[u8], limits: &OpcLimits) -> Result<OpcPackage, SlxError> {
    let mut ar =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| SlxError::Package(e.to_string()))?;
    if ar.len() > limits.max_parts {
        return Err(SlxError::Package(format!(
            "too many parts ({}, limit {})",
            ar.len(),
            limits.max_parts
        )));
    }
    let mut total: u64 = 0;
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
        // Never read more than this part may add to the package: a small total limit
        // must not be preceded by a full `max_part_bytes` allocation.
        let remaining_total = limits.max_total_bytes.saturating_sub(total);
        let cap = limits.max_part_bytes.min(remaining_total);
        // Read one byte more than allowed so that "exactly at the limit" still passes.
        let read = (&mut f)
            .take(cap.saturating_add(1))
            .read_to_end(&mut data)? as u64;
        if read > cap {
            return Err(SlxError::Package(if read > limits.max_part_bytes {
                format!(
                    "part '{name}' is larger than {} bytes",
                    limits.max_part_bytes
                )
            } else {
                format!(
                    "package is larger than {} bytes when unpacked",
                    limits.max_total_bytes
                )
            }));
        }
        total = total.saturating_add(read);
        if total > limits.max_total_bytes {
            return Err(SlxError::Package(format!(
                "package is larger than {} bytes when unpacked",
                limits.max_total_bytes
            )));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn package(parts: &[(&str, usize)]) -> Vec<u8> {
        let mut pkg = OpcPackage::default();
        pkg.set_part("[Content_Types].xml", b"<Types/>".to_vec());
        for (name, len) in parts {
            pkg.set_part(name, vec![b'a'; *len]);
        }
        write_package(&pkg).unwrap()
    }

    fn message(r: Result<OpcPackage, SlxError>) -> String {
        match r {
            Err(SlxError::Package(m)) => m,
            other => panic!("expected a package error, got {other:?}"),
        }
    }

    #[test]
    fn limits_are_enforced_on_unpacked_sizes() {
        // 8 bytes of content types + 100 + 100.
        let bytes = package(&[("a.xml", 100), ("b.xml", 100)]);
        let lim = |max_parts, max_part_bytes, max_total_bytes| OpcLimits {
            max_parts,
            max_part_bytes,
            max_total_bytes,
        };
        assert_eq!(
            read_package_with_limits(&bytes, &lim(3, 100, 208))
                .unwrap()
                .parts
                .len(),
            3
        );
        assert_eq!(
            message(read_package_with_limits(&bytes, &lim(2, 100, 208))),
            "too many parts (3, limit 2)"
        );
        assert_eq!(
            message(read_package_with_limits(&bytes, &lim(3, 99, 208))),
            "part 'a.xml' is larger than 99 bytes"
        );
        assert_eq!(
            message(read_package_with_limits(&bytes, &lim(3, 100, 207))),
            "package is larger than 207 bytes when unpacked"
        );
        assert!(read_package(&bytes).is_ok());
    }

    #[test]
    fn a_small_total_limit_wins_over_a_large_part_limit() {
        // 8 bytes of content types + 100 + 100.
        let bytes = package(&[("a.xml", 100), ("b.xml", 100)]);
        let lim = |max_parts, max_part_bytes, max_total_bytes| OpcLimits {
            max_parts,
            max_part_bytes,
            max_total_bytes,
        };
        // The part limit would allow a.xml, but only 42 bytes are left in the package.
        assert_eq!(
            message(read_package_with_limits(&bytes, &lim(3, 1000, 50))),
            "package is larger than 50 bytes when unpacked"
        );
        // Earlier parts may exhaust the total so that the next part is rejected by the
        // total even though it is smaller than the part limit.
        assert_eq!(
            message(read_package_with_limits(&bytes, &lim(3, 100, 110))),
            "package is larger than 110 bytes when unpacked"
        );
    }

    #[test]
    fn exact_limits_are_accepted() {
        let bytes = package(&[("a.xml", 100), ("b.xml", 100)]);
        let lim = |max_parts, max_part_bytes, max_total_bytes| OpcLimits {
            max_parts,
            max_part_bytes,
            max_total_bytes,
        };
        // 208 bytes in total, one part of exactly 100 bytes.
        assert!(read_package_with_limits(&bytes, &lim(3, 100, 208)).is_ok());
        // One byte less in total is not enough for the last part.
        assert_eq!(
            message(read_package_with_limits(&bytes, &lim(3, 100, 207))),
            "package is larger than 207 bytes when unpacked"
        );
        // A part limit exactly at the part size is still accepted.
        assert!(read_package_with_limits(&bytes, &lim(3, 100, 1 << 20)).is_ok());
        assert_eq!(
            message(read_package_with_limits(&bytes, &lim(3, 99, 1 << 20))),
            "part 'a.xml' is larger than 99 bytes"
        );
    }
}

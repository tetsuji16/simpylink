//! Fixed runtime appended to every generated file.

/// Generator format 1 runtime (starts with two empty lines).
pub const RUNTIME_FORMAT1: &str = include_str!("runtime_format1.py");

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_shape() {
        assert!(RUNTIME_FORMAT1.starts_with(
            "\n\n# ===========================================================================\n# Fixed runtime (identical in every generated file; generator format 1)\n"
        ));
        assert!(RUNTIME_FORMAT1.ends_with("if __name__ == \"__main__\":\n    sys.exit(main())\n"));
        assert!(!RUNTIME_FORMAT1.contains('\r') && !RUNTIME_FORMAT1.contains('\t'));
        assert!(RUNTIME_FORMAT1.lines().all(|l| !l.ends_with(' ')));
        assert_eq!(RUNTIME_FORMAT1.lines().count(), 132);
    }
}

//! Workspace smoke tests: every crate links and the fixtures directory exists.

#[test]
fn all_crates_link() {
    assert_eq!(sim_core::crate_name(), "sim_core");
    assert_eq!(sim_ir::crate_name(), "sim_ir");
    assert_eq!(sim_codegen_py::crate_name(), "sim_codegen_py");
    assert_eq!(sim_runner::crate_name(), "sim_runner");
    assert_eq!(sim_slx::crate_name(), "sim_slx");
}

#[test]
fn fixtures_dir_exists() {
    assert!(sim_integration::fixtures_dir().join("models").is_dir());
}

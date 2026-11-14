//! Result-file loading tests (Issue #65, DES-04 §5).
use sim_runner::{load_results, ResultsError, SimResults};

const META_OK: &str = r#"{"format":"simpylink-results","format_version":1,"model":"m","status":"ok","error":null,"num_samples":5,"channels":[{"name":"Scope/1","block_sid":3,"port":1}]}"#;

fn load(meta: &str, csv: &str) -> Result<SimResults, ResultsError> {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("results.meta.json"), meta).unwrap();
    std::fs::write(dir.path().join("results.csv"), csv).unwrap();
    load_results(dir.path())
}

#[test]
fn loads_the_contract_example() {
    let results = load(
        META_OK,
        "time,Scope/1\n0.0,0.0\n0.5,0.0\n1.0,6.0\n1.5,6.0\n2.0,6.0\n",
    )
    .unwrap();
    assert_eq!(results.model, "m");
    assert_eq!(results.time, vec![0.0, 0.5, 1.0, 1.5, 2.0]);
    assert_eq!(results.channels.len(), 1);
    let channel = &results.channels[0];
    assert_eq!(channel.name, "Scope/1");
    assert_eq!((channel.block_sid, channel.port), (3, 1));
    assert_eq!(channel.values, vec![0.0, 0.0, 6.0, 6.0, 6.0]);
    assert_eq!(results.channel("Scope/1"), Some(channel));
}

#[test]
fn parses_nan_inf_and_crlf() {
    let meta = META_OK.replace("\"num_samples\":5", "\"num_samples\":3");
    let results = load(&meta, "time,Scope/1\r\n0.0,nan\r\n0.5,inf\r\n1.0,-inf\r\n").unwrap();
    assert_eq!(results.time, vec![0.0, 0.5, 1.0]);
    let values = &results.channels[0].values;
    assert!(values[0].is_nan());
    assert_eq!(values[1], f64::INFINITY);
    assert_eq!(values[2], f64::NEG_INFINITY);
}

#[test]
fn loads_without_channels() {
    let meta = r#"{"format":"simpylink-results","format_version":1,"model":"m","status":"ok","error":null,"num_samples":2,"channels":[]}"#;
    let results = load(meta, "time\n0.0\n1.0\n").unwrap();
    assert_eq!(results.time, vec![0.0, 1.0]);
    assert!(results.channels.is_empty());
}

#[test]
fn missing_meta_file_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        load_results(dir.path()),
        Err(ResultsError::Missing(name)) if name == "results.meta.json"
    ));
}

#[test]
fn missing_csv_file_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("results.meta.json"), META_OK).unwrap();
    assert!(matches!(
        load_results(dir.path()),
        Err(ResultsError::Missing(name)) if name == "results.csv"
    ));
}

#[test]
fn unreadable_meta_file_is_an_io_error_not_missing() {
    // The file exists but cannot be decoded: that is not "the run wrote nothing" (#207).
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("results.meta.json"), [0xff, 0xfe, 0xfd]).unwrap();
    match load_results(dir.path()) {
        Err(ResultsError::Io { path, .. }) => {
            assert!(path.ends_with("results.meta.json"), "{path}");
        }
        other => panic!("expected ResultsError::Io, got {other:?}"),
    }
}

#[test]
fn reported_simulation_error_is_returned() {
    let meta = META_OK.replace(
        "\"ok\",\"error\":null",
        "\"error\",\"error\":\"SimulationError: x\"",
    );
    assert!(
        matches!(load(&meta, ""), Err(ResultsError::Reported(message)) if message == "SimulationError: x")
    );
}

#[test]
fn header_must_start_with_time() {
    assert!(matches!(
        load(META_OK, "t,Scope/1\n0.0,0.0\n"),
        Err(ResultsError::Csv { line: 1, ref message }) if message == "first column must be 'time'"
    ));
}

#[test]
fn header_must_match_channels() {
    assert!(matches!(
        load(META_OK, "time,Other\n0.0,0.0\n"),
        Err(ResultsError::Csv { line: 1, ref message }) if message == "header does not match channels"
    ));
}

#[test]
fn row_width_must_match_header() {
    assert!(matches!(
        load(META_OK, "time,Scope/1\n0.0,0.0\n1.0\n"),
        Err(ResultsError::Csv { line: 3, ref message }) if message == "expected 2 columns, found 1"
    ));
}

#[test]
fn invalid_numbers_are_reported_with_the_line() {
    assert!(matches!(
        load(META_OK, "time,Scope/1\n0.0,abc\n"),
        Err(ResultsError::Csv { line: 2, ref message }) if message == "invalid number 'abc'"
    ));
}

#[test]
fn sample_count_must_match_meta() {
    assert!(matches!(
        load(META_OK, "time,Scope/1\n0.0,1.0\n"),
        Err(ResultsError::Meta(_))
    ));
}

#[test]
fn results_format_version_must_be_one() {
    let meta = META_OK.replace("\"format_version\":1", "\"format_version\":2");
    assert!(
        matches!(load(&meta, ""), Err(ResultsError::Meta(message)) if message == "unsupported results format")
    );
}

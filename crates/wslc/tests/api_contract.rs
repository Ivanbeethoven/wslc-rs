use std::path::PathBuf;
use std::time::Duration;

use wslc::{
    DeleteContainerOptions, Error, ImagePullOptions, ProcessOptions, Service, Session, Signal,
    WslcErrorKind,
};

#[test]
fn session_builder_rejects_empty_session_name_before_loading_sdk() {
    let err = Session::builder("", PathBuf::from(r"C:\WslcData\test"))
        .start()
        .unwrap_err();

    assert!(matches!(err, Error::InvalidInput(message) if message.contains("session name")));
}

#[test]
fn session_builder_rejects_zero_cpu_count() {
    let err = Session::builder("test", PathBuf::from(r"C:\WslcData\test"))
        .cpu_count(0)
        .start()
        .unwrap_err();

    assert!(matches!(err, Error::InvalidInput(message) if message.contains("cpu_count")));
}

#[test]
fn process_options_reject_empty_command_when_used_as_init_process() {
    let err = ProcessOptions::new(Vec::<String>::new())
        .validate()
        .unwrap_err();

    assert!(matches!(err, Error::InvalidInput(message) if message.contains("command")));
}

#[test]
fn image_pull_options_reject_empty_uri_before_loading_sdk() {
    let err = ImagePullOptions::new("").validate().unwrap_err();

    assert!(matches!(err, Error::InvalidInput(message) if message.contains("image uri")));
}

#[test]
fn delete_container_options_default_is_not_forceful() {
    assert!(!DeleteContainerOptions::default().force);
}

#[test]
fn timeout_validation_rejects_values_that_do_not_fit_u32_milliseconds() {
    let err = Session::builder("test", PathBuf::from(r"C:\WslcData\test"))
        .timeout(Duration::from_millis(u64::from(u32::MAX) + 1))
        .start()
        .unwrap_err();

    assert!(matches!(err, Error::InvalidInput(message) if message.contains("timeout")));
}

#[test]
fn hresult_errors_expose_original_code_and_wslc_kind() {
    let err = Error::from_hresult(0x8004_0603_u32 as i32, "missing container");

    assert_eq!(err.hresult(), Some(0x8004_0603_u32 as i32));
    assert_eq!(err.wslc_kind(), Some(WslcErrorKind::ContainerNotFound));
    assert!(err.to_string().contains("missing container"));
}

#[test]
fn service_reports_missing_sdk_as_sdk_not_found_or_components() {
    let result = Service::version();

    if let Err(error) = result {
        assert!(
            matches!(error, Error::SdkNotFound(_) | Error::MissingComponents(_)),
            "unexpected error: {error:?}"
        );
    }
}

#[test]
fn signal_values_match_linux_signal_numbers() {
    assert_eq!(Signal::Sigterm.as_raw(), 15);
    assert_eq!(Signal::Sigkill.as_raw(), 9);
}

#[test]
fn safe_crate_sources_do_not_contain_unsafe_code() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let mut violations = Vec::new();

    for entry in std::fs::read_dir(&src_dir).expect("read src dir") {
        let path = entry.expect("read src entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }

        let source = std::fs::read_to_string(&path).expect("read source file");
        for (line_number, line) in source.lines().enumerate() {
            if line.contains("unsafe") || line.contains("extern \"system\"") {
                violations.push(format!(
                    "{}:{}: {}",
                    path.strip_prefix(&manifest_dir).unwrap().display(),
                    line_number + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "the safe wslc crate must not contain unsafe code:\n{}",
        violations.join("\n")
    );
}

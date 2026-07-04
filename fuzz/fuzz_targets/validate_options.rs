#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;
use std::time::Duration;
use wslc::{
    ContainerOptions, Error, ImagePullOptions, ProcessOptions, Session, WslcErrorKind,
};

#[derive(Debug, Arbitrary)]
struct Input {
    session_name: String,
    storage_path: String,
    image: String,
    image_uri: String,
    registry_auth: Option<String>,
    cmdline: Vec<String>,
    working_dir: Option<String>,
    env: Vec<(String, String)>,
    timeout_ms: u64,
    hresult: i32,
}

fuzz_target!(|input: Input| {
    let _ = ContainerOptions::new(input.image).validate();

    let mut pull = ImagePullOptions::new(input.image_uri);
    if let Some(registry_auth) = input.registry_auth {
        pull = pull.registry_auth(registry_auth);
    }
    let _ = pull.validate();

    let mut process = ProcessOptions::new(input.cmdline);
    if let Some(working_dir) = input.working_dir {
        process = process.working_dir(working_dir);
    }
    for (key, value) in input.env {
        process = process.env(key, value);
    }
    let _ = process.validate();

    let _ = Session::builder(input.session_name, PathBuf::from(input.storage_path))
        .timeout(Duration::from_millis(input.timeout_ms))
        .start();

    let err = Error::from_hresult(input.hresult, "fuzz");
    match err.wslc_kind() {
        Some(WslcErrorKind::Unknown(code)) => assert_eq!(code, input.hresult),
        Some(_) | None => {}
    }
});

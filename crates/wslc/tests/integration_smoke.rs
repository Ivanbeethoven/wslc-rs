#![cfg(feature = "integration")]

use std::path::PathBuf;

use wslc::{ImagePullOptions, ProcessOptions, Service, Session};

#[test]
fn service_is_available_when_integration_feature_is_enabled() {
    Service::ensure_available().expect("WSLC service should be available for integration tests");
}

#[test]
fn session_lifecycle_smoke_test() {
    Service::ensure_available().expect("WSLC service should be available for integration tests");

    let session = Session::builder(
        "wslc-rs-session-lifecycle",
        PathBuf::from(r"C:\WslcData\wslc-rs-session-lifecycle"),
    )
    .cpu_count(2)
    .memory_mb(2048)
    .start()
    .expect("session should start");

    session.terminate().expect("session should terminate");
}

#[test]
#[ignore = "requires Docker Hub access and pulls docker.io/library/alpine:latest"]
fn alpine_echo_smoke_test() {
    Service::ensure_available().expect("WSLC service should be available for integration tests");

    let session = Session::builder(
        "wslc-rs-integration",
        PathBuf::from(r"C:\WslcData\wslc-rs-integration"),
    )
    .cpu_count(2)
    .memory_mb(2048)
    .start()
    .expect("session should start");

    session
        .pull_image(ImagePullOptions::new("docker.io/library/alpine:latest"))
        .run()
        .expect("alpine pull should succeed");

    let output = session
        .container(wslc::ContainerOptions::new("alpine:latest"))
        .name("wslc-rs-integration-echo")
        .init_process(ProcessOptions::new(["/bin/echo", "hello from wslc-rs"]).capture_stdout())
        .auto_remove(true)
        .create()
        .expect("container should be created")
        .start_and_wait()
        .expect("container should run");

    assert_eq!(output.status, 0);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "hello from wslc-rs"
    );

    session.terminate().expect("session should terminate");
}

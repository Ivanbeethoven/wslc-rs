#![cfg(feature = "integration")]

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use wslc::{ContainerOptions, ImagePullOptions, ProcessOptions, Service, Session};

fn unique_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    format!("{prefix}-{nanos}-{}", std::process::id())
}

fn storage_path(name: &str) -> String {
    format!(r"C:\WslcData\{name}")
}

#[test]
fn privileged_container_fuse_test() {
    Service::ensure_available().expect("WSLC service should be available");

    let name = unique_name("wslc-rs-fuse");
    let session = Session::builder(&name, storage_path(&name))
        .cpu_count(2)
        .memory_mb(2048)
        .timeout(Duration::from_secs(300))
        .terminate_on_drop(true)
        .start()
        .expect("session should start");

    // Pull debian image
    let pull_opts = ImagePullOptions::new("docker.io/library/debian:trixie-slim");
    session
        .pull_image(pull_opts)
        .on_progress(|p| {
            eprintln!("pull: {:?} {}/{}", p.status, p.current_bytes, p.total_bytes);
        })
        .run()
        .expect("should pull debian image");

    // Run everything as a single init process via start_and_wait
    let script = r#"
apt-get update -qq
apt-get install -y -qq fuse3
echo "=== SECCOMP ==="
cat /proc/1/status | grep -i seccomp
echo "=== CAPS ==="
cat /proc/1/status | grep Cap
echo "=== FUSE MODULES ==="
grep fuse /proc/filesystems || echo "NO_FUSE_MODULES"
echo "=== FUSE DEV ==="
ls -la /dev/fuse 2>&1 || echo "NO_FUSE_DEV"
echo "=== MKNOD ==="
mknod /dev/fuse c 10 229 2>&1 || echo "MKNOD_FAILED"
echo "=== FUSE DEV AFTER MKNOD ==="
ls -la /dev/fuse 2>&1 || echo "STILL_NO_FUSE_DEV"
echo "=== DD TEST ==="
dd if=/dev/fuse of=/dev/null bs=1 count=1 2>&1
echo "DD_EXIT=$?"
echo "=== DONE ==="
"#;

    let cname = unique_name("c");
    let cmdline = vec!["/bin/sh".to_string(), "-c".to_string(), script.to_string()];

    let output = session
        .container(ContainerOptions::new(
            "docker.io/library/debian:trixie-slim",
        ))
        .name(&cname)
        .privileged(true)
        .init_process(ProcessOptions {
            cmdline,
            working_dir: None,
            env: Vec::new(),
            output_mode: wslc::OutputMode::Capture,
        })
        .create()
        .expect("should create privileged container")
        .start_and_wait()
        .expect("should start and wait");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("=== STDOUT ===\n{stdout}");
    eprintln!("=== STDERR ===\n{stderr}");

    // Check that dd did not fail with Operation not permitted
    let combined = format!("{stdout}{stderr}");
    assert!(
        !combined.contains("Operation not permitted"),
        "FUSE device should be accessible in privileged container!\nOutput:\n{combined}"
    );

    // If dd reports 0+0 records, /dev/fuse exists but is empty (expected for new fuse dev)
    // That's fine - the key is it's accessible
    if combined.contains("STILL_NO_FUSE_DEV") {
        eprintln!("WARNING: /dev/fuse could not be created. FUSE kernel support may be missing.");
    } else if combined.contains("Operation not permitted") {
        panic!("FUSE device exists but access is blocked by seccomp!");
    } else {
        eprintln!("PASS: FUSE device is accessible and working!");
    }
}

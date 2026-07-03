use std::path::PathBuf;
use std::time::Duration;

use wslc::{
    ContainerOptions, DeleteContainerOptions, ImagePullOptions, ProcessOptions, Service, Session,
    Signal,
};

fn main() -> wslc::Result<()> {
    Service::ensure_available()?;

    let session = Session::builder("hello-wslc-rs", PathBuf::from(r"C:\WslcData\hello-wslc-rs"))
        .cpu_count(2)
        .memory_mb(2048)
        .start()?;

    session
        .pull_image(ImagePullOptions::new("docker.io/library/alpine:latest"))
        .on_progress(|p| {
            println!("pull: {:?} {}/{}", p.status, p.current_bytes, p.total_bytes);
        })
        .run()?;

    let process = ProcessOptions::new(["/bin/echo", "hello from wslc-rs"])
        .capture_stdout()
        .capture_stderr();

    let container = session
        .container(ContainerOptions::new("alpine:latest"))
        .name("hello-wslc-rs")
        .init_process(process)
        .auto_remove(true)
        .create()?;

    let output = container.start_and_wait()?;
    println!("status: {}", output.status);
    println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));

    container
        .stop(Signal::Sigterm, Duration::from_secs(10))
        .ok();
    container.delete(DeleteContainerOptions::default()).ok();
    session.terminate()?;

    Ok(())
}

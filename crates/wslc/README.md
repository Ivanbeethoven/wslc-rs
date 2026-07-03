# wslc

Safe Rust wrapper for the preview Microsoft WSL Containers SDK.

The crate loads `wslcsdk.dll` at runtime, validates common inputs before calling
the SDK, maps failing HRESULT values into `wslc::Error`, and releases SDK
handles with RAII. It does not redistribute Microsoft SDK files.

```rust
use wslc::{ImagePullOptions, Service, Session};

fn main() -> wslc::Result<()> {
    Service::ensure_available()?;
    let session = Session::builder("my-app", r"C:\WslcData\my-app").start()?;
    session.pull_image(ImagePullOptions::new("alpine:latest")).run()?;
    session.terminate()?;
    Ok(())
}
```

Run unit tests with `cargo test -p wslc`. Real WSLC smoke tests require
`--features integration`.

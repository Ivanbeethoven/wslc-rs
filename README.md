# wslc-rs

Rust bindings for the preview Microsoft WSL Containers SDK.

`wslc-rs` is for Windows host applications that want to create sessions, pull
images, create containers, run init processes, and capture basic output through
the WSLC SDK (`wslcsdk.dll`). It is not a Docker API implementation and it is not
a traditional WSL distro-management crate.

> WSLC is a preview Microsoft API. Function signatures and behavior can change
> before the platform reaches GA. This crate starts at `0.1.x` for that reason.

## Crates

- `wslc-sys`: raw ABI-compatible Rust types plus the runtime unsafe boundary
  for loading `wslcsdk.dll`, calling SDK exports, handling callbacks, and
  releasing SDK-allocated memory. It does not redistribute Microsoft binaries.
- `wslc`: safe blocking wrapper with builders, input validation, HRESULT error
  mapping, RAII handle release, and MVP session/container APIs. The safe crate
  keeps SDK `unsafe` code out of its source tree.

## Quick Start

```rust
use std::path::PathBuf;

use wslc::{ImagePullOptions, ProcessOptions, Service, Session};

fn main() -> wslc::Result<()> {
    Service::ensure_available()?;

    let session = Session::builder("hello-wslc-rs", PathBuf::from(r"C:\WslcData\hello-wslc-rs"))
        .cpu_count(2)
        .memory_mb(2048)
        .start()?;

    session
        .pull_image(ImagePullOptions::new("docker.io/library/alpine:latest"))
        .run()?;

    let output = session
        .container(wslc::ContainerOptions::new("alpine:latest"))
        .init_process(ProcessOptions::new(["/bin/echo", "hello from wslc-rs"]).capture_stdout())
        .auto_remove(true)
        .create()?
        .start_and_wait()?;

    println!("{}", String::from_utf8_lossy(&output.stdout));
    session.terminate()?;
    Ok(())
}
```

## SDK Loading

The safe crate loads `wslcsdk.dll` at runtime. This means normal builds,
documentation, and unit tests can run on machines that do not have the preview
SDK installed. Calls into WSLC return `Error::SdkNotFound` when the DLL or a
required export is unavailable.

At runtime, make sure the native SDK directory is on `PATH`, for example:

```powershell
$env:WSLC_SDK_DIR="C:\Users\<you>\.nuget\packages\microsoft.wsl.containers\<version>"
$env:PATH="$env:WSLC_SDK_DIR\runtimes\win-x64\native;$env:PATH"
```

## Testing

Ordinary tests do not require WSLC:

```powershell
cargo test --workspace
```

Real container smoke tests are gated:

```powershell
wsl --update
wslc version
cargo test -p wslc --features integration --test integration_smoke
```

The default integration suite checks service availability and session lifecycle.
The Alpine echo test is ignored by default because it pulls a registry image; run
it explicitly with `-- --ignored` when registry access is available. Override the
image with `WSLC_ALPINE_IMAGE` when needed:

```powershell
$env:WSLC_ALPINE_IMAGE="um1aojh4p148gc.xuanyuan.run/library/alpine:latest"
cargo test -p wslc --features integration --test integration_smoke -- --ignored --nocapture
```

The repository also includes a manual GitHub Actions workflow,
`WSLC network E2E`, that installs the preview SDK from NuGet on a Windows runner
and runs the ignored Alpine pull/container test. Trigger it from the Actions tab
with runner labels such as `["windows-latest"]` or
`["self-hosted","Windows","X64"]`.

Fuzz targets live under `fuzz/` and focus on pure Rust validation paths that do
not call the WSLC runtime:

```powershell
cargo install cargo-fuzz --locked
cargo fuzz run validate_options
```

On Windows MSVC, libFuzzer also needs the LLVM/Clang ASan runtime libraries on
the linker search path. If `cargo fuzz run` reports a missing
`clang_rt.asan_dynamic_runtime_thunk-x86_64.lib`, install the LLVM/Clang tools
for your Visual Studio toolchain or add the directory containing that library to
`LIB`. At runtime, add the matching ASan DLL directory to `PATH`, for example:

```powershell
$msvc = "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.41.34120"
$env:PATH="$msvc\bin\Hostx64\x64;$env:PATH"
```

## Status

Implemented in `0.1.0`:

- raw ABI types, runtime FFI boundary, and layout tests;
- service availability/version checks;
- session builder and lifecycle;
- image pull with progress callback;
- container builder, start, attached start, stop, delete, inspect, state;
- process options, basic callbacks, exit waiting, and output capture;
- image list/delete/tag and VHD volume create/delete;
- explicit COM initialization helper.

Deferred:

- async runtime integration;
- stdin streaming;
- richer registry authentication helpers;
- crash dump subscriptions;
- stable thread-safety guarantees for handles;
- dynamic function compatibility shims across future preview SDK versions.

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT license

Microsoft SDK binaries and headers are not redistributed in this repository or
in the crates.

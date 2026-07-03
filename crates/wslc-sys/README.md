# wslc-sys

Raw Rust ABI definitions for the preview Microsoft WSL Container SDK
(`wslcsdk.h` / `wslcsdk.dll`).

Policy:

- ABI types, constants, callback signatures, and function pointer signatures
  only.
- No safe wrapper behavior.
- No SDK linking and no SDK binary redistribution.
- Safe users should depend on `wslc`.

This crate includes layout tests for the opaque settings blobs and image info
struct.

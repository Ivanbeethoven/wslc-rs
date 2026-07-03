# FFI 与安全边界

## 1. 基本原则

`wslc-sys` 是 unsafe 边界，`wslc` 负责把 unsafe 变成安全 API。

必须处理的问题：

- raw handle 的所有权；
- `HRESULT` 错误转换；
- `PWSTR*` / `PSTR*` 返回内存释放；
- C 字符串生命周期；
- callback 的 Rust 闭包生命周期；
- 进程 IO buffer 的短生命周期；
- COM 初始化；
- Windows HANDLE 等待；
- 多线程 Send / Sync 边界。

## 2. HRESULT 转换

底层函数通常返回 `HRESULT`。建议统一封装：

```rust
fn check_hresult(hr: HRESULT, error_message: *mut u16) -> Result<()> {
    if hr >= 0 {
        return Ok(());
    }

    let message = unsafe { take_cotaskmem_wide_string(error_message) };
    Err(Error::HResult {
        code: hr,
        message,
    })
}
```

注意：

- `S_OK` 以及非失败 HRESULT 视为成功；
- `error_message` 可能为空；
- 错误码要保留原始 `HRESULT`，便于用户排查；
- WSLC 专有错误码可映射成更友好的 enum，但不要丢失原始 code。

## 3. CoTaskMemAlloc 内存释放

官方 C API 中，部分字符串或数组由 SDK 分配，调用方拥有所有权，需要用 `CoTaskMemFree` 释放。

建议封装：

```rust
struct CoTaskMem<T>(*mut T);

impl<T> Drop for CoTaskMem<T> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                windows_sys::Win32::System::Com::CoTaskMemFree(Some(self.0.cast()));
            }
        }
    }
}
```

然后提供：

```rust
unsafe fn take_cotaskmem_utf8(ptr: *mut i8) -> Result<String>;
unsafe fn take_cotaskmem_utf16(ptr: *mut u16) -> Result<String>;
unsafe fn take_cotaskmem_slice<T: Copy>(ptr: *mut T, len: usize) -> Vec<T>;
```

## 4. 字符串转换

### PCWSTR

用于 session name、storage path、Windows path。

```rust
fn to_wide_null(s: &OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(std::iter::once(0)).collect()
}
```

### PCSTR

用于 image name、container path、argv、env。

```rust
let c = CString::new(value).map_err(Error::nul)?;
```

必须拒绝内部 `\0`：

```rust
Error::InvalidInput("string contains interior NUL")
```

## 5. COM 初始化

C 示例里会调用 COM 初始化。Rust 库建议提供两种策略：

### 策略 A：用户显式初始化

```rust
let _com = wslc::com::initialize_mta()?;
```

优点：语义清楚。  
缺点：使用门槛高。

### 策略 B：库内部惰性初始化

```rust
Service::ensure_available()?;
```

第一次调用时在当前线程尝试 `CoInitializeEx(COINIT_MULTITHREADED)`。

问题：

- 如果调用线程已经用 STA 初始化 COM，MTA 初始化可能失败；
- Rust 库不能随意改变调用方线程模型。

### 推荐

MVP 可以采用：

```rust
pub fn initialize_mta() -> Result<ComGuard>;
```

同时在高级 API 中尽量自动处理；如果当前线程 COM apartment 不兼容，返回清晰错误：

```rust
Error::ComInitialization {
    code,
    message: "WSLC requires COM initialization on this thread; use wslc::initialize_mta() or run on a dedicated worker thread",
}
```

长期可提供 dedicated worker thread：

```rust
let runtime = WslcRuntime::new()?;
runtime.block_on(|service| { ... });
```

## 6. Callback 安全

### SDK 回调特点

进程 stdout/stderr 回调收到的 buffer：

- 由 SDK 拥有；
- 只在 callback 调用期间有效；
- 不能保存原始指针；
- 必须立即复制；
- callback 必须快速返回。

因此 Rust 回调必须这样做：

```rust
extern "system" fn stdout_trampoline(
    handle: WslcProcessIOHandle,
    data: *const u8,
    len: u32,
    context: *mut c_void,
) {
    let slice = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let owned = slice.to_vec();

    let tx = unsafe { &*(context as *const crossbeam_channel::Sender<OutputEvent>) };
    let _ = tx.try_send(OutputEvent::Stdout(owned));
}
```

不要在 trampoline 里：

- 做阻塞 IO；
- 加大锁；
- 调用用户闭包做复杂逻辑；
- 保存 `data` 指针；
- panic 穿过 FFI 边界。

所有 callback 都要包 `catch_unwind` 或保证内部无 panic。

## 7. `Send` / `Sync`

不要默认给 raw handle 标记 `Send + Sync`。

建议策略：

- MVP：`Session` / `Container` / `Process` 先不实现 `Send` / `Sync`，或仅在明确确认 SDK 线程安全后实现；
- 如果需要异步 API，使用单独 worker thread 拥有所有 handle；
- 用户线程通过 channel 给 worker 发命令。

## 8. Drop 中的错误处理

`Drop` 不能返回错误。释放句柄失败时只能：

- debug log；
- tracing warn；
- 忽略。

不要在 Drop 中 panic。

```rust
impl Drop for SessionInner {
    fn drop(&mut self) {
        unsafe {
            let _ = wslc_sys::WslcReleaseSession(self.raw.as_ptr());
        }
    }
}
```

## 9. 不安全 API 隔离

`wslc` crate 中尽量只在少数文件出现 unsafe：

```text
src/raw.rs              # handle wrapper
src/string.rs           # CString / wide string / CoTaskMem
src/callback.rs         # trampoline
src/wait.rs             # Windows HANDLE wait
```

并对每个 unsafe block 写明 safety 注释：

```rust
// SAFETY:
// - settings points to a properly initialized WslcSessionSettings.
// - raw_session is valid for writes.
// - error_message is either null or receives CoTaskMem-allocated memory.
unsafe {
    hr = wslc_sys::WslcCreateSession(&mut settings, &mut raw_session, &mut error_message);
}
```

## 10. 测试策略

### 单元测试

- 字符串转换；
- HRESULT 映射；
- bitflags 转换；
- Drop 顺序；
- callback 复制数据。

### 集成测试

要求本机存在 WSLC：

```powershell
wsl --update
wslc run --rm hello-world
cargo test --features integration
```

用 feature 控制：

```toml
[features]
integration = []
```

避免普通 CI / crate 用户在无 WSLC 环境下测试失败。

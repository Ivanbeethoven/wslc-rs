use std::ffi::{c_char, c_void, OsStr};
use std::ptr::NonNull;
use std::sync::OnceLock;

use crate::strings;
use crate::{Error, Result};

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(lpLibFileName: *const u16) -> *mut c_void;
    fn GetProcAddress(hModule: *mut c_void, lpProcName: *const c_char) -> *mut c_void;
    fn FreeLibrary(hLibModule: *mut c_void) -> i32;
    fn WaitForSingleObject(hHandle: *mut c_void, dwMilliseconds: u32) -> u32;
}

#[cfg(windows)]
#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: u32) -> wslc_sys::HRESULT;
    fn CoUninitialize();
    fn CoTaskMemFree(pv: *mut c_void);
}

pub(crate) struct Library {
    handle: NonNull<c_void>,
}

impl Library {
    #[cfg(windows)]
    fn load(name: &OsStr) -> Result<Self> {
        let wide = strings::wide_os(name);
        let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
        let handle = NonNull::new(handle).ok_or_else(|| {
            Error::SdkNotFound(
                "could not load wslcsdk.dll; install Microsoft.WSL.Containers or put the SDK native directory on PATH"
                    .to_owned(),
            )
        })?;
        Ok(Self { handle })
    }

    #[cfg(not(windows))]
    fn load(_name: &OsStr) -> Result<Self> {
        Err(Error::UnsupportedPlatform(
            "WSLC requires a Windows host target",
        ))
    }

    #[cfg(windows)]
    unsafe fn get<T: Copy>(&self, symbol: &'static [u8]) -> Result<T> {
        let ptr = unsafe { GetProcAddress(self.handle.as_ptr(), symbol.as_ptr().cast()) };
        if ptr.is_null() {
            let name = std::str::from_utf8(symbol)
                .unwrap_or("<invalid symbol>")
                .trim_end_matches('\0');
            return Err(Error::SdkNotFound(format!(
                "wslcsdk.dll does not export {name}"
            )));
        }

        Ok(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&ptr) })
    }
}

unsafe impl Send for Library {}
unsafe impl Sync for Library {}

impl Drop for Library {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            let _ = FreeLibrary(self.handle.as_ptr());
        }
    }
}

#[allow(non_snake_case, dead_code)]
pub(crate) struct Sdk {
    _library: Library,
    pub WslcInitSessionSettings: wslc_sys::WslcInitSessionSettingsFn,
    pub WslcCreateSession: wslc_sys::WslcCreateSessionFn,
    pub WslcSetSessionSettingsCpuCount: wslc_sys::WslcSetSessionSettingsCpuCountFn,
    pub WslcSetSessionSettingsMemory: wslc_sys::WslcSetSessionSettingsMemoryFn,
    pub WslcSetSessionSettingsTimeout: wslc_sys::WslcSetSessionSettingsTimeoutFn,
    pub WslcSetSessionSettingsVhd: wslc_sys::WslcSetSessionSettingsVhdFn,
    pub WslcSetSessionSettingsFeatureFlags: wslc_sys::WslcSetSessionSettingsFeatureFlagsFn,
    pub WslcTerminateSession: wslc_sys::WslcTerminateSessionFn,
    pub WslcReleaseSession: wslc_sys::WslcReleaseSessionFn,
    pub WslcInitContainerSettings: wslc_sys::WslcInitContainerSettingsFn,
    pub WslcCreateContainer: wslc_sys::WslcCreateContainerFn,
    pub WslcStartContainer: wslc_sys::WslcStartContainerFn,
    pub WslcSetContainerSettingsName: wslc_sys::WslcSetContainerSettingsNameFn,
    pub WslcSetContainerSettingsInitProcess: wslc_sys::WslcSetContainerSettingsInitProcessFn,
    pub WslcSetContainerSettingsNetworkingMode: wslc_sys::WslcSetContainerSettingsNetworkingModeFn,
    pub WslcSetContainerSettingsHostName: wslc_sys::WslcSetContainerSettingsHostNameFn,
    pub WslcSetContainerSettingsDomainName: wslc_sys::WslcSetContainerSettingsDomainNameFn,
    pub WslcSetContainerSettingsFlags: wslc_sys::WslcSetContainerSettingsFlagsFn,
    pub WslcSetContainerSettingsPortMappings: wslc_sys::WslcSetContainerSettingsPortMappingsFn,
    pub WslcSetContainerSettingsVolumes: wslc_sys::WslcSetContainerSettingsVolumesFn,
    pub WslcSetContainerSettingsNamedVolumes: wslc_sys::WslcSetContainerSettingsNamedVolumesFn,
    pub WslcCreateContainerProcess: wslc_sys::WslcCreateContainerProcessFn,
    pub WslcReleaseContainer: wslc_sys::WslcReleaseContainerFn,
    pub WslcGetContainerID: wslc_sys::WslcGetContainerIDFn,
    pub WslcGetContainerInitProcess: wslc_sys::WslcGetContainerInitProcessFn,
    pub WslcInspectContainer: wslc_sys::WslcInspectContainerFn,
    pub WslcGetContainerState: wslc_sys::WslcGetContainerStateFn,
    pub WslcStopContainer: wslc_sys::WslcStopContainerFn,
    pub WslcDeleteContainer: wslc_sys::WslcDeleteContainerFn,
    pub WslcInitProcessSettings: wslc_sys::WslcInitProcessSettingsFn,
    pub WslcSetProcessSettingsWorkingDirectory: wslc_sys::WslcSetProcessSettingsWorkingDirectoryFn,
    pub WslcSetProcessSettingsCmdLine: wslc_sys::WslcSetProcessSettingsCmdLineFn,
    pub WslcSetProcessSettingsEnvVariables: wslc_sys::WslcSetProcessSettingsEnvVariablesFn,
    pub WslcSetProcessSettingsCallbacks: wslc_sys::WslcSetProcessSettingsCallbacksFn,
    pub WslcGetProcessPid: wslc_sys::WslcGetProcessPidFn,
    pub WslcGetProcessExitEvent: wslc_sys::WslcGetProcessExitEventFn,
    pub WslcGetProcessState: wslc_sys::WslcGetProcessStateFn,
    pub WslcGetProcessExitCode: wslc_sys::WslcGetProcessExitCodeFn,
    pub WslcSignalProcess: wslc_sys::WslcSignalProcessFn,
    pub WslcReleaseProcess: wslc_sys::WslcReleaseProcessFn,
    pub WslcPullSessionImage: wslc_sys::WslcPullSessionImageFn,
    pub WslcDeleteSessionImage: wslc_sys::WslcDeleteSessionImageFn,
    pub WslcTagSessionImage: wslc_sys::WslcTagSessionImageFn,
    pub WslcListSessionImages: wslc_sys::WslcListSessionImagesFn,
    pub WslcCreateSessionVhdVolume: wslc_sys::WslcCreateSessionVhdVolumeFn,
    pub WslcDeleteSessionVhdVolume: wslc_sys::WslcDeleteSessionVhdVolumeFn,
    pub WslcGetMissingComponents: wslc_sys::WslcGetMissingComponentsFn,
    pub WslcGetVersion: wslc_sys::WslcGetVersionFn,
    pub WslcInstallWithDependencies: wslc_sys::WslcInstallWithDependenciesFn,
}

impl Sdk {
    fn load() -> Result<Self> {
        let library = Library::load(OsStr::new("wslcsdk.dll"))?;
        macro_rules! sym {
            ($field:ident) => {
                unsafe { library.get(concat!(stringify!($field), "\0").as_bytes())? }
            };
        }

        Ok(Self {
            WslcInitSessionSettings: sym!(WslcInitSessionSettings),
            WslcCreateSession: sym!(WslcCreateSession),
            WslcSetSessionSettingsCpuCount: sym!(WslcSetSessionSettingsCpuCount),
            WslcSetSessionSettingsMemory: sym!(WslcSetSessionSettingsMemory),
            WslcSetSessionSettingsTimeout: sym!(WslcSetSessionSettingsTimeout),
            WslcSetSessionSettingsVhd: sym!(WslcSetSessionSettingsVhd),
            WslcSetSessionSettingsFeatureFlags: sym!(WslcSetSessionSettingsFeatureFlags),
            WslcTerminateSession: sym!(WslcTerminateSession),
            WslcReleaseSession: sym!(WslcReleaseSession),
            WslcInitContainerSettings: sym!(WslcInitContainerSettings),
            WslcCreateContainer: sym!(WslcCreateContainer),
            WslcStartContainer: sym!(WslcStartContainer),
            WslcSetContainerSettingsName: sym!(WslcSetContainerSettingsName),
            WslcSetContainerSettingsInitProcess: sym!(WslcSetContainerSettingsInitProcess),
            WslcSetContainerSettingsNetworkingMode: sym!(WslcSetContainerSettingsNetworkingMode),
            WslcSetContainerSettingsHostName: sym!(WslcSetContainerSettingsHostName),
            WslcSetContainerSettingsDomainName: sym!(WslcSetContainerSettingsDomainName),
            WslcSetContainerSettingsFlags: sym!(WslcSetContainerSettingsFlags),
            WslcSetContainerSettingsPortMappings: sym!(WslcSetContainerSettingsPortMappings),
            WslcSetContainerSettingsVolumes: sym!(WslcSetContainerSettingsVolumes),
            WslcSetContainerSettingsNamedVolumes: sym!(WslcSetContainerSettingsNamedVolumes),
            WslcCreateContainerProcess: sym!(WslcCreateContainerProcess),
            WslcReleaseContainer: sym!(WslcReleaseContainer),
            WslcGetContainerID: sym!(WslcGetContainerID),
            WslcGetContainerInitProcess: sym!(WslcGetContainerInitProcess),
            WslcInspectContainer: sym!(WslcInspectContainer),
            WslcGetContainerState: sym!(WslcGetContainerState),
            WslcStopContainer: sym!(WslcStopContainer),
            WslcDeleteContainer: sym!(WslcDeleteContainer),
            WslcInitProcessSettings: sym!(WslcInitProcessSettings),
            WslcSetProcessSettingsWorkingDirectory: sym!(WslcSetProcessSettingsWorkingDirectory),
            WslcSetProcessSettingsCmdLine: sym!(WslcSetProcessSettingsCmdLine),
            WslcSetProcessSettingsEnvVariables: sym!(WslcSetProcessSettingsEnvVariables),
            WslcSetProcessSettingsCallbacks: sym!(WslcSetProcessSettingsCallbacks),
            WslcGetProcessPid: sym!(WslcGetProcessPid),
            WslcGetProcessExitEvent: sym!(WslcGetProcessExitEvent),
            WslcGetProcessState: sym!(WslcGetProcessState),
            WslcGetProcessExitCode: sym!(WslcGetProcessExitCode),
            WslcSignalProcess: sym!(WslcSignalProcess),
            WslcReleaseProcess: sym!(WslcReleaseProcess),
            WslcPullSessionImage: sym!(WslcPullSessionImage),
            WslcDeleteSessionImage: sym!(WslcDeleteSessionImage),
            WslcTagSessionImage: sym!(WslcTagSessionImage),
            WslcListSessionImages: sym!(WslcListSessionImages),
            WslcCreateSessionVhdVolume: sym!(WslcCreateSessionVhdVolume),
            WslcDeleteSessionVhdVolume: sym!(WslcDeleteSessionVhdVolume),
            WslcGetMissingComponents: sym!(WslcGetMissingComponents),
            WslcGetVersion: sym!(WslcGetVersion),
            WslcInstallWithDependencies: sym!(WslcInstallWithDependencies),
            _library: library,
        })
    }
}

pub(crate) fn sdk() -> Result<&'static Sdk> {
    static SDK: OnceLock<Result<Sdk>> = OnceLock::new();
    SDK.get_or_init(Sdk::load).as_ref().map_err(clone_error)
}

fn clone_error(error: &Error) -> Error {
    match error {
        Error::UnsupportedPlatform(value) => Error::UnsupportedPlatform(value),
        Error::SdkNotFound(value) => Error::SdkNotFound(value.clone()),
        Error::MissingComponents(value) => Error::MissingComponents(*value),
        Error::ComInitialization { code, message } => Error::ComInitialization {
            code: *code,
            message: message.clone(),
        },
        Error::HResult { code, message } => Error::HResult {
            code: *code,
            message: message.clone(),
        },
        Error::InvalidInput(value) => Error::InvalidInput(value.clone()),
        Error::Nul(value) => Error::Nul(value.clone()),
        Error::Io(value) => Error::Io(std::io::Error::new(value.kind(), value.to_string())),
        Error::Utf8(value) => Error::InvalidInput(value.to_string()),
    }
}

pub(crate) fn succeeded(hr: wslc_sys::HRESULT) -> bool {
    hr >= 0
}

pub(crate) unsafe fn check_hr(hr: wslc_sys::HRESULT, error_message: wslc_sys::PWSTR) -> Result<()> {
    if succeeded(hr) {
        free_cotaskmem(error_message.cast());
        return Ok(());
    }

    let message = unsafe { strings::utf16_ptr_to_string(error_message) };
    free_cotaskmem(error_message.cast());
    Err(Error::from_hresult(hr, message))
}

pub(crate) fn co_initialize_ex(reserved: *mut c_void, coinit: u32) -> wslc_sys::HRESULT {
    #[cfg(windows)]
    unsafe {
        CoInitializeEx(reserved, coinit)
    }

    #[cfg(not(windows))]
    {
        let _ = (reserved, coinit);
        0x8000_4001_u32 as i32
    }
}

pub(crate) fn co_uninitialize() {
    #[cfg(windows)]
    unsafe {
        CoUninitialize();
    }
}

pub(crate) fn free_cotaskmem(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }

    #[cfg(windows)]
    unsafe {
        CoTaskMemFree(ptr);
    }

    #[cfg(not(windows))]
    {
        let _ = ptr;
    }
}

pub(crate) fn wait_for_single_object(handle: wslc_sys::HANDLE, timeout_ms: u32) -> u32 {
    #[cfg(windows)]
    unsafe {
        WaitForSingleObject(handle, timeout_ms)
    }

    #[cfg(not(windows))]
    {
        let _ = (handle, timeout_ms);
        0xffff_ffff
    }
}

use std::path::PathBuf;
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::Duration;

use crate::process::{CaptureRegistration, Output, Process, ProcessInner, ProcessOptions};
use crate::{raw, strings, Error, Result, Session};

/// Container creation options.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerOptions {
    /// Image name or ID.
    pub image: String,
}

impl ContainerOptions {
    /// Creates container options.
    pub fn new(image: impl Into<String>) -> Self {
        Self {
            image: image.into(),
        }
    }

    /// Validates container options.
    pub fn validate(&self) -> Result<()> {
        if self.image.trim().is_empty() {
            return Err(Error::InvalidInput(
                "container image cannot be empty".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Container networking mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkingMode {
    /// No networking.
    None,
    /// Bridged networking.
    Bridged,
}

impl Default for NetworkingMode {
    fn default() -> Self {
        Self::Bridged
    }
}

impl NetworkingMode {
    fn as_raw(self) -> wslc_sys::WslcContainerNetworkingMode {
        match self {
            Self::None => {
                wslc_sys::WslcContainerNetworkingMode::WSLC_CONTAINER_NETWORKING_MODE_NONE
            }
            Self::Bridged => {
                wslc_sys::WslcContainerNetworkingMode::WSLC_CONTAINER_NETWORKING_MODE_BRIDGED
            }
        }
    }
}

/// Linux signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Signal {
    /// SIGHUP.
    Sighup,
    /// SIGINT.
    Sigint,
    /// SIGQUIT.
    Sigquit,
    /// SIGKILL.
    Sigkill,
    /// SIGTERM.
    Sigterm,
}

impl Signal {
    /// Returns the Linux signal number.
    pub const fn as_raw(self) -> i32 {
        match self {
            Self::Sighup => 1,
            Self::Sigint => 2,
            Self::Sigquit => 3,
            Self::Sigkill => 9,
            Self::Sigterm => 15,
        }
    }

    pub(crate) fn as_raw_signal(self) -> wslc_sys::WslcSignal {
        match self {
            Self::Sighup => wslc_sys::WslcSignal::WSLC_SIGNAL_SIGHUP,
            Self::Sigint => wslc_sys::WslcSignal::WSLC_SIGNAL_SIGINT,
            Self::Sigquit => wslc_sys::WslcSignal::WSLC_SIGNAL_SIGQUIT,
            Self::Sigkill => wslc_sys::WslcSignal::WSLC_SIGNAL_SIGKILL,
            Self::Sigterm => wslc_sys::WslcSignal::WSLC_SIGNAL_SIGTERM,
        }
    }
}

/// Options for deleting a container.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeleteContainerOptions {
    /// Force deletion.
    pub force: bool,
}

impl DeleteContainerOptions {
    /// Enables force deletion.
    pub fn force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    fn as_raw(self) -> wslc_sys::WslcDeleteContainerFlags {
        if self.force {
            wslc_sys::WSLC_DELETE_CONTAINER_FLAG_FORCE
        } else {
            wslc_sys::WSLC_DELETE_CONTAINER_FLAG_NONE
        }
    }
}

#[derive(Clone, Debug)]
struct PortMapping {
    windows_port: u16,
    container_port: u16,
}

#[derive(Clone, Debug)]
struct VolumeMount {
    windows_path: PathBuf,
    container_path: String,
    read_only: bool,
}

/// Container builder.
pub struct ContainerBuilder {
    session: Session,
    options: ContainerOptions,
    name: Option<String>,
    hostname: Option<String>,
    domain_name: Option<String>,
    networking: NetworkingMode,
    flags: u32,
    init_process: Option<ProcessOptions>,
    ports: Vec<PortMapping>,
    volumes: Vec<VolumeMount>,
}

impl ContainerBuilder {
    pub(crate) fn new(session: Session, options: ContainerOptions) -> Self {
        Self {
            session,
            options,
            name: None,
            hostname: None,
            domain_name: None,
            networking: NetworkingMode::default(),
            flags: wslc_sys::WSLC_CONTAINER_FLAG_NONE,
            init_process: None,
            ports: Vec::new(),
            volumes: Vec::new(),
        }
    }

    /// Sets the container name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the hostname.
    pub fn hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = Some(hostname.into());
        self
    }

    /// Sets the domain name.
    pub fn domain_name(mut self, domain: impl Into<String>) -> Self {
        self.domain_name = Some(domain.into());
        self
    }

    /// Sets networking mode.
    pub fn networking(mut self, mode: NetworkingMode) -> Self {
        self.networking = mode;
        self
    }

    /// Enables auto-remove.
    pub fn auto_remove(mut self, enable: bool) -> Self {
        set_flag(
            &mut self.flags,
            wslc_sys::WSLC_CONTAINER_FLAG_AUTO_REMOVE,
            enable,
        );
        self
    }

    /// Enables privileged mode.
    pub fn privileged(mut self, enable: bool) -> Self {
        set_flag(
            &mut self.flags,
            wslc_sys::WSLC_CONTAINER_FLAG_PRIVILEGED,
            enable,
        );
        self
    }

    /// Enables GPU.
    pub fn enable_gpu(mut self, enable: bool) -> Self {
        set_flag(
            &mut self.flags,
            wslc_sys::WSLC_CONTAINER_FLAG_ENABLE_GPU,
            enable,
        );
        self
    }

    /// Sets the init process.
    pub fn init_process(mut self, process: ProcessOptions) -> Self {
        self.init_process = Some(process);
        self
    }

    /// Adds a TCP port mapping.
    pub fn port(mut self, windows_port: u16, container_port: u16) -> Self {
        self.ports.push(PortMapping {
            windows_port,
            container_port,
        });
        self
    }

    /// Adds a volume mount.
    pub fn volume(
        mut self,
        windows_path: impl Into<PathBuf>,
        container_path: impl Into<String>,
    ) -> Self {
        self.volumes.push(VolumeMount {
            windows_path: windows_path.into(),
            container_path: container_path.into(),
            read_only: false,
        });
        self
    }

    fn validate(&self) -> Result<()> {
        self.options.validate()?;
        for port in &self.ports {
            if port.windows_port == 0 || port.container_port == 0 {
                return Err(Error::InvalidInput(
                    "port mappings cannot use port 0".to_owned(),
                ));
            }
        }
        for volume in &self.volumes {
            if volume.container_path.is_empty() || !volume.container_path.starts_with('/') {
                return Err(Error::InvalidInput(
                    "container volume path must be absolute".to_owned(),
                ));
            }
            if volume.windows_path.as_os_str().is_empty() {
                return Err(Error::InvalidInput(
                    "windows volume path cannot be empty".to_owned(),
                ));
            }
        }
        if let Some(process) = &self.init_process {
            process.validate()?;
        }
        Ok(())
    }

    /// Creates the container.
    pub fn create(self) -> Result<Container> {
        self.validate()?;
        let sdk = raw::sdk()?;
        let image = strings::cstring(&self.options.image, "image")?;
        let mut settings = wslc_sys::WslcContainerSettings::default();
        let hr = unsafe { (sdk.WslcInitContainerSettings)(image.as_ptr(), &mut settings) };
        unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;

        let name;
        if let Some(value) = &self.name {
            name = strings::cstring(value, "container name")?;
            let hr = unsafe { (sdk.WslcSetContainerSettingsName)(&mut settings, name.as_ptr()) };
            unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        }
        let hostname;
        if let Some(value) = &self.hostname {
            hostname = strings::cstring(value, "hostname")?;
            let hr =
                unsafe { (sdk.WslcSetContainerSettingsHostName)(&mut settings, hostname.as_ptr()) };
            unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        }
        let domain_name;
        if let Some(value) = &self.domain_name {
            domain_name = strings::cstring(value, "domain_name")?;
            let hr = unsafe {
                (sdk.WslcSetContainerSettingsDomainName)(&mut settings, domain_name.as_ptr())
            };
            unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        }

        let hr = unsafe {
            (sdk.WslcSetContainerSettingsNetworkingMode)(&mut settings, self.networking.as_raw())
        };
        unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;

        let hr = unsafe { (sdk.WslcSetContainerSettingsFlags)(&mut settings, self.flags) };
        unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;

        let capture = if self.init_process.is_some() {
            Some(CaptureRegistration::new())
        } else {
            None
        };
        let mut init_settings;
        if let Some(process) = &self.init_process {
            init_settings = process.to_raw(sdk, capture.as_ref())?;
            let hr = unsafe {
                (sdk.WslcSetContainerSettingsInitProcess)(&mut settings, &mut init_settings)
            };
            unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        }

        let raw_ports: Vec<_> = self
            .ports
            .iter()
            .map(|port| wslc_sys::WslcContainerPortMapping {
                windowsPort: port.windows_port,
                containerPort: port.container_port,
                protocol: wslc_sys::WslcPortProtocol::WSLC_PORT_PROTOCOL_TCP,
                windowsAddress: std::ptr::null_mut(),
            })
            .collect();
        if !raw_ports.is_empty() {
            let hr = unsafe {
                (sdk.WslcSetContainerSettingsPortMappings)(
                    &mut settings,
                    raw_ports.as_ptr(),
                    raw_ports.len() as u32,
                )
            };
            unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        }

        let windows_paths: Vec<_> = self
            .volumes
            .iter()
            .map(|volume| strings::wide_path(&volume.windows_path))
            .collect();
        let container_paths = strings::cstrings(
            self.volumes
                .iter()
                .map(|volume| volume.container_path.as_str()),
            "container volume path",
        )?;
        let raw_volumes: Vec<_> = self
            .volumes
            .iter()
            .enumerate()
            .map(|(index, volume)| wslc_sys::WslcContainerVolume {
                windowsPath: windows_paths[index].as_ptr(),
                containerPath: container_paths[index].as_ptr(),
                readOnly: i32::from(volume.read_only),
            })
            .collect();
        if !raw_volumes.is_empty() {
            let hr = unsafe {
                (sdk.WslcSetContainerSettingsVolumes)(
                    &mut settings,
                    raw_volumes.as_ptr(),
                    raw_volumes.len() as u32,
                )
            };
            unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        }

        let mut raw_container = std::ptr::null_mut();
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe {
            (sdk.WslcCreateContainer)(
                self.session.raw(),
                &settings,
                &mut raw_container,
                &mut error_message,
            )
        };
        unsafe { raw::check_hr(hr, error_message) }?;
        let raw = NonNull::new(raw_container).ok_or_else(|| {
            Error::from_hresult(
                wslc_sys::S_OK,
                "WslcCreateContainer returned a null container",
            )
        })?;
        Ok(Container {
            inner: Rc::new(ContainerInner {
                raw,
                session: self.session.inner.clone(),
                init_capture: capture,
            }),
        })
    }
}

fn set_flag(flags: &mut u32, flag: u32, enable: bool) {
    if enable {
        *flags |= flag;
    } else {
        *flags &= !flag;
    }
}

/// Container state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerState {
    /// Invalid state.
    Invalid,
    /// Created.
    Created,
    /// Running.
    Running,
    /// Exited.
    Exited,
    /// Deleted.
    Deleted,
}

impl From<wslc_sys::WslcContainerState> for ContainerState {
    fn from(value: wslc_sys::WslcContainerState) -> Self {
        match value {
            wslc_sys::WslcContainerState::WSLC_CONTAINER_STATE_CREATED => Self::Created,
            wslc_sys::WslcContainerState::WSLC_CONTAINER_STATE_RUNNING => Self::Running,
            wslc_sys::WslcContainerState::WSLC_CONTAINER_STATE_EXITED => Self::Exited,
            wslc_sys::WslcContainerState::WSLC_CONTAINER_STATE_DELETED => Self::Deleted,
            _ => Self::Invalid,
        }
    }
}

pub(crate) struct ContainerInner {
    raw: NonNull<std::ffi::c_void>,
    #[allow(dead_code)]
    session: Rc<crate::session::SessionInner>,
    init_capture: Option<CaptureRegistration>,
}

/// WSLC container handle.
pub struct Container {
    pub(crate) inner: Rc<ContainerInner>,
}

impl std::fmt::Debug for Container {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Container").finish_non_exhaustive()
    }
}

impl Container {
    fn raw(&self) -> wslc_sys::WslcContainer {
        self.inner.raw.as_ptr()
    }

    /// Returns the container ID.
    pub fn id(&self) -> Result<String> {
        let sdk = raw::sdk()?;
        let mut id = [0i8; wslc_sys::WSLC_CONTAINER_ID_BUFFER_SIZE];
        let hr = unsafe { (sdk.WslcGetContainerID)(self.raw(), id.as_mut_ptr()) };
        unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        unsafe { strings::c_ptr_to_string(id.as_ptr()) }
    }

    /// Starts the container.
    pub fn start(&self) -> Result<()> {
        let sdk = raw::sdk()?;
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe {
            (sdk.WslcStartContainer)(
                self.raw(),
                wslc_sys::WSLC_CONTAINER_START_FLAG_NONE,
                &mut error_message,
            )
        };
        unsafe { raw::check_hr(hr, error_message) }
    }

    /// Starts the container attached.
    pub fn start_attached(&self) -> Result<()> {
        let sdk = raw::sdk()?;
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe {
            (sdk.WslcStartContainer)(
                self.raw(),
                wslc_sys::WSLC_CONTAINER_START_FLAG_ATTACH,
                &mut error_message,
            )
        };
        unsafe { raw::check_hr(hr, error_message) }
    }

    /// Starts the container and returns captured output.
    pub fn start_and_wait(&self) -> Result<Output> {
        self.start_attached()?;
        Ok(self
            .inner
            .init_capture
            .as_ref()
            .map(CaptureRegistration::wait_output)
            .unwrap_or_default())
    }

    /// Returns raw inspect JSON.
    pub fn inspect(&self) -> Result<String> {
        let sdk = raw::sdk()?;
        let mut inspect = std::ptr::null_mut();
        let hr = unsafe { (sdk.WslcInspectContainer)(self.raw(), &mut inspect) };
        unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        let value = unsafe { strings::c_ptr_to_string(inspect) }?;
        raw::free_cotaskmem(inspect.cast());
        Ok(value)
    }

    /// Returns container state.
    pub fn state(&self) -> Result<ContainerState> {
        let sdk = raw::sdk()?;
        let mut state = wslc_sys::WslcContainerState::WSLC_CONTAINER_STATE_INVALID;
        let hr = unsafe { (sdk.WslcGetContainerState)(self.raw(), &mut state) };
        unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        Ok(state.into())
    }

    /// Creates an additional process in the container.
    pub fn exec(&self, options: ProcessOptions) -> Result<Process> {
        let sdk = raw::sdk()?;
        let mut process_settings = options.to_raw(sdk, None)?;
        let mut raw_process = std::ptr::null_mut();
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe {
            (sdk.WslcCreateContainerProcess)(
                self.raw(),
                &mut process_settings,
                &mut raw_process,
                &mut error_message,
            )
        };
        unsafe { raw::check_hr(hr, error_message) }?;
        let raw = NonNull::new(raw_process).ok_or_else(|| {
            Error::from_hresult(
                wslc_sys::S_OK,
                "WslcCreateContainerProcess returned a null process",
            )
        })?;
        Ok(Process {
            inner: Rc::new(ProcessInner { raw: raw.as_ptr() }),
        })
    }

    /// Stops the container.
    pub fn stop(&self, signal: Signal, timeout: Duration) -> Result<()> {
        let timeout_seconds = u32::try_from(timeout.as_secs())
            .map_err(|_| Error::InvalidInput("timeout does not fit in u32 seconds".to_owned()))?;
        let sdk = raw::sdk()?;
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe {
            (sdk.WslcStopContainer)(
                self.raw(),
                signal.as_raw_signal(),
                timeout_seconds,
                &mut error_message,
            )
        };
        unsafe { raw::check_hr(hr, error_message) }
    }

    /// Deletes the container.
    pub fn delete(&self, options: DeleteContainerOptions) -> Result<()> {
        let sdk = raw::sdk()?;
        let mut error_message = std::ptr::null_mut();
        let hr =
            unsafe { (sdk.WslcDeleteContainer)(self.raw(), options.as_raw(), &mut error_message) };
        unsafe { raw::check_hr(hr, error_message) }
    }
}

impl Drop for ContainerInner {
    fn drop(&mut self) {
        if let Ok(sdk) = raw::sdk() {
            let _com = crate::com::try_initialize_mta().ok().flatten();
            unsafe {
                let _ = (sdk.WslcReleaseContainer)(self.raw.as_ptr());
            }
        }
    }
}

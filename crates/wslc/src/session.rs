use std::path::PathBuf;
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::Duration;

use crate::container::{ContainerBuilder, ContainerOptions};
use crate::image::{ImageInfo, ImagePullOperation, ImagePullOptions};
use crate::storage::VhdOptions;
use crate::{com, raw, strings, Error, Result};

/// WSLC session handle.
#[derive(Clone)]
pub struct Session {
    pub(crate) inner: Rc<SessionInner>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").finish_non_exhaustive()
    }
}

pub(crate) struct SessionInner {
    pub raw: NonNull<std::ffi::c_void>,
    terminate_on_drop: bool,
    _com: Option<com::ComGuard>,
}

impl Session {
    /// Creates a session builder.
    pub fn builder(name: impl Into<String>, storage_path: impl Into<PathBuf>) -> SessionBuilder {
        SessionBuilder {
            name: name.into(),
            storage_path: storage_path.into(),
            cpu_count: None,
            memory_mb: None,
            timeout_ms: None,
            enable_gpu: false,
            vhd: None,
            terminate_on_drop: false,
        }
    }

    pub(crate) fn raw(&self) -> wslc_sys::WslcSession {
        self.inner.raw.as_ptr()
    }

    /// Starts an image pull operation.
    pub fn pull_image(&self, options: ImagePullOptions) -> ImagePullOperation<'_> {
        ImagePullOperation {
            session: self,
            options,
            progress: None,
        }
    }

    /// Lists images in the session.
    pub fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let sdk = raw::sdk()?;
        let mut raw_images = std::ptr::null_mut();
        let mut count = 0;
        let hr = unsafe { (sdk.WslcListSessionImages)(self.raw(), &mut raw_images, &mut count) };
        unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        if raw_images.is_null() || count == 0 {
            return Ok(Vec::new());
        }

        let slice = unsafe { std::slice::from_raw_parts(raw_images, count as usize) };
        let images = slice
            .iter()
            .copied()
            .map(ImageInfo::try_from)
            .collect::<Result<Vec<_>>>()?;
        raw::free_cotaskmem(raw_images.cast());
        Ok(images)
    }

    /// Deletes an image by name or ID.
    pub fn delete_image(&self, name_or_id: impl AsRef<str>) -> Result<()> {
        let name_or_id = name_or_id.as_ref();
        if name_or_id.trim().is_empty() {
            return Err(Error::InvalidInput(
                "image name or id cannot be empty".to_owned(),
            ));
        }
        let sdk = raw::sdk()?;
        let name_or_id = strings::cstring(name_or_id, "name_or_id")?;
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe {
            (sdk.WslcDeleteSessionImage)(self.raw(), name_or_id.as_ptr(), &mut error_message)
        };
        unsafe { raw::check_hr(hr, error_message) }
    }

    /// Tags an image.
    pub fn tag_image(
        &self,
        image: impl AsRef<str>,
        repo: impl AsRef<str>,
        tag: impl AsRef<str>,
    ) -> Result<()> {
        let image = image.as_ref();
        let repo = repo.as_ref();
        let tag = tag.as_ref();
        if image.is_empty() || repo.is_empty() || tag.is_empty() {
            return Err(Error::InvalidInput(
                "image, repo, and tag cannot be empty".to_owned(),
            ));
        }

        let sdk = raw::sdk()?;
        let image = strings::cstring(image, "image")?;
        let repo = strings::cstring(repo, "repo")?;
        let tag = strings::cstring(tag, "tag")?;
        let options = wslc_sys::WslcTagImageOptions {
            image: image.as_ptr(),
            repo: repo.as_ptr(),
            tag: tag.as_ptr(),
        };
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe { (sdk.WslcTagSessionImage)(self.raw(), &options, &mut error_message) };
        unsafe { raw::check_hr(hr, error_message) }
    }

    /// Creates a container builder.
    pub fn container(&self, options: ContainerOptions) -> ContainerBuilder {
        ContainerBuilder::new(self.clone(), options)
    }

    /// Creates a VHD-backed volume.
    pub fn create_vhd_volume(&self, options: VhdOptions) -> Result<()> {
        let sdk = raw::sdk()?;
        let name = strings::cstring(&options.name, "vhd name")?;
        let (flags, uid, gid) = if let Some((uid, gid)) = options.owner {
            (wslc_sys::WSLC_VHD_REQ_FLAG_OWNER, uid, gid)
        } else {
            (wslc_sys::WSLC_VHD_REQ_FLAG_NONE, 0, 0)
        };
        let raw_options = wslc_sys::WslcVhdRequirements {
            name: name.as_ptr(),
            sizeBytes: options.size_bytes,
            type_: options.vhd_type.as_raw(),
            flags,
            uid,
            gid,
        };
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe {
            (sdk.WslcCreateSessionVhdVolume)(self.raw(), &raw_options, &mut error_message)
        };
        unsafe { raw::check_hr(hr, error_message) }
    }

    /// Deletes a VHD-backed volume.
    pub fn delete_vhd_volume(&self, name: impl AsRef<str>) -> Result<()> {
        let name = name.as_ref();
        if name.trim().is_empty() {
            return Err(Error::InvalidInput(
                "volume name cannot be empty".to_owned(),
            ));
        }
        let sdk = raw::sdk()?;
        let name = strings::cstring(name, "volume name")?;
        let mut error_message = std::ptr::null_mut();
        let hr = unsafe {
            (sdk.WslcDeleteSessionVhdVolume)(self.raw(), name.as_ptr(), &mut error_message)
        };
        unsafe { raw::check_hr(hr, error_message) }
    }

    /// Terminates the session.
    pub fn terminate(&self) -> Result<()> {
        let sdk = raw::sdk()?;
        let hr = unsafe { (sdk.WslcTerminateSession)(self.raw()) };
        unsafe { raw::check_hr(hr, std::ptr::null_mut()) }
    }
}

/// Session builder.
pub struct SessionBuilder {
    name: String,
    storage_path: PathBuf,
    cpu_count: Option<u32>,
    memory_mb: Option<u32>,
    timeout_ms: Option<u32>,
    enable_gpu: bool,
    vhd: Option<VhdOptions>,
    terminate_on_drop: bool,
}

impl SessionBuilder {
    /// Sets CPU count.
    pub fn cpu_count(mut self, cpu_count: u32) -> Self {
        self.cpu_count = Some(cpu_count);
        self
    }

    /// Sets memory in MB.
    pub fn memory_mb(mut self, memory_mb: u32) -> Self {
        self.memory_mb = Some(memory_mb);
        self
    }

    /// Sets create timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout_ms = u32::try_from(timeout.as_millis()).ok();
        if self.timeout_ms.is_none() {
            self.timeout_ms = Some(u32::MAX);
        }
        self
    }

    /// Enables GPU support.
    pub fn enable_gpu(mut self, enable: bool) -> Self {
        self.enable_gpu = enable;
        self
    }

    /// Sets a session VHD.
    pub fn vhd(mut self, options: VhdOptions) -> Self {
        self.vhd = Some(options);
        self
    }

    /// Terminates the session when dropped.
    pub fn terminate_on_drop(mut self, enable: bool) -> Self {
        self.terminate_on_drop = enable;
        self
    }

    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::InvalidInput(
                "session name cannot be empty".to_owned(),
            ));
        }
        if self.name.contains('\0') {
            return Err(Error::Nul("session name".to_owned()));
        }
        if self.storage_path.as_os_str().is_empty() {
            return Err(Error::InvalidInput(
                "storage path cannot be empty".to_owned(),
            ));
        }
        if self.cpu_count == Some(0) {
            return Err(Error::InvalidInput(
                "cpu_count must be greater than zero".to_owned(),
            ));
        }
        if self.memory_mb == Some(0) {
            return Err(Error::InvalidInput(
                "memory_mb must be greater than zero".to_owned(),
            ));
        }
        if self.timeout_ms == Some(u32::MAX) {
            return Err(Error::InvalidInput(
                "timeout does not fit in u32 milliseconds".to_owned(),
            ));
        }
        Ok(())
    }

    /// Starts the WSLC session.
    pub fn start(self) -> Result<Session> {
        self.validate()?;
        let com = com::try_initialize_mta()?;
        let sdk = raw::sdk()?;
        let name = strings::wide_str(&self.name);
        let storage_path = strings::wide_path(&self.storage_path);
        let mut settings = wslc_sys::WslcSessionSettings::default();
        let hr = unsafe {
            (sdk.WslcInitSessionSettings)(name.as_ptr(), storage_path.as_ptr(), &mut settings)
        };
        unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;

        if let Some(cpu_count) = self.cpu_count {
            let hr = unsafe { (sdk.WslcSetSessionSettingsCpuCount)(&mut settings, cpu_count) };
            unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        }
        if let Some(memory_mb) = self.memory_mb {
            let hr = unsafe { (sdk.WslcSetSessionSettingsMemory)(&mut settings, memory_mb) };
            unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        }
        if let Some(timeout_ms) = self.timeout_ms {
            let hr = unsafe { (sdk.WslcSetSessionSettingsTimeout)(&mut settings, timeout_ms) };
            unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        }
        if self.enable_gpu {
            let hr = unsafe {
                (sdk.WslcSetSessionSettingsFeatureFlags)(
                    &mut settings,
                    wslc_sys::WSLC_SESSION_FEATURE_FLAG_ENABLE_GPU,
                )
            };
            unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        }

        let vhd_name;
        let vhd_raw;
        if let Some(vhd) = &self.vhd {
            vhd_name = strings::cstring(&vhd.name, "vhd name")?;
            let (flags, uid, gid) = if let Some((uid, gid)) = vhd.owner {
                (wslc_sys::WSLC_VHD_REQ_FLAG_OWNER, uid, gid)
            } else {
                (wslc_sys::WSLC_VHD_REQ_FLAG_NONE, 0, 0)
            };
            vhd_raw = wslc_sys::WslcVhdRequirements {
                name: vhd_name.as_ptr(),
                sizeBytes: vhd.size_bytes,
                type_: vhd.vhd_type.as_raw(),
                flags,
                uid,
                gid,
            };
            let hr = unsafe { (sdk.WslcSetSessionSettingsVhd)(&mut settings, &vhd_raw) };
            unsafe { raw::check_hr(hr, std::ptr::null_mut()) }?;
        }

        let mut raw_session = std::ptr::null_mut();
        let mut error_message = std::ptr::null_mut();
        let hr =
            unsafe { (sdk.WslcCreateSession)(&mut settings, &mut raw_session, &mut error_message) };
        unsafe { raw::check_hr(hr, error_message) }?;
        let raw = NonNull::new(raw_session).ok_or_else(|| {
            Error::from_hresult(wslc_sys::S_OK, "WslcCreateSession returned a null session")
        })?;
        Ok(Session {
            inner: Rc::new(SessionInner {
                raw,
                terminate_on_drop: self.terminate_on_drop,
                _com: com,
            }),
        })
    }
}

impl Drop for SessionInner {
    fn drop(&mut self) {
        if let Ok(sdk) = raw::sdk() {
            unsafe {
                if self.terminate_on_drop {
                    let _ = (sdk.WslcTerminateSession)(self.raw.as_ptr());
                }
                let _ = (sdk.WslcReleaseSession)(self.raw.as_ptr());
            }
        }
    }
}

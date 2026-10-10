use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::Duration;

use crate::container::{Container, ContainerBuilder, ContainerOptions};
use crate::image::{
    self, AuthenticateResult, IdentityTokenType, ImageInfo, ImagePullOperation, ImagePullOptions,
    ImagePushOptions,
};
use crate::logs::{self, ContainerLogs, LogsOptions};
use crate::storage::VhdOptions;
use crate::{com, raw, strings, Error, Result};

/// Session termination reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionTerminationReason {
    /// Unknown reason.
    Unknown,
    /// Session was shut down.
    Shutdown,
    /// Session crashed.
    Crashed,
}

impl From<wslc_sys::WslcSessionTerminationReason> for SessionTerminationReason {
    fn from(value: wslc_sys::WslcSessionTerminationReason) -> Self {
        match value {
            wslc_sys::WslcSessionTerminationReason::WSLC_SESSION_TERMINATION_REASON_SHUTDOWN => {
                Self::Shutdown
            }
            wslc_sys::WslcSessionTerminationReason::WSLC_SESSION_TERMINATION_REASON_CRASHED => {
                Self::Crashed
            }
            _ => Self::Unknown,
        }
    }
}

/// Crash dump information delivered by a session subscription.
pub type CrashDumpInfo = wslc_sys::runtime::CrashDumpInfo;
/// Owns a crash dump callback subscription.
pub type CrashDumpSubscription = wslc_sys::runtime::CrashDumpRegistration;

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
    pub name: String,
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

    /// Returns the session display name used when the session was created.
    pub fn name(&self) -> &str {
        &self.inner.name
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
        raw::map_result(sdk.list_session_images(self.raw()))?
            .into_iter()
            .map(ImageInfo::try_from)
            .collect::<Result<Vec<_>>>()
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
        raw::map_result(sdk.delete_session_image(self.raw(), name_or_id.as_ptr()))
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
        raw::map_result(sdk.tag_session_image(self.raw(), &options))
    }

    /// Imports an image from a local archive file.
    pub fn import_image_from_file(
        &self,
        image_name: impl AsRef<str>,
        path: impl AsRef<Path>,
    ) -> Result<()> {
        let image_name = image_name.as_ref();
        let path = path.as_ref();
        if image_name.trim().is_empty() {
            return Err(Error::InvalidInput("image name cannot be empty".to_owned()));
        }
        if image_name.contains('\0') {
            return Err(Error::Nul("image name".to_owned()));
        }
        image::validate_image_path(path, "image path")?;
        let sdk = raw::sdk()?;
        let image_name = strings::cstring(image_name, "image name")?;
        let path = strings::wide_path(path);
        raw::map_result(sdk.import_session_image_from_file(
            self.raw(),
            image_name.as_ptr(),
            path.as_ptr(),
        ))
    }

    /// Loads an image archive from a local file.
    pub fn load_image_from_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        image::validate_image_path(path, "image path")?;
        let sdk = raw::sdk()?;
        let path = strings::wide_path(path);
        raw::map_result(sdk.load_session_image_from_file(self.raw(), path.as_ptr()))
    }

    /// Pushes an image to a registry.
    pub fn push_image(&self, options: ImagePushOptions) -> Result<()> {
        options.validate()?;
        let sdk = raw::sdk()?;
        let image = strings::cstring(&options.image, "image")?;
        let registry_auth;
        let registry_auth_ptr = if let Some(auth) = &options.registry_auth {
            registry_auth = strings::cstring(auth, "registry_auth")?;
            registry_auth.as_ptr()
        } else {
            std::ptr::null()
        };
        raw::map_result(sdk.push_session_image(self.raw(), image.as_ptr(), registry_auth_ptr))
    }

    /// Authenticates with a container registry.
    pub fn authenticate(
        &self,
        server_address: impl AsRef<str>,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<AuthenticateResult> {
        let server_address = server_address.as_ref();
        let username = username.as_ref();
        let password = password.as_ref();
        if server_address.trim().is_empty() {
            return Err(Error::InvalidInput(
                "server address cannot be empty".to_owned(),
            ));
        }
        if server_address.contains('\0') {
            return Err(Error::Nul("server address".to_owned()));
        }
        if username.contains('\0') {
            return Err(Error::Nul("username".to_owned()));
        }
        if password.contains('\0') {
            return Err(Error::Nul("password".to_owned()));
        }
        let sdk = raw::sdk()?;
        let server_address = strings::cstring(server_address, "server address")?;
        let username = strings::cstring(username, "username")?;
        let password = strings::cstring(password, "password")?;
        let (identity_token, token_type) = raw::map_result(sdk.session_authenticate(
            self.raw(),
            server_address.as_ptr(),
            username.as_ptr(),
            password.as_ptr(),
        ))?;
        Ok(AuthenticateResult {
            identity_token,
            token_type: IdentityTokenType::from(token_type),
        })
    }

    /// Returns the session termination waitable event handle.
    pub fn termination_event(&self) -> Result<wslc_sys::HANDLE> {
        let sdk = raw::sdk()?;
        raw::map_result(sdk.session_termination_event(self.raw()))
    }

    /// Returns the session termination reason.
    pub fn termination_reason(&self) -> Result<SessionTerminationReason> {
        let sdk = raw::sdk()?;
        let reason = raw::map_result(sdk.session_termination_reason(self.raw()))?;
        Ok(reason.into())
    }

    /// Registers a crash dump callback for this session.
    pub fn on_crash_dump<F>(&self, callback: F) -> Result<CrashDumpSubscription>
    where
        F: FnMut(CrashDumpInfo) + Send + 'static,
    {
        let sdk = raw::sdk()?;
        raw::map_result(sdk.register_session_crash_dump_callback(self.raw(), callback))
    }

    /// Creates a container builder.
    pub fn container(&self, options: ContainerOptions) -> ContainerBuilder {
        ContainerBuilder::new(self.clone(), options)
    }

    /// Opens an existing container by name, full ID, or unambiguous ID prefix.
    pub fn open_container(&self, name_or_id: impl AsRef<str>) -> Result<Container> {
        let name_or_id = name_or_id.as_ref();
        if name_or_id.trim().is_empty() {
            return Err(Error::InvalidInput(
                "container name or id cannot be empty".to_owned(),
            ));
        }
        if name_or_id.contains('\0') {
            return Err(Error::Nul("container name or id".to_owned()));
        }

        let sdk = raw::sdk()?;
        let name_or_id = strings::cstring(name_or_id, "container name or id")?;
        let raw_container = raw::map_result(sdk.open_container(self.raw(), name_or_id.as_ptr()))?;
        let raw = NonNull::new(raw_container).ok_or_else(|| {
            Error::from_hresult(
                wslc_sys::S_OK,
                "WslcOpenContainer returned a null container",
            )
        })?;
        Ok(Container::from_opened(raw, self.inner.clone()))
    }

    /// Fetches logs for a container in this session.
    ///
    /// The public C SDK does not yet expose container logs, so this helper uses
    /// the `wslc container logs` CLI. Override the executable with `WSLC_CLI_PATH`
    /// when needed.
    pub fn logs(
        &self,
        container_id: impl AsRef<str>,
        options: LogsOptions,
    ) -> Result<ContainerLogs> {
        logs::fetch_container_logs(self.name(), container_id.as_ref(), &options)
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
        raw::map_result(sdk.create_session_vhd_volume(self.raw(), &raw_options))
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
        raw::map_result(sdk.delete_session_vhd_volume(self.raw(), name.as_ptr()))
    }

    /// Terminates the session.
    pub fn terminate(&self) -> Result<()> {
        let sdk = raw::sdk()?;
        raw::map_result(sdk.terminate_session(self.raw()))
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
        raw::map_result(sdk.init_session_settings(
            name.as_ptr(),
            storage_path.as_ptr(),
            &mut settings,
        ))?;

        if let Some(cpu_count) = self.cpu_count {
            raw::map_result(sdk.set_session_cpu_count(&mut settings, cpu_count))?;
        }
        if let Some(memory_mb) = self.memory_mb {
            raw::map_result(sdk.set_session_memory(&mut settings, memory_mb))?;
        }
        if let Some(timeout_ms) = self.timeout_ms {
            raw::map_result(sdk.set_session_timeout(&mut settings, timeout_ms))?;
        }
        if self.enable_gpu {
            raw::map_result(sdk.set_session_feature_flags(
                &mut settings,
                wslc_sys::WSLC_SESSION_FEATURE_FLAG_ENABLE_GPU,
            ))?;
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
            raw::map_result(sdk.set_session_vhd(&mut settings, &vhd_raw))?;
        }

        let raw_session = raw::map_result(sdk.create_session(&mut settings))?;
        let raw = NonNull::new(raw_session).ok_or_else(|| {
            Error::from_hresult(wslc_sys::S_OK, "WslcCreateSession returned a null session")
        })?;
        Ok(Session {
            inner: Rc::new(SessionInner {
                raw,
                name: self.name,
                terminate_on_drop: self.terminate_on_drop,
                _com: com,
            }),
        })
    }
}

impl Drop for SessionInner {
    fn drop(&mut self) {
        if let Ok(sdk) = raw::sdk() {
            if self.terminate_on_drop {
                let _ = sdk.terminate_session(self.raw.as_ptr());
            }
            let _ = sdk.release_session(self.raw.as_ptr());
        }
    }
}

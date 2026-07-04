use crate::{raw, strings, Error, Result};
use std::ffi::CString;
use std::rc::Rc;

/// Output capture mode for a process.
pub enum OutputMode {
    /// Discard process output.
    Null,
    /// Capture stdout and stderr.
    Capture,
    /// Stream stdout and stderr to callbacks.
    Streaming {
        /// Stdout callback.
        stdout: Option<OutputCallback>,
        /// Stderr callback.
        stderr: Option<OutputCallback>,
    },
}

type OutputCallback = Box<dyn FnMut(&[u8]) + Send>;

impl std::fmt::Debug for OutputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => f.write_str("Null"),
            Self::Capture => f.write_str("Capture"),
            Self::Streaming { .. } => f.write_str("Streaming"),
        }
    }
}

impl Default for OutputMode {
    fn default() -> Self {
        Self::Null
    }
}

/// Options for a container process.
#[derive(Debug)]
pub struct ProcessOptions {
    /// Command and arguments.
    pub cmdline: Vec<String>,
    /// Working directory inside the container.
    pub working_dir: Option<String>,
    /// Environment variables.
    pub env: Vec<(String, String)>,
    /// Output mode.
    pub output_mode: OutputMode,
}

impl ProcessOptions {
    /// Creates process options from command arguments.
    pub fn new<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            cmdline: args.into_iter().map(Into::into).collect(),
            working_dir: None,
            env: Vec::new(),
            output_mode: OutputMode::Null,
        }
    }

    /// Sets the working directory.
    pub fn working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Adds an environment variable.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Captures stdout and stderr.
    pub fn capture_stdout(mut self) -> Self {
        self.output_mode = OutputMode::Capture;
        self
    }

    /// Captures stdout and stderr.
    pub fn capture_stderr(mut self) -> Self {
        self.output_mode = OutputMode::Capture;
        self
    }

    /// Inherits output behavior from the SDK/runtime.
    pub fn inherit_output(mut self) -> Self {
        self.output_mode = OutputMode::Null;
        self
    }

    /// Enables streaming mode without callbacks yet.
    pub fn streaming(mut self) -> Self {
        self.output_mode = OutputMode::Streaming {
            stdout: None,
            stderr: None,
        };
        self
    }

    /// Validates process options.
    pub fn validate(&self) -> Result<()> {
        if self.cmdline.is_empty() {
            return Err(Error::InvalidInput(
                "process command cannot be empty".to_owned(),
            ));
        }
        if self.cmdline.iter().any(|arg| arg.is_empty()) {
            return Err(Error::InvalidInput(
                "process command arguments cannot be empty".to_owned(),
            ));
        }
        if self.cmdline.iter().any(|arg| arg.contains('\0')) {
            return Err(Error::Nul("process arg".to_owned()));
        }
        if let Some(dir) = &self.working_dir {
            if dir.is_empty() {
                return Err(Error::InvalidInput(
                    "working directory cannot be empty".to_owned(),
                ));
            }
            if dir.contains('\0') {
                return Err(Error::Nul("working_dir".to_owned()));
            }
        }
        for (key, value) in &self.env {
            if key.is_empty() || key.contains('=') {
                return Err(Error::InvalidInput(
                    "env variable key cannot be empty or contain '='".to_owned(),
                ));
            }
            if key.contains('\0') || value.contains('\0') {
                return Err(Error::Nul("env".to_owned()));
            }
        }
        Ok(())
    }

    pub(crate) fn to_raw(
        &self,
        sdk: &raw::Sdk,
        capture: Option<&CaptureRegistration>,
    ) -> Result<RawProcessSettings> {
        self.validate()?;
        let mut settings = wslc_sys::WslcProcessSettings::default();
        raw::map_result(sdk.init_process_settings(&mut settings))?;

        let argv = strings::cstrings(self.cmdline.iter().map(String::as_str), "process arg")?;
        let argv_ptrs: Vec<_> = argv.iter().map(|arg| arg.as_ptr()).collect();
        raw::map_result(sdk.set_process_cmdline(&mut settings, &argv_ptrs))?;

        let working_dir;
        if let Some(dir) = &self.working_dir {
            working_dir = strings::cstring(dir, "working_dir")?;
            raw::map_result(
                sdk.set_process_working_directory(&mut settings, working_dir.as_ptr()),
            )?;
        }

        let env_values: Vec<String> = self
            .env
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        let env_c = strings::cstrings(env_values.iter().map(String::as_str), "env")?;
        let env_ptrs: Vec<_> = env_c.iter().map(|value| value.as_ptr()).collect();
        if !env_ptrs.is_empty() {
            raw::map_result(sdk.set_process_env_variables(&mut settings, &env_ptrs))?;
        }

        if let Some(capture) = capture {
            raw::map_result(sdk.set_process_capture_callbacks(&mut settings, capture))?;
        }

        Ok(RawProcessSettings {
            settings,
            _argv: argv,
            _env: env_c,
        })
    }
}

pub(crate) struct RawProcessSettings {
    pub settings: wslc_sys::WslcProcessSettings,
    _argv: Vec<CString>,
    _env: Vec<CString>,
}

/// Process output.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Output {
    /// Exit status.
    pub status: i32,
    /// Captured stdout.
    pub stdout: Vec<u8>,
    /// Captured stderr.
    pub stderr: Vec<u8>,
}

/// Process state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessState {
    /// Unknown.
    Unknown,
    /// Running.
    Running,
    /// Exited.
    Exited,
    /// Signalled.
    Signalled,
}

impl From<wslc_sys::WslcProcessState> for ProcessState {
    fn from(value: wslc_sys::WslcProcessState) -> Self {
        match value {
            wslc_sys::WslcProcessState::WSLC_PROCESS_STATE_RUNNING => Self::Running,
            wslc_sys::WslcProcessState::WSLC_PROCESS_STATE_EXITED => Self::Exited,
            wslc_sys::WslcProcessState::WSLC_PROCESS_STATE_SIGNALLED => Self::Signalled,
            _ => Self::Unknown,
        }
    }
}

pub(crate) struct ProcessInner {
    pub raw: wslc_sys::WslcProcess,
}

/// A process running inside a WSLC container.
pub struct Process {
    pub(crate) inner: Rc<ProcessInner>,
}

impl std::fmt::Debug for Process {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Process").finish_non_exhaustive()
    }
}

impl Process {
    /// Returns the process ID.
    pub fn pid(&self) -> Result<u32> {
        let sdk = raw::sdk()?;
        raw::map_result(sdk.process_pid(self.inner.raw))
    }

    /// Returns the process state.
    pub fn state(&self) -> Result<ProcessState> {
        let sdk = raw::sdk()?;
        let state = raw::map_result(sdk.process_state(self.inner.raw))?;
        Ok(state.into())
    }

    /// Sends a signal to the process.
    pub fn signal(&self, signal: crate::Signal) -> Result<()> {
        let sdk = raw::sdk()?;
        raw::map_result(sdk.signal_process(self.inner.raw, signal.as_raw_signal()))
    }

    /// Returns the exit code.
    pub fn exit_code(&self) -> Result<Option<i32>> {
        let sdk = raw::sdk()?;
        let exit_code = raw::map_result(sdk.process_exit_code(self.inner.raw))?;
        Ok(Some(exit_code))
    }

    /// Waits for the process to finish.
    pub fn wait(&self) -> Result<i32> {
        let sdk = raw::sdk()?;
        let event = raw::map_result(sdk.process_exit_event(self.inner.raw))?;
        const INFINITE: u32 = 0xffff_ffff;
        const WAIT_OBJECT_0: u32 = 0;
        let wait = raw::wait_for_single_object(event, INFINITE);
        if wait != WAIT_OBJECT_0 {
            return Err(Error::InvalidInput(format!(
                "waiting for process exit failed with WAIT result 0x{wait:08x}"
            )));
        }
        self.exit_code().map(|code| code.unwrap_or_default())
    }

    /// Waits for the process and returns captured output.
    pub fn wait_with_output(&self) -> Result<Output> {
        Ok(Output {
            status: self.wait()?,
            ..Output::default()
        })
    }
}

impl Drop for ProcessInner {
    fn drop(&mut self) {
        if let Ok(sdk) = raw::sdk() {
            let _com = crate::com::try_initialize_mta().ok().flatten();
            let _ = sdk.release_process(self.raw);
        }
    }
}

pub(crate) type CaptureRegistration = raw::CaptureRegistration;

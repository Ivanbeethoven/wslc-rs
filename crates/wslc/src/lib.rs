//! Safe Rust wrapper for Microsoft WSL Containers.

mod container;
mod image;
mod logs;
mod process;
mod raw;
mod registry;
mod service;
mod session;
mod storage;
mod strings;

pub mod com;
pub mod error;

/// Version of the preview WSLC SDK API shape this crate was written against.
pub const TESTED_WSLC_SDK_VERSION: &str = "preview";

pub use container::{
    Container, ContainerBuilder, ContainerOptions, ContainerState, DeleteContainerOptions,
    NetworkingMode, Signal,
};
pub use error::WslcErrorKind;
pub use error::{Error, Result};
pub use image::{
    AuthenticateResult, IdentityTokenType, ImageInfo, ImageProgress, ImageProgressStatus,
    ImagePullOperation, ImagePullOptions, ImagePushOptions,
};
pub use logs::{ContainerLogs, LogsOptions};
pub use process::{
    CaptureRegistration, Output, OutputMode, Process, ProcessIo, ProcessOptions, ProcessState,
};
pub use service::{ComponentFlags, InstallProgress, Service, Version};
pub use session::{
    CrashDumpInfo, CrashDumpSubscription, Session, SessionBuilder, SessionTerminationReason,
};
pub use storage::{VhdOptions, VhdType};

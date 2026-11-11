//! The platform backends. Each target dispatches to one bridge; a target with none lands on
//! `unsupported`, which fakes no success.

#[cfg(target_os = "android")]
mod android;
#[cfg(target_vendor = "apple")]
mod apple;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(any(
    target_vendor = "apple",
    target_os = "android",
    target_os = "windows",
    target_os = "linux",
    target_arch = "wasm32"
)))]
mod unsupported;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_os = "windows")]
mod windows;

use std::sync::Arc;

use crate::backend::Backend;

/// The backend of the target this build runs on, dispatched in a fixed order: apple, android,
/// windows, linux, web, then `unsupported`.
pub(crate) fn default_backend() -> Arc<dyn Backend> {
    #[cfg(target_vendor = "apple")]
    return apple::backend();

    #[cfg(target_os = "android")]
    return android::backend();

    #[cfg(target_os = "windows")]
    return windows::backend();

    #[cfg(target_os = "linux")]
    return linux::backend();

    #[cfg(target_arch = "wasm32")]
    return web::backend();

    #[cfg(not(any(
        target_vendor = "apple",
        target_os = "android",
        target_os = "windows",
        target_os = "linux",
        target_arch = "wasm32"
    )))]
    return unsupported::backend();
}

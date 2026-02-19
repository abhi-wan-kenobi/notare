use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::{Mutex, MutexGuard};

use crate::error::{Error, Result};

pub struct Model {
    handle: NonNull<std::ffi::c_void>,
    inference_lock: Mutex<()>,
    cloud_api_key: Option<String>,
}

unsafe impl Send for Model {}
// SAFETY: All FFI methods that touch model state are serialized by `inference_lock`.
// The sole exception is `stop()`, which only sets a `std::atomic<bool>` on the C++ side.
unsafe impl Sync for Model {}

pub struct ModelBuilder {
    model_path: PathBuf,
    cloud_api_key: Option<String>,
}

impl ModelBuilder {
    pub fn cloud_api_key(mut self, key: impl Into<String>) -> Self {
        self.cloud_api_key = Some(key.into());
        self
    }

    pub fn build(self) -> Result<Model> {
        let path = CString::new(self.model_path.to_string_lossy().into_owned())?;
        let raw = unsafe { cactus_sys::cactus_init(path.as_ptr(), std::ptr::null(), false) };
        let handle =
            NonNull::new(raw).ok_or_else(|| Error::Init("cactus_init returned null".into()))?;

        Ok(Model {
            handle,
            inference_lock: Mutex::new(()),
            cloud_api_key: self.cloud_api_key,
        })
    }
}

impl Model {
    pub fn builder(model_path: impl AsRef<Path>) -> ModelBuilder {
        ModelBuilder {
            model_path: model_path.as_ref().to_path_buf(),
            cloud_api_key: None,
        }
    }

    pub fn new(model_path: impl AsRef<Path>) -> Result<Self> {
        Self::builder(model_path).build()
    }

    /// Cancel an in-progress inference. Safe to call concurrently — only sets an
    /// atomic flag on the C++ side.
    pub fn stop(&self) {
        unsafe {
            cactus_sys::cactus_stop(self.handle.as_ptr());
        }
    }

    pub fn reset(&mut self) {
        let _guard = self.lock_inference();
        unsafe {
            cactus_sys::cactus_reset(self.handle.as_ptr());
        }
    }

    pub(crate) fn lock_inference(&self) -> MutexGuard<'_, ()> {
        self.inference_lock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn raw_handle(&self) -> *mut std::ffi::c_void {
        self.handle.as_ptr()
    }

    /// Set `CACTUS_CLOUD_API_KEY` in the process environment if this model was
    /// configured with a cloud API key. Must be called while holding the
    /// `inference_lock` so the env write and the FFI read are atomic with
    /// respect to this model's call sequence.
    pub(crate) fn prepare_cloud_env(&self) {
        if let Some(key) = &self.cloud_api_key {
            // SAFETY: called under inference_lock; the C++ engine reads the env
            // var synchronously inside the same locked FFI call, so no other
            // thread can observe a partially-written value through this model.
            unsafe { std::env::set_var("CACTUS_CLOUD_API_KEY", key) };
        }
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        unsafe {
            cactus_sys::cactus_destroy(self.handle.as_ptr());
        }
    }
}

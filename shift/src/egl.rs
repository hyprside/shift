#![allow(non_camel_case_types)]
pub type khronos_utime_nanoseconds_t = u64;
pub type khronos_uint64_t = u64;
pub type khronos_ssize_t = isize;

pub type EGLint = i32;
pub type EGLNativeDisplayType = *mut std::ffi::c_void;
pub type EGLNativePixmapType = *mut std::ffi::c_void;
pub type EGLNativeWindowType = *mut std::ffi::c_void;
pub type NativeDisplayType = *mut std::ffi::c_void;
pub type NativePixmapType = *mut std::ffi::c_void;
pub type NativeWindowType = *mut std::ffi::c_void;

include!(concat!(env!("OUT_DIR"), "/egl_bindings.rs"));

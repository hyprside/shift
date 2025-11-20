use std::{
	ffi::{CStr, CString},
	os::raw::c_char,
};

use tab_protocol::DEFAULT_SOCKET_PATH;

use crate::TabClient;

/// C-friendly opaque handle.
#[repr(C)]
pub struct TabClientHandle {
	inner: TabClient,
}

/// Connect to a Tab socket and authenticate immediately. Returns NULL on failure.
#[unsafe(no_mangle)]
pub extern "C" fn tab_client_connect(
	socket_path: *const c_char,
	token: *const c_char,
) -> *mut TabClientHandle {
	let path = unsafe {
		if socket_path.is_null() {
			DEFAULT_SOCKET_PATH.to_string()
		} else {
			match CStr::from_ptr(socket_path).to_str() {
				Ok(s) => s.to_string(),
				Err(_) => return std::ptr::null_mut(),
			}
		}
	};

	if token.is_null() {
		return std::ptr::null_mut();
	}
	let token = match unsafe { CStr::from_ptr(token) }.to_str() {
		Ok(s) => s.to_string(),
		Err(_) => return std::ptr::null_mut(),
	};

	match TabClient::connect(path, token) {
		Ok(client) => Box::into_raw(Box::new(TabClientHandle { inner: client })),
		Err(_) => std::ptr::null_mut(),
	}
}

/// Disconnect and free the handle.
#[unsafe(no_mangle)]
pub extern "C" fn tab_client_disconnect(handle: *mut TabClientHandle) {
	if handle.is_null() {
		return;
	}
	unsafe {
		drop(Box::from_raw(handle));
	}
}
macro_rules! to_cstr {
	($s:expr) => {
		CString::new($s)
			.map(CString::into_raw)
			.unwrap_or(std::ptr::null_mut())
	};
}
macro_rules! unwrap_handle {
	($client:expr) => {{
		let Some(client) = (unsafe { $client.as_mut() }) else {
			panic!("NullPointerException: tab client cannot be a null pointer");
		};
		&mut client.inner
	}};
}
/// Retrieve and clear the last error as an owned C string. Caller must free via `tab_client_string_free`.
#[unsafe(no_mangle)]
pub extern "C" fn tab_client_get_protocol_name(handle: *mut TabClientHandle) -> *mut c_char {
	let client = unwrap_handle!(handle);
	to_cstr!(client.hello.protocol.as_str())
}
/// Retrieve and clear the last error as an owned C string. Caller must free via `tab_client_string_free`.
#[unsafe(no_mangle)]
pub extern "C" fn tab_client_get_server_name(handle: *mut TabClientHandle) -> *mut c_char {
	let client = unwrap_handle!(handle);
	to_cstr!(client.hello.server.as_str())
}
/// Retrieve and clear the last error as an owned C string. Caller must free via `tab_client_string_free`.
#[unsafe(no_mangle)]
pub extern "C" fn tab_client_take_error(handle: *mut TabClientHandle) -> *mut c_char {
	let client = unwrap_handle!(handle);
	let err = client.last_error.take();
	match err {
		Some(msg) => to_cstr!(msg),
		None => 0 as _,
	}
}

/// Retrieve the authenticated session JSON blob (caller must free with `tab_client_string_free`).
#[unsafe(no_mangle)]
pub extern "C" fn tab_client_get_session_json(handle: *mut TabClientHandle) -> *mut c_char {
	let client = unwrap_handle!(handle);
	match client.session() {
		Some(session) => match serde_json::to_string(session) {
			Ok(json) => to_cstr!(json),
			Err(err) => {
				client.record_error(err.to_string());
				std::ptr::null_mut()
			}
		},
		None => std::ptr::null_mut(),
	}
}

/// Notify Shift that this session is ready to present.
#[unsafe(no_mangle)]
pub extern "C" fn tab_client_send_ready(handle: *mut TabClientHandle) -> bool {
	let client = unwrap_handle!(handle);
	match client.send_ready() {
		Ok(_) => true,
		Err(err) => {
			client.record_error(err.to_string());
			false
		}
	}
}

/// Free a string returned by `tab_client_take_error`.
#[unsafe(no_mangle)]
pub extern "C" fn tab_client_string_free(s: *mut c_char) {
	if s.is_null() {
		return;
	}
	unsafe {
		let _ = CString::from_raw(s);
	}
}

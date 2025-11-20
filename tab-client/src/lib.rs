//! Tab client-side helper crate.
//! - Rust API for connecting to Shift via Tab v1
//! - C ABI surface for C/C++ consumers (cdylib/staticlib)
//! FD passing is not yet abstracted; consumers can access the raw UnixStream.

use std::io::Read;
use std::os::unix::net::UnixStream;
use std::path::Path;

use tab_protocol::{
	AuthOkPayload, AuthPayload, DEFAULT_SOCKET_PATH, HelloPayload, PROTOCOL_VERSION, ProtocolError,
	SessionInfo, SessionReadyPayload, TabMessage, TabMessageFrame, message_header,
};

/// Client-side error wrapper.
#[derive(Debug, thiserror::Error)]
pub enum TabClientError {
	#[error("io error: {0}")]
	Io(#[from] std::io::Error),
	#[error("protocol error: {0}")]
	Protocol(#[from] ProtocolError),
	#[error("utf-8 error: {0}")]
	Utf8(#[from] std::str::Utf8Error),
	#[error("unexpected message: {0} (expected hello)")]
	UnexpectedHeader(String),
	#[error("serde error: {0}")]
	Serde(#[from] serde_json::Error),
	#[error("unsupported protocol: {0}")]
	UnsupportedProtocol(String),
	#[error("authentication rejected: {0}")]
	AuthRejected(String),
	#[error("unexpected message during auth: {0:?}")]
	UnexpectedMessage(TabMessage),
	#[error("client is not authenticated yet")]
	NotAuthenticated,
}

/// Rust-oriented Tab client.
pub struct TabClient {
	stream: UnixStream,
	read_buffer: Vec<u8>,
	last_error: Option<String>,
	hello: HelloPayload,
	session: Option<SessionInfo>,
}

impl TabClient {
	/// Connect to a Tab socket at an explicit path.
	pub fn connect<P: AsRef<Path>, S: Into<String>>(
		path: P,
		token: S,
	) -> Result<Self, TabClientError> {
		let stream = UnixStream::connect(path)?;
		let hello_msg = TabMessageFrame::read_framed(&stream)?;
		let parsed = TabMessage::parse_message_frame(hello_msg)?;
		let hello = match parsed {
			TabMessage::Hello(p) => p,
			other => return Err(TabClientError::UnexpectedHeader(format!("{:?}", other))),
		};

		if hello.protocol != PROTOCOL_VERSION {
			return Err(TabClientError::UnsupportedProtocol(hello.protocol));
		}

		let mut this = Self {
			stream,
			read_buffer: Vec::new(),
			last_error: None,
			hello,
			session: None,
		};
		this.authenticate(token)?;
		Ok(this)
	}

	/// Connect to the default `/tmp/shift.sock` socket.
	pub fn connect_default(token: impl Into<String>) -> Result<Self, TabClientError> {
		Self::connect(DEFAULT_SOCKET_PATH, token)
	}

	/// Send a framed Tab message.
	pub fn send(&mut self, msg: &TabMessageFrame) -> Result<(), TabClientError> {
		msg.encode_and_send(&self.stream)?;
		Ok(())
	}

	/// Receive a parsed Tab message (blocking).
	pub fn receive(&mut self) -> Result<TabMessage, TabClientError> {
		let frame = self.read_frame_blocking()?;
		Ok(TabMessage::parse_message_frame(frame)?)
	}

	/// Borrow the underlying socket (for FD passing or poll integration).
	pub fn stream(&self) -> &UnixStream {
		&self.stream
	}

	/// Borrow the underlying socket mutably.
	pub fn stream_mut(&mut self) -> &mut UnixStream {
		&mut self.stream
	}

	/// Access the received `hello` payload.
	pub fn hello(&self) -> &HelloPayload {
		&self.hello
	}

	pub fn authenticate(&mut self, token: impl Into<String>) -> Result<SessionInfo, TabClientError> {
		let token = token.into();
		let frame = TabMessageFrame::json(message_header::AUTH, AuthPayload { token });
		self.send(&frame)?;
		loop {
			match self.receive()? {
				TabMessage::AuthOk(AuthOkPayload { session }) => {
					self.session = Some(session.clone());
					return Ok(session);
				}
				TabMessage::AuthError(payload) => {
					return Err(TabClientError::AuthRejected(payload.error));
				}
				TabMessage::Error(payload) => {
					let msg = payload.message.unwrap_or_else(|| payload.code);
					return Err(TabClientError::AuthRejected(msg));
				}
				other => return Err(TabClientError::UnexpectedMessage(other)),
			}
		}
	}

	pub fn session(&self) -> Option<&SessionInfo> {
		self.session.as_ref()
	}

	pub fn send_ready(&mut self) -> Result<(), TabClientError> {
		let session = self
			.session
			.as_ref()
			.ok_or(TabClientError::NotAuthenticated)?;
		let payload = SessionReadyPayload {
			session_id: session.id.clone(),
		};
		let frame = TabMessageFrame::json(message_header::SESSION_READY, payload);
		self.send(&frame)
	}

	pub(crate) fn record_error(&mut self, err: impl ToString) {
		self.last_error = Some(err.to_string());
	}

	fn read_frame_blocking(&mut self) -> Result<TabMessageFrame, ProtocolError> {
		loop {
			if let Some(frame) = self.try_parse_buffered_frame()? {
				return Ok(frame);
			}
			self.read_more()?;
		}
	}

	fn try_parse_buffered_frame(&mut self) -> Result<Option<TabMessageFrame>, ProtocolError> {
		if self.read_buffer.is_empty() {
			return Ok(None);
		}
		match TabMessageFrame::parse_from_bytes(&self.read_buffer, Vec::new())? {
			Some((frame, consumed)) => {
				self.read_buffer.drain(..consumed);
				Ok(Some(frame))
			}
			None => Ok(None),
		}
	}

	fn read_more(&mut self) -> Result<(), ProtocolError> {
		let mut buf = [0u8; 4096];
		let bytes = self.stream.read(&mut buf)?;
		if bytes == 0 {
			return Err(ProtocolError::UnexpectedEof);
		}
		self.read_buffer.extend_from_slice(&buf[..bytes]);
		Ok(())
	}
}
mod c_bindings;

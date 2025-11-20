use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::client::{Client, ServerEvent};
use crate::connection::TabConnection;
use crate::session::SessionRegistry;
use tab_protocol::{
	DEFAULT_SOCKET_PATH, FramebufferLinkPayload, ProtocolError, SessionInfo, SessionRole,
	SessionStatePayload, TabMessageFrame, message_header,
};
use tracing::warn;

/// Server-side error type.
#[derive(Debug, thiserror::Error)]
pub enum TabServerError {
	#[error("io error: {0}")]
	Io(#[from] std::io::Error),
	#[error("protocol error: {0}")]
	Protocol(#[from] ProtocolError),
	#[error("texture load error: {0}")]
	Texture(String),
}

type LoaderFn<Texture> =
	Arc<dyn Fn(RawFd, &FramebufferLinkPayload) -> Result<Texture, TabServerError> + Send + Sync>;

/// Headless Tab protocol server that coordinates client sessions.
pub struct TabServer<Texture> {
	path: PathBuf,
	listener: UnixListener,
	clients: Vec<Client<Texture>>,
	sessions: SessionRegistry,
	load_dmabuf: LoaderFn<Texture>,
	monitor_textures: HashMap<String, Texture>,
}

impl<Texture> TabServer<Texture> {
	/// Create and bind a Tab server socket, cleaning up any stale path.
	pub fn bind(
		path: impl AsRef<Path>,
		load_dmabuf: impl Fn(RawFd, &FramebufferLinkPayload) -> Result<Texture, TabServerError>
		+ Send
		+ Sync
		+ 'static,
	) -> Result<Self, TabServerError> {
		let path = path.as_ref();
		if path.exists() {
			fs::remove_file(path)?;
		}
		let listener = UnixListener::bind(path)?;
		listener.set_nonblocking(true)?;
		Ok(Self {
			path: path.to_path_buf(),
			listener,
			clients: Vec::new(),
			sessions: SessionRegistry::new(),
			load_dmabuf: Arc::new(load_dmabuf),
			monitor_textures: HashMap::new(),
		})
	}

	/// Convenience helper to bind to the default `/tmp/shift.sock`.
	pub fn bind_default(
		load_dmabuf: impl Fn(RawFd, &FramebufferLinkPayload) -> Result<Texture, TabServerError>
		+ Send
		+ Sync
		+ 'static,
	) -> Result<Self, TabServerError> {
		Self::bind(DEFAULT_SOCKET_PATH, load_dmabuf)
	}

	/// Drive acceptance and message processing without blocking.
	pub fn pump(&mut self) -> Result<(), TabServerError> {
		self.accept_new_clients()?;
		self.process_clients()?;
		Ok(())
	}

	/// Register a pending session/token pair waiting for a client connection.
	pub fn register_session(
		&mut self,
		session_id: impl Into<String>,
		token: impl Into<String>,
		role: SessionRole,
		display_name: Option<String>,
	) {
		self
			.sessions
			.insert_pending(session_id.into(), token.into(), role, display_name);
	}

	/// Raw file descriptor for the listening socket (for poll integration).
	pub fn listener_fd(&self) -> RawFd {
		self.listener.as_raw_fd()
	}

	/// Raw file descriptors for all connected clients (for poll integration).
	pub fn client_fds(&self) -> impl Iterator<Item = RawFd> + '_ {
		self.clients.iter().map(|c| c.connection.as_raw_fd())
	}

	/// Immutable view of connected clients.
	pub fn clients(&self) -> impl Iterator<Item = &Client<Texture>> {
		self.clients.iter()
	}

	/// Path currently bound by this server.
	pub fn path(&self) -> &Path {
		&self.path
	}

	pub fn monitor_texture(&self, monitor_id: &str) -> Option<&Texture> {
		self.monitor_textures.get(monitor_id)
	}

	fn dispatch_event(&mut self, event: ServerEvent<Texture>) {
		match event {
			ServerEvent::SessionState {
				session,
				exclude_client_id,
			} => self.broadcast_session_state(session, exclude_client_id),
			ServerEvent::FramebufferLinked {
				monitor_id,
				texture,
			} => {
				self.monitor_textures.insert(monitor_id, texture);
			}
		}
	}

	fn broadcast_session_state(&mut self, session: SessionInfo, exclude_client_id: Option<String>) {
		let frame = TabMessageFrame::json(
			message_header::SESSION_STATE,
			SessionStatePayload {
				session: session.clone(),
			},
		);
		for client in self.clients.iter_mut() {
			if client.session.role == Some(SessionRole::Admin) {
				if exclude_client_id
					.as_ref()
					.is_some_and(|skip| skip == &client.id)
				{
					continue;
				}
				if let Err(err) = client.connection.send_frame(&frame) {
					warn!(client_id = %client.id, %err, "Failed to send session_state");
				}
			}
		}
	}

	fn accept_new_clients(&mut self) -> Result<(), TabServerError> {
		loop {
			match self.listener.accept() {
				Ok((stream, _)) => {
					let mut connection = TabConnection::new(stream)?;
					connection.send_hello("Shift dev")?;
					let client = Client::new(connection, Arc::clone(&self.load_dmabuf));
					self.clients.push(client);
				}
				Err(e) if e.kind() == ErrorKind::WouldBlock => break,
				Err(e) => return Err(TabServerError::Io(e)),
			}
		}
		Ok(())
	}

	fn process_clients(&mut self) -> Result<(), TabServerError> {
		let mut idx = 0;
		while idx < self.clients.len() {
			let mut remove = false;
			match self.clients[idx].connection.read_message_nonblocking() {
				Ok(Some(msg)) => {
					let events = self.clients[idx].handle_message(msg, &mut self.sessions);
					for event in events {
						self.dispatch_event(event);
					}
				}
				Ok(None) => {}
				Err(ProtocolError::WouldBlock) => {}
				Err(err) => {
					warn!(
						client_id = %self.clients[idx].id,
						%err,
						"Dropping client due to protocol error"
					);
					remove = true;
				}
			}

			if remove {
				if let Some(session_id) = self.clients[idx].session.session_id.clone() {
					if let Some(info) = self.sessions.mark_consumed(&session_id) {
						self.dispatch_event(ServerEvent::SessionState {
							session: info,
							exclude_client_id: None,
						});
					}
				}
				self.clients.swap_remove(idx);
			} else {
				idx += 1;
			}
		}
		Ok(())
	}
}

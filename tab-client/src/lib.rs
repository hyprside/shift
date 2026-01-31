//! Tab client rewrite crate.

mod config;
mod error;
mod events;
mod gbm_allocator;
mod monitor;
mod swapchain;
mod c_bindings;

pub use config::TabClientConfig;
pub use error::TabClientError;
pub use events::{MonitorEvent, RenderEvent};
pub use monitor::{MonitorId, MonitorState};
pub use swapchain::{TabBuffer, TabSwapchain};

use std::collections::HashMap;
use std::os::{
	fd::{AsRawFd, RawFd},
	unix::net::UnixStream,
};
use std::sync::Arc;

use tab_protocol::message_frame::{TabMessageFrame, TabMessageFrameReader};
use tab_protocol::message_header;
use tab_protocol::{
	AuthErrorPayload, AuthOkPayload, AuthPayload, BufferIndex, FrameDonePayload, MonitorInfo,
	SessionInfo, TabMessage,
};

use crate::gbm_allocator::GbmAllocator;

/// Primary synchronous Tab client handle.
pub struct TabClient {
	socket: UnixStream,
	reader: TabMessageFrameReader,
	session: SessionInfo,
	monitors: HashMap<MonitorId, MonitorState>,
	monitor_listeners: Vec<Arc<dyn Fn(&MonitorEvent) + Send + Sync>>,
	render_listeners: Vec<Arc<dyn Fn(&RenderEvent) + Send + Sync>>,
	gbm: GbmAllocator,
}

impl TabClient {
	pub fn connect(config: TabClientConfig) -> Result<Self, TabClientError> {
		let socket = tab_protocol::unix_socket_utils::connect_seqpacket(config.socket_path_ref())?;
		let mut reader = TabMessageFrameReader::new();
		let hello = Self::read_message(&socket, &mut reader)?;
		let TabMessage::Hello(payload) = hello else {
			return Err(TabClientError::Unexpected("expected hello"));
		};
		if payload.protocol != tab_protocol::PROTOCOL_VERSION {
			return Err(TabClientError::Unexpected("protocol mismatch"));
		}
		let auth_frame = TabMessageFrame::json(
			message_header::AUTH,
			AuthPayload {
				token: config.token().to_string(),
			},
		);
		auth_frame.encode_and_send(&socket)?;
		let auth_ok = Self::wait_for_auth(&socket, &mut reader)?;
		let monitors = auth_ok
			.monitors
			.into_iter()
			.map(|info| (info.id.clone(), MonitorState::new(info)))
			.collect();
		let gbm = GbmAllocator::new(config.render_node_path())?;
		socket.set_nonblocking(true)?;
		Ok(Self {
			socket,
			reader,
			session: auth_ok.session,
			monitors,
			monitor_listeners: Vec::new(),
			render_listeners: Vec::new(),
			gbm,
		})
	}

	pub fn session(&self) -> &SessionInfo {
		&self.session
	}

	pub fn monitors(&self) -> impl Iterator<Item = &MonitorState> {
		self.monitors.values()
	}

	pub fn monitor(&self, id: &str) -> Option<&MonitorState> {
		self.monitors.get(id)
	}

	pub fn socket_fd(&self) -> RawFd {
		self.socket.as_raw_fd()
	}

	pub fn poll_fds(&self) -> [RawFd; 2] {
		[self.socket.as_raw_fd(), self.drm_fd()]
	}

	pub fn drm_fd(&self) -> RawFd {
		self.gbm.drm_fd()
	}

	pub fn create_swapchain(&self, monitor_id: &str) -> Result<TabSwapchain, TabClientError> {
		let monitor = self
			.monitors
			.get(monitor_id)
			.ok_or_else(|| TabClientError::UnknownMonitor(monitor_id.to_string()))?;
		let swapchain = self.gbm.create_swapchain(monitor)?;
		self.framebuffer_link(&swapchain)?;
		Ok(swapchain)
	}

	pub fn framebuffer_link(&self, swapchain: &TabSwapchain) -> Result<(), TabClientError> {
		let payload = swapchain.framebuffer_link_payload();
		let mut frame = TabMessageFrame::json(message_header::FRAMEBUFFER_LINK, payload);
		let fds = swapchain.export_fds()?;
		frame.fds = fds.iter().map(|fd| fd.as_raw_fd()).collect();
		frame.encode_and_send(&self.socket)?;
		drop(fds);
		Ok(())
	}

	pub fn swap_buffers(&self, monitor_id: &str, buffer: BufferIndex) -> Result<(), TabClientError> {
		let payload = format!("{monitor_id} {}", buffer as u8);
		let frame = TabMessageFrame {
			header: message_header::SWAP_BUFFERS.into(),
			payload: Some(payload),
			fds: Vec::new(),
		};
		frame.encode_and_send(&self.socket)?;
		Ok(())
	}

	pub fn on_monitor_event<F>(&mut self, listener: F)
	where
		F: Fn(&MonitorEvent) + Send + Sync + 'static,
	{
		self.monitor_listeners.push(Arc::new(listener));
	}

	pub fn on_render_event<F>(&mut self, listener: F)
	where
		F: Fn(&RenderEvent) + Send + Sync + 'static,
	{
		self.render_listeners.push(Arc::new(listener));
	}

	pub fn dispatch_events(&mut self) -> Result<(), TabClientError> {
		loop {
			match self.reader.read_framed(&self.socket) {
				Ok(frame) => {
					let message = TabMessage::try_from(frame)?;
					self.handle_message(message)?;
				}
				Err(tab_protocol::ProtocolError::WouldBlock) => break,
				Err(other) => return Err(other.into()),
			}
		}
		Ok(())
	}

	fn read_message(
		socket: &UnixStream,
		reader: &mut TabMessageFrameReader,
	) -> Result<TabMessage, TabClientError> {
		let frame = reader.read_framed(socket)?;
		Ok(TabMessage::try_from(frame)?)
	}

	fn wait_for_auth(
		socket: &UnixStream,
		reader: &mut TabMessageFrameReader,
	) -> Result<AuthOkPayload, TabClientError> {
		loop {
			match Self::read_message(socket, reader)? {
				TabMessage::AuthOk(payload) => return Ok(payload),
				TabMessage::AuthError(AuthErrorPayload { error }) => {
					return Err(TabClientError::Auth(error));
				}
				other => {
					return Err(TabClientError::Unexpected(match other {
						TabMessage::Hello(_) => "duplicate hello",
						TabMessage::Auth(_) => "unexpected auth from server",
						_ => "unexpected pre-auth message",
					}));
				}
			}
		}
	}

	fn handle_message(&mut self, message: TabMessage) -> Result<(), TabClientError> {
		match message {
			TabMessage::MonitorAdded(payload) => {
				self.handle_monitor_added(payload.monitor);
			}
			TabMessage::MonitorRemoved(payload) => {
				self.handle_monitor_removed(payload.monitor_id);
			}
			TabMessage::FrameDone(payload) => {
				self.handle_frame_done(payload);
			}
			_ => {}
		}
		Ok(())
	}

	fn handle_monitor_added(&mut self, info: MonitorInfo) {
		let state = MonitorState::new(info);
		self.monitors.insert(state.info.id.clone(), state.clone());
		let event = MonitorEvent::Added(state);
		for listener in &self.monitor_listeners {
			listener(&event);
		}
	}

	fn handle_monitor_removed(&mut self, monitor_id: String) {
		self.monitors.remove(&monitor_id);
		let event = MonitorEvent::Removed(monitor_id);
		for listener in &self.monitor_listeners {
			listener(&event);
		}
	}

	fn handle_frame_done(&mut self, payload: FrameDonePayload) {
		let event = RenderEvent::FrameDone {
			monitor_id: payload.monitor_id,
		};
		for listener in &self.render_listeners {
			listener(&event);
		}
	}
}

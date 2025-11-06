use std::cell::RefCell;
use std::os::fd::RawFd;
use std::process::{Child, Command, Stdio};
use std::rc::{Rc, Weak};

use easydrm::EasyDRM;
use tab_protocol::SessionRole;
use tab_server::{TabServer, TabServerError, generate_id};
use tracing::info;

use crate::dma_buf_importer::ExternalTexture;
use crate::error::{FrameAck, ShiftError};
use crate::output::OutputContext;
use crate::presenter::FramePresenter;

pub struct ShiftApp {
	easydrm: Rc<RefCell<EasyDRM<OutputContext>>>,
	server: TabServer<ExternalTexture>,
	_admin_child: Child,
	frame_presenter: FramePresenter,
}

impl ShiftApp {
	pub fn new() -> Result<Self, ShiftError> {
		let easydrm = Rc::new(RefCell::new(EasyDRM::init(OutputContext::new)?));
		let frame_presenter = FramePresenter::new();
		let mut server = Self::bind_server(&easydrm)?;
		let _admin_child = Self::spawn_admin(&mut server)?;
		server.ensure_monitors_are_up_to_date_with_easydrm(&mut *easydrm.borrow_mut());
		Ok(Self {
			easydrm,
			server,
			_admin_child,
			frame_presenter,
		})
	}

	fn bind_server(
		easydrm: &Rc<RefCell<EasyDRM<OutputContext>>>,
	) -> Result<TabServer<ExternalTexture>, ShiftError> {
		let loader: Weak<RefCell<_>> = Rc::downgrade(easydrm);
		let server = TabServer::bind_default(move |fd: RawFd, info| {
			let Some(edrm_rc) = loader.upgrade() else {
				return Err(TabServerError::Texture(
					"EasyDRM no longer available".into(),
				));
			};
			let mut edrm = edrm_rc.borrow_mut();
			let Some(monitor) = edrm.monitors_mut().find(|m| {
				m.context()
					.monitor_id()
					.is_some_and(|id| id == info.monitor_id.as_str())
			}) else {
				return Err(TabServerError::Texture(format!(
					"No easydrm monitor for `{}`",
					info.monitor_id
				)));
			};
			monitor
				.make_current()
				.map_err(|e| TabServerError::Texture(e.to_string()))?;
			unsafe {
				crate::dma_buf_importer::ExternalTexture::import(
					monitor.gl(),
					&monitor.context().egl,
					fd,
					info,
				)
				.map_err(|e| TabServerError::Texture(e.to_string()))
			}
		})?;
		Ok(server)
	}

	fn spawn_admin(server: &mut TabServer<ExternalTexture>) -> Result<Child, ShiftError> {
		let admin_session_id = generate_id("ses");
		let admin_token = generate_id("adm");
		server.register_session(
			admin_session_id.clone(),
			admin_token.clone(),
			SessionRole::Admin,
			Some("admin".to_string()),
		);
		info!("listening on {}", server.path().display());
		info!(session_id = %admin_session_id, token = %admin_token, "admin session pending");
		let admin_client_bin = std::env::var("SHIFT_ADMIN_CLIENT_BIN")?;
		let mut cmd = Command::new(admin_client_bin);
		cmd.env("SHIFT_SESSION_TOKEN", admin_token);
		cmd.stdout(Stdio::inherit());
		cmd.stderr(Stdio::inherit());
		let child = cmd.spawn()?;
		info!(pid = child.id(), "spawned admin client");
		Ok(child)
	}

	pub fn run(&mut self) -> Result<(), ShiftError> {
		loop {
			self.pump_once()?;
		}
	}

	fn pump_once(&mut self) -> Result<(), ShiftError> {
		let snapshot = self.server.render_snapshot();
		let frame_pairs = {
			let mut edrm = self.easydrm.borrow_mut();
			let rendered = self.frame_presenter.render(&snapshot, &mut edrm)?;
			edrm.swap_buffers()?;
			let poll_fds = self.server.poll_fds();
			edrm.poll_events_ex(poll_fds)?;
			rendered
		};
		self
			.server
			.ensure_monitors_are_up_to_date_with_easydrm(&mut *self.easydrm.borrow_mut());
		self.notify_frames(&frame_pairs);
		self.server.pump()?;
		Ok(())
	}

	fn notify_frames(&mut self, frames: &FrameAck) {
		let ack_iter = frames.iter().map(|(m, s)| (m.as_str(), s.as_str()));
		self.server.notify_frame_rendered(ack_iter);
	}
}

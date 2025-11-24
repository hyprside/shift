use easydrm::MonitorContextCreationRequest;

use crate::egl::Egl;
use crate::renderer::MonitorRenderer;
use tab_server::MonitorIdStorage;

pub struct OutputContext {
	monitor_id: Option<String>,
	pub egl: Egl,
	pub renderer: MonitorRenderer,
	pending_sessions: Vec<String>,
}

impl OutputContext {
	pub fn new(request: &MonitorContextCreationRequest<'_>) -> Self {
		let egl = Egl::load_with(request.get_proc_address);
		let renderer = MonitorRenderer::new(request.gl).expect("failed to initialize renderer");
		Self {
			monitor_id: None,
			egl,
			renderer,
			pending_sessions: Vec::new(),
		}
	}
	pub fn monitor_id(&self) -> Option<&str> {
		self.monitor_id.as_deref()
	}

	pub fn take_pending_sessions(&mut self) -> Vec<String> {
		std::mem::take(&mut self.pending_sessions)
	}

	pub fn set_pending_sessions(&mut self, sessions: Vec<String>) {
		self.pending_sessions = sessions;
	}
}

impl MonitorIdStorage for OutputContext {
	fn monitor_id(&self) -> Option<&str> {
		self.monitor_id()
	}

	fn set_monitor_id(&mut self, id: String) {
		self.monitor_id = Some(id);
	}
}

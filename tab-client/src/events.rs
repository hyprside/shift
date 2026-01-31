use crate::MonitorState;

/// Monitor lifecycle event emitted to listeners.
#[derive(Debug, Clone)]
pub enum MonitorEvent {
	Added(MonitorState),
	Removed(String),
}

/// Rendering-related notifications.
#[derive(Debug, Clone)]
pub enum RenderEvent {
	FrameDone { monitor_id: String },
}

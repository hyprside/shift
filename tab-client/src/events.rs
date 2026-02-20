use crate::MonitorState;
use tab_protocol::BufferIndex;

/// Monitor lifecycle event emitted to listeners.
#[derive(Debug, Clone)]
pub enum MonitorEvent {
	Added(MonitorState),
	Removed(String),
}

/// Rendering-related notifications.
#[derive(Debug, Clone)]
pub enum RenderEvent {
	BufferReleased {
		monitor_id: String,
		buffer: BufferIndex,
	},
}

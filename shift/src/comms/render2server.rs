use std::sync::Arc;

use crate::monitor::MonitorId;

/// Events emitted by the rendering layer back into the server core.
#[derive(Debug)]
pub enum RenderEvt {
    /// A monitor became available for rendering.
    MonitorOnline {
        monitor_id: MonitorId,
    },
    /// A monitor was disconnected or became unusable.
    MonitorOffline {
        monitor_id: MonitorId,
    },
    /// Rendering reported an unrecoverable condition.
    FatalError {
        reason: Arc<str>,
    },
}

pub type RenderEvtRx = tokio::sync::mpsc::Receiver<RenderEvt>;
pub type RenderEvtTx = tokio::sync::mpsc::Sender<RenderEvt>;
pub type RenderEvtWeakTx = tokio::sync::mpsc::WeakSender<RenderEvt>;

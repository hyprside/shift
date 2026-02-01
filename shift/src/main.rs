use std::path::PathBuf;

use crate::{
	rendering_layer::{RenderingLayer, channels::Channels as RenderChannels},
	server_layer::ShiftServer,
};

mod auth;
mod client_layer;
mod comms;
mod ids;
mod monitor;
mod rendering_layer;
mod server_layer;
mod sessions;
#[tokio::main]
async fn main() {
	// ---- logging/tracing ----
	tracing_subscriber::fmt()
		.with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".to_string()))
		.init();

	// ---- socket path ----
	let socket_path = std::env::var_os("SHIFT_SOCKET")
		.map(PathBuf::from)
		.unwrap_or_else(|| "/tmp/shift.sock".into());

	// ---- create inter-layer channels ----
	let render_channels = RenderChannels::new();
	let (server_render_channels, rendering_render_channels) = render_channels.split();

	// ---- create server ----
	let mut server = match ShiftServer::bind(&socket_path, server_render_channels).await {
		Ok(s) => s,
		Err(e) => {
			tracing::error!("failed to bind ShiftServer at {:?}: {e}", socket_path);
			return;
		}
	};
	let token = server.add_initial_session().to_base64url();
	std::fs::write("/home/tiago/Desktop/shift/shift/token", token).unwrap();
	tracing::info!("starting ShiftServer on {:?}", socket_path);

	// ---- create rendering ----
	let rendering = match RenderingLayer::init(rendering_render_channels) {
		Ok(r) => r,
		Err(e) => {
			tracing::error!("failed to init rendering layer: {e}");
			return;
		}
	};
	let result = tokio::join!(server.start(), rendering.run());
	if let Err(e) = result.1 {
		tracing::error!("rendering thread ended with error: {e}");
	}
}

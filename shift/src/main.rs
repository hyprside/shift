use std::env;
use std::os::fd::RawFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use nix::unistd::close;
use tab_protocol::{FramebufferLinkPayload, SessionRole};
use tab_server::{TabServer, TabServerError, generate_id};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

fn main() {
	init_tracing();
	if let Err(err) = run() {
		error!(error = %err, "Shift daemon crashed");
	}
}

fn init_tracing() {
	let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
	tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
	let mut server = TabServer::bind_default(|fd: RawFd, _info: &FramebufferLinkPayload| {
		close(fd).map_err(|e| TabServerError::Texture(format!("{e}")))?;
		Ok(())
	})?;
	let admin_session_id = generate_id("ses");
	let admin_token = generate_id("adm");
	server.register_session(
		admin_session_id.clone(),
		admin_token.clone(),
		SessionRole::Admin,
		Some("admin-root".to_string()),
	);
	info!("listening on {}", server.path().display());
	info!(
		session_id = %admin_session_id,
		token = %admin_token,
		"admin session pending"
	);

	let admin_client_bin = admin_client_binary_path();
	let mut admin_cmd = Command::new(admin_client_bin);
	admin_cmd.env("SHIFT_ADMIN_TOKEN", admin_token);
	admin_cmd.stdout(Stdio::inherit());
	admin_cmd.stderr(Stdio::inherit());
	let child = admin_cmd.spawn()?;
	info!(pid = child.id(), "spawned admin client");
	let _admin_child = child;

	loop {
		server.pump()?;
		thread::sleep(Duration::from_millis(16));
	}
}

fn admin_client_binary_path() -> PathBuf {
	if let Ok(path) = env::var("SHIFT_ADMIN_CLIENT_BIN") {
		return PathBuf::from(path);
	}

	let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.expect("workspace root")
		.to_path_buf();
	workspace_root
		.join("target")
		.join("debug")
		.join("examples")
		.join("admin_client")
}

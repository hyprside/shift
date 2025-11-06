use std::env;
use std::io::{self, Write};

use tab_client::TabClient;
use tab_protocol::{
	SessionCreatePayload, SessionCreatedPayload, SessionLifecycle, SessionRole, SessionStatePayload,
	TabMessage, TabMessageFrame, message_header,
};
use tracing::{debug, info, warn};

fn main() -> Result<(), Box<dyn std::error::Error>> {
	tracing_subscriber::fmt().with_target(false).init();

	let token = env::var("SHIFT_SESSION_TOKEN")
		.expect("SHIFT_SESSION_TOKEN env var must contain the admin token");

	let mut client = TabClient::connect_default(token)?;
	info!(
		server = client.hello().server,
		protocol = client.hello().protocol,
		"Connected to Shift"
	);

	let mut created_sessions: Vec<CreatedSession> = Vec::new();
	repl(&mut client, &mut created_sessions)?;
	Ok(())
}

fn repl(
	client: &mut TabClient,
	created: &mut Vec<CreatedSession>,
) -> Result<(), Box<dyn std::error::Error>> {
	print_help();
	let stdin = io::stdin();
	loop {
		print!("admin> ");
		io::stdout().flush()?;
		let mut line = String::new();
		if stdin.read_line(&mut line)? == 0 {
			break;
		}
		let line = line.trim();
		if line.is_empty() {
			continue;
		}
		match parse_command(line) {
			Command::Create { role, display_name } => {
				match create_session(client, role, display_name.clone()) {
					Ok(payload) => {
						info!(
							new_session = payload.session.id.as_str(),
							role = ?payload.session.role,
							token = payload.token.as_str(),
							"Created session"
						);
						created.push(CreatedSession {
							id: payload.session.id,
							token: payload.token,
							role,
							display_name,
							state: payload.session.state,
						});
					}
					Err(err) => warn!("Session creation failed: {err}"),
				}
			}
			Command::List => {
				if created.is_empty() {
					println!("No sessions created yet");
				} else {
					for sess in created.iter() {
						println!(
							"- id={} role={:?} token={}{}",
							sess.id,
							sess.role,
							sess.token,
							sess
								.display_name
								.as_ref()
								.map(|name| format!(" display_name=\"{name}\""))
								.unwrap_or_default()
						);
					}
				}
			}
			Command::Recv => {
				handle_incoming(client, created)?;
			}
			Command::Help => print_help(),
			Command::Quit => break,
			Command::Unknown(msg) => println!("{msg}"),
		}
	}
	Ok(())
}

fn handle_incoming(
	client: &mut TabClient,
	created: &mut Vec<CreatedSession>,
) -> Result<(), Box<dyn std::error::Error>> {
	match client.receive()? {
		TabMessage::SessionCreated(payload) => {
			info!(
				session_id = payload.session.id.as_str(),
				role = ?payload.session.role,
				token = payload.token.as_str(),
				"Unsolicited session_created"
			);
		}
		TabMessage::SessionState(payload) => {
			handle_session_state(payload, created);
		}
		TabMessage::MonitorAdded(payload) => {
			info!(monitor_id = payload.monitor.id, "Monitor added");
		}
		TabMessage::MonitorRemoved(payload) => {
			info!(monitor_id = payload.monitor_id, "Monitor removed");
		}
		TabMessage::Error(payload) => {
			warn!(
				code = payload.code.as_str(),
				message = ?payload.message,
				"Error from server"
			);
		}
		other => {
			debug!(?other, "Received message");
		}
	}
	Ok(())
}

fn create_session(
	client: &mut TabClient,
	role: SessionRole,
	display_name: Option<String>,
) -> Result<SessionCreatedPayload, Box<dyn std::error::Error>> {
	let payload = SessionCreatePayload { role, display_name };
	let frame = TabMessageFrame::json(message_header::SESSION_CREATE, payload);
	client.send(&frame)?;
	wait_for_session_created(client)
}

fn wait_for_session_created(
	client: &mut TabClient,
) -> Result<SessionCreatedPayload, Box<dyn std::error::Error>> {
	loop {
		match client.receive()? {
			TabMessage::SessionCreated(payload) => return Ok(payload),
			TabMessage::SessionState(payload) => {
				debug!(session_id = payload.session.id.as_str(), state = ?payload.session.state, "Session state update while waiting");
			}
			other => {
				debug!(?other, "Received while waiting for session_created");
			}
		}
	}
}

fn parse_command(input: &str) -> Command {
	let mut parts = input.split_whitespace();
	let cmd = parts.next().unwrap_or_default();
	match cmd {
		"help" | "?" => Command::Help,
		"quit" | "exit" => Command::Quit,
		"recv" => Command::Recv,
		"list" | "sessions" => Command::List,
		"create" | "create-session" => {
			let role_str = match parts.next() {
				Some(r) => r,
				None => {
					return Command::Unknown("usage: create-session <admin|session> [display_name]".into());
				}
			};
			let role = match role_str {
				"admin" => SessionRole::Admin,
				"session" => SessionRole::Session,
				other => {
					return Command::Unknown(format!("unknown role '{other}', expected admin|session"));
				}
			};
			let display_name = parts.collect::<Vec<_>>().join(" ");
			let display_name = if display_name.is_empty() {
				None
			} else {
				Some(display_name)
			};
			Command::Create { role, display_name }
		}
		other => Command::Unknown(format!("unknown command '{other}' (type 'help')")),
	}
}

fn print_help() {
	println!("Commands:");
	println!("  create-session <admin|session> [display_name]  - Create a pending session token");
	println!(
		"  list                                           - List tokens generated during this session"
	);
	println!("  recv                                           - Wait for a message from Shift");
	println!("  help                                           - Show this message");
	println!("  quit                                           - Exit");
}

#[derive(Clone)]
struct CreatedSession {
	id: String,
	token: String,
	role: SessionRole,
	display_name: Option<String>,
	state: SessionLifecycle,
}

enum Command {
	Create {
		role: SessionRole,
		display_name: Option<String>,
	},
	List,
	Recv,
	Help,
	Quit,
	Unknown(String),
}

fn record_session(
	created: &mut Vec<CreatedSession>,
	session: &tab_protocol::SessionInfo,
	token: Option<String>,
) {
	if let Some(existing) = created.iter_mut().find(|s| s.id == session.id) {
		existing.state = session.state;
		existing.display_name = session.display_name.clone();
		existing.role = session.role;
		if let Some(token) = token {
			existing.token = token;
		}
	} else {
		created.push(CreatedSession {
			id: session.id.clone(),
			token: token.unwrap(),
			role: session.role,
			display_name: session.display_name.clone(),
			state: session.state,
		});
	}
}

fn handle_session_state(payload: SessionStatePayload, created: &mut Vec<CreatedSession>) {
	info!(
		session_id = payload.session.id.as_str(),
		state = ?payload.session.state,
		"Session state changed"
	);
	record_session(created, &payload.session, None);
}

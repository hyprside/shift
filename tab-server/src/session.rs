use std::collections::HashMap;

use tab_protocol::{MonitorInfo, SessionInfo, SessionLifecycle, SessionRole};

#[derive(Debug, Clone)]
pub struct Session {
	pub id: String,
	pub token: String,
	pub role: SessionRole,
	pub state: SessionLifecycle,
	pub display_name: Option<String>,
	pub is_active: bool,
	pub cursor_position: (i32, i32),
	pub monitors: Vec<MonitorInfo>,
}

#[derive(Debug, Default)]
pub struct SessionRegistry {
	sessions: HashMap<String, Session>,
	token_index: HashMap<String, String>,
}

impl SessionRegistry {
	pub fn new() -> Self {
		Self {
			sessions: HashMap::new(),
			token_index: HashMap::new(),
		}
	}

	pub fn insert_pending(
		&mut self,
		id: impl Into<String>,
		token: impl Into<String>,
		role: SessionRole,
		display_name: Option<String>,
	) {
		let id = id.into();
		let token = token.into();
		let session = Session {
			id: id.clone(),
			token: token.clone(),
			role,
			state: SessionLifecycle::Pending,
			display_name,
			is_active: false,
			cursor_position: (0, 0),
			monitors: Vec::new(),
		};
		self.token_index.insert(token, id.clone());
		self.sessions.insert(id, session);
	}

	pub fn authenticate_with_token(&mut self, token: &str) -> Option<String> {
		let session_id = self.token_index.remove(token)?;
		let session = self.sessions.get_mut(&session_id)?;
		if session.state == SessionLifecycle::Pending {
			session.state = SessionLifecycle::Loading;
			Some(session.id.clone())
		} else {
			None
		}
	}

	pub fn mark_consumed(&mut self, session_id: &str) -> Option<SessionInfo> {
		self.set_state(session_id, SessionLifecycle::Consumed)
	}

	pub fn get(&self, session_id: &str) -> Option<&Session> {
		self.sessions.get(session_id)
	}

	pub fn session_info(&self, session_id: &str) -> Option<SessionInfo> {
		let session = self.sessions.get(session_id)?;
		Some(SessionInfo {
			id: session.id.clone(),
			role: session.role,
			display_name: session.display_name.clone(),
			state: session.state,
			is_active: session.is_active,
			cursor_position: session.cursor_position,
			monitors: session.monitors.clone(),
		})
	}

	pub fn iter(&self) -> impl Iterator<Item = &Session> {
		self.sessions.values()
	}

	pub fn create_pending(
		&mut self,
		role: SessionRole,
		display_name: Option<String>,
	) -> (SessionInfo, String, String) {
		let session_id = crate::generate_id("ses");
		let token = crate::generate_id("tok");
		self.insert_pending(session_id.clone(), token.clone(), role, display_name);
		let info = self
			.session_info(&session_id)
			.expect("just inserted session must exist");
		(info, session_id, token)
	}

	pub fn set_state(
		&mut self,
		session_id: &str,
		new_state: SessionLifecycle,
	) -> Option<SessionInfo> {
		let session = self.sessions.get_mut(session_id)?;
		session.state = new_state;
		self.session_info(session_id)
	}
}

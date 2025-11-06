use std::collections::HashMap;

use tab_protocol::{SessionInfo, SessionLifecycle, SessionRole};

#[derive(Debug, Clone)]
pub struct Session {
	pub(crate) id: String,
	pub(crate) token: String,
	pub(crate) role: SessionRole,
	pub(crate) state: SessionLifecycle,
	pub(crate) display_name: Option<String>,
}
impl Session {
	pub fn token(&self) -> &str {
		self.token.as_str()
	}
	pub fn role(&self) -> SessionRole {
		self.role
	}
	pub fn state(&self) -> SessionLifecycle {
		self.state
	}
	pub fn display_name(&self) -> Option<&str> {
		self.display_name.as_ref().map(|s| s.as_str())
	}
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
	pub fn exists(&self, session_id: &str) -> bool {
		self.sessions.contains_key(session_id)
	}

	pub fn session_info(&self, session_id: &str) -> Option<SessionInfo> {
		let session = self.sessions.get(session_id)?;
		Some(SessionInfo {
			id: session.id.clone(),
			role: session.role,
			display_name: session.display_name.clone(),
			state: session.state,
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

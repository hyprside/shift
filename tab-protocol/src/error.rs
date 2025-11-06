/// Protocol level errors for framing/encoding/decoding.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
	#[error("unexpected end of stream")]
	UnexpectedEof,
	#[error("io error: {0}")]
	Io(#[from] std::io::Error),
	#[error("json error: {0}")]
	Json(#[from] serde_json::Error),
	#[error("invalid payload error: {0}")]
	InvalidPayload(String),
	#[error("utf8 error: {0}")]
	Utf8(#[from] std::string::FromUtf8Error),
	#[error("nix error: {0}")]
	Nix(#[from] nix::Error),
	#[error("unexpected extra data after payload")]
	TrailingData,
	#[error("operation would block (try again later)")]
	WouldBlock,
	#[error("message payload was truncated (buffer too small)")]
	Truncated,
	#[error("The received message header requires a payload but none was received")]
	ExpectedPayload,
	#[error(
		"Expected the received message to contain exactly {expected} attached file descriptors, got {found}"
	)]
	ExpectedFds { expected: u32, found: u32 },
}

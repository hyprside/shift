use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use uuid::Uuid;

pub fn generate_id(prefix: &str) -> String {
	let part1 = encode_uuid();
	let part2 = encode_uuid();
	format!("{prefix}_{}{}", part1, part2)
}

fn encode_uuid() -> String {
	let uuid = Uuid::new_v4();
	STANDARD_NO_PAD.encode(uuid.as_bytes())
}

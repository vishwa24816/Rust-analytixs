//! Session stitching: session_id = sha256(day_salt | ip | ua | domain).
//! Same visitor returns the same id all day; a new day rotates it.
//! The 30-min activity window lives in sessions_mat.last_at (writer).

use sha2::{Digest, Sha256};

pub fn session_id(ip: &str, user_agent: &str, domain: &str, day: &str) -> String {
    let mut h = Sha256::new();
    h.update(day.as_bytes());
    h.update(b"|");
    h.update(ip.as_bytes());
    h.update(b"|");
    h.update(user_agent.as_bytes());
    h.update(b"|");
    h.update(domain.as_bytes());
    format!("{:x}", h.finalize())
}

pub fn today_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

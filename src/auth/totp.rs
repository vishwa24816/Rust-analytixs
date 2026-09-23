use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;

// ponytail: totp-lite verify only; 160-bit secret, otpauth url for QR
pub fn new_secret() -> String {
    let mut buf = [0u8; 20];
    rand::thread_rng().fill_bytes(&mut buf);
    B64.encode(buf)
}

pub fn otpauth_url(secret_b64: &str, email: &str) -> String {
    let raw = B64.decode(secret_b64).unwrap_or_default();
    let b32 = base32_nopad(&raw);
    format!("otpauth://totp/RustAnalytix:{email}?secret={b32}&issuer=RustAnalytix")
}

fn base32_nopad(bytes: &[u8]) -> String {
    const ALPH: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &b in bytes {
        buf = (buf << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPH[((buf >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPH[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

pub fn verify(secret_b64: &str, code: &str) -> bool {
    let raw = match B64.decode(secret_b64) {
        Ok(r) => r,
        Err(_) => return false,
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // accept ±1 step (30s)
    [0, 1u64.wrapping_neg(), 1].iter().any(|&drift| {
        let t = now.wrapping_add(drift.wrapping_mul(30)) / 30;
        totp_lite::totp_custom::<sha1::Sha1>(30, 6, &raw, t) == code
    })
}

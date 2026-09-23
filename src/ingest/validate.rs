//! Request-level drops: DNT header, bot user-agents, empty UA.

/// True when the request must be silently dropped (202, not stored).
pub fn should_drop(user_agent: &str, dnt: Option<&str>) -> bool {
    if dnt == Some("1") {
        return true;
    }
    is_bot(user_agent)
}

const BOT_HINTS: &[&str] = &[
    "bot", "crawl", "spider", "slurp", "mediapartners", "baidu", "yandex", "sogou",
    "exabot", "facebot", "ia_archiver", "ahrefs", "semrush", "mj12", "dotbot",
    "petalbot", "headless", "phantom", "nightmare", "selenium", "webdriver",
    "python-requests", "go-http-client", "curl", "wget",
];

pub fn is_bot(ua: &str) -> bool {
    if ua.trim().is_empty() {
        return true;
    }
    let lower = ua.to_lowercase();
    BOT_HINTS.iter().any(|h| lower.contains(h))
}

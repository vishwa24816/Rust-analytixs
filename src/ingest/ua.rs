//! Tiny UA heuristics. No regex database; upgrade to a real parser
//! (uaparser + regexes.yaml) when misclassification shows up in stats.
//! ponytail: substring scan, good enough for device/browser/os splits.

pub struct Client {
    pub device: &'static str,
    pub browser: &'static str,
    pub os: &'static str,
}

pub fn parse(ua: &str) -> Client {
    let l = ua.to_lowercase();
    let device = if l.contains("mobile") || l.contains("iphone") || l.contains("android") {
        if l.contains("tablet") || l.contains("ipad") {
            "Tablet"
        } else {
            "Mobile"
        }
    } else {
        "Desktop"
    };
    let browser = if l.contains("edg/") || l.contains("edge") {
        "Edge"
    } else if l.contains("opr/") || l.contains("opera") {
        "Opera"
    } else if l.contains("chrome") {
        "Chrome"
    } else if l.contains("firefox") || l.contains("fxios") {
        "Firefox"
    } else if l.contains("safari") {
        "Safari"
    } else {
        ""
    };
    let os = if l.contains("windows") {
        "Windows"
    } else if l.contains("iphone") || l.contains("ipad") {
        "iOS"
    } else if l.contains("mac os") || l.contains("macintosh") {
        "macOS"
    } else if l.contains("android") {
        "Android"
    } else if l.contains("linux") {
        "Linux"
    } else {
        ""
    };
    Client { device, browser, os }
}

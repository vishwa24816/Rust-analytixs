//! Fixed-window in-memory rate limiter: 300 req/min per IP.
//! ponytail: HashMap + sweep, no governor crate, single-node only.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

pub struct Limiter {
    inner: Mutex<HashMap<String, (u32, Instant)>>,
    max: u32,
    window: Duration,
}

impl Limiter {
    pub fn new(max_per_minute: u32) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            max: max_per_minute,
            window: Duration::from_secs(60),
        }
    }

    /// True when the request is allowed.
    pub fn allow(&self, ip: &str) -> bool {
        let mut map = self.inner.lock().unwrap();
        let now = Instant::now();
        // opportunistic sweep
        if map.len() > 10_000 {
            map.retain(|_, (_, t)| now.duration_since(*t) < self.window * 2);
        }
        match map.get_mut(ip) {
            Some((n, start)) => {
                if now.duration_since(*start) >= self.window {
                    *n = 1;
                    *start = now;
                    true
                } else if *n < self.max {
                    *n += 1;
                    true
                } else {
                    false
                }
            }
            None => {
                map.insert(ip.to_string(), (1, now));
                true
            }
        }
    }
}

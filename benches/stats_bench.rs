use criterion::{criterion_group, criterion_main, Criterion};
use rust_analytix::ingest::RawEvent;

fn payload(name: &str) -> String {
    serde_json::json!({
        "n": name,
        "v": 36,
        "u": "https://example.com/blog?utm_source=google",
        "d": "example.com",
        "r": "https://google.com/",
        "p": {"author": "x"},
    })
    .to_string()
}

fn bench_normalize(c: &mut Criterion) {
    let body = payload("pageview");
    c.bench_function("ingest/normalize_pageview", |b| {
        b.iter(|| {
            let raw: RawEvent = serde_json::from_str(&body).unwrap();
            raw.normalize().unwrap()
        })
    });
    let custom = payload("Signup");
    c.bench_function("ingest/normalize_custom", |b| {
        b.iter(|| {
            let raw: RawEvent = serde_json::from_str(&custom).unwrap();
            raw.normalize().unwrap()
        })
    });
}

criterion_group!(benches, bench_normalize);
criterion_main!(benches);

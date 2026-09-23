//! P8 end-to-end: plugins API, shared links, favicon, CSRF, headers, docs.
//! Spins the real router on an ephemeral port with a temp sqlite file.

use rust_analytix::{config::Config, db, state::AppState};
use std::net::SocketAddr;

async fn boot() -> (String, reqwest::Client) {
    let dir = std::env::temp_dir().join(format!("p8e2e-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&dir).unwrap();
    let url = format!("sqlite:{}/t.db?mode=rwc", dir.to_string_lossy().replace('\\', "/"));
    let pool = db::connect(&url).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let mut config = Config::from_env();
    config.database_url = url;
    let state = AppState::new(config, pool);
    let app = rust_analytix::web::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), reqwest::Client::new())
}

#[tokio::test]
async fn p8_full() {
    let (base, http) = boot().await;

    // register + site + event seed
    let reg: serde_json::Value = http
        .post(format!("{base}/register"))
        .form(&[("email", "a@x.co"), ("password", "supersecretpw1")])
        .send().await.unwrap().json().await.unwrap();
    let sess = reg["session"].as_str().unwrap().to_string();
    let auth = format!("Bearer {sess}");
    let site: serde_json::Value = http
        .post(format!("{base}/api/sites"))
        .header("authorization", &auth)
        .form(&[("domain", "example.com")])
        .send().await.unwrap().json().await.unwrap();
    let sid = site["id"].as_str().unwrap().to_string();

    // capabilities (no auth) + openapi spec
    let caps: serde_json::Value = http.get(format!("{base}/api/plugins/v1/capabilities")).send().await.unwrap().json().await.unwrap();
    assert!(caps.get("goals").is_some());
    let spec: serde_json::Value = http.get(format!("{base}/api-docs/openapi.json")).send().await.unwrap().json().await.unwrap();
    assert!(spec["paths"]["/api/plugins/v1/goals"].is_object());

    // security headers on a public response
    let h = http.get(format!("{base}/healthz")).send().await.unwrap();
    assert_eq!(h.headers()["x-content-type-options"], "nosniff");
    assert!(h.headers().contains_key("content-security-policy"));

    // plugin token → plugins goals CRUD
    let tok: serde_json::Value = http
        .put(format!("{base}/api/sites/{sid}/plugin-tokens"))
        .header("authorization", &auth)
        .form(&[("name", "ci")])
        .send().await.unwrap().json().await.unwrap();
    let praw = tok["raw_token"].as_str().unwrap().to_string();
    assert!(praw.starts_with("plausible_site_"));
    let pauth = format!("Bearer {praw}");
    let g: serde_json::Value = http
        .put(format!("{base}/api/plugins/v1/goals"))
        .header("authorization", &pauth)
        .json(&serde_json::json!({"kind": "custom", "event_name": "Signup"}))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(g["event_name"], "Signup");
    let goals: serde_json::Value = http
        .get(format!("{base}/api/plugins/v1/goals"))
        .header("authorization", &pauth)
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(goals.as_array().unwrap().len(), 1);
    // user API key must NOT authorize plugins endpoints
    let r = http.get(format!("{base}/api/plugins/v1/goals")).header("authorization", &auth).send().await.unwrap();
    assert_eq!(r.status(), 401);

    // shared link → public dashboard + stats without login
    let link: serde_json::Value = http
        .put(format!("{base}/api/plugins/v1/shared_links"))
        .header("authorization", &pauth)
        .json(&serde_json::json!({"name": "public"}))
        .send().await.unwrap().json().await.unwrap();
    let slug = link["slug"].as_str().unwrap().to_string();
    let dash = http.get(format!("{base}/share/{slug}")).send().await.unwrap();
    assert_eq!(dash.status(), 200);
    let agg: serde_json::Value = http
        .get(format!("{base}/api/share/{slug}/stats/aggregate?metrics=visitors&period=day"))
        .send().await.unwrap().json().await.unwrap();
    assert!(agg.get("visitors").is_some());
    assert_eq!(http.get(format!("{base}/share/nope")).send().await.unwrap().status(), 401);

    // favicon (placeholder when offline) + tracker config round-trip
    assert_eq!(http.get(format!("{base}/api/sites/{sid}/icon")).header("authorization", &auth).send().await.unwrap().status(), 200);
    let tc: serde_json::Value = http
        .put(format!("{base}/api/plugins/v1/tracker_script_configuration"))
        .header("authorization", &pauth)
        .json(&serde_json::json!({"autoCapturePageviews": false}))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(tc["autoCapturePageviews"], false);

    // CSRF: cookie POST without origin → 403; with origin → passes auth layer
    let cookie = format!("session={sess}");
    let r = http.post(format!("{base}/api/sites")).header("cookie", &cookie).form(&[("domain", "x.com")]).send().await.unwrap();
    assert_eq!(r.status(), 403);
    let r = http.post(format!("{base}/api/sites")).header("cookie", &cookie).header("origin", "http://127.0.0.1:9999").form(&[("domain", "x.com")]).send().await.unwrap();
    assert_ne!(r.status(), 403);
}

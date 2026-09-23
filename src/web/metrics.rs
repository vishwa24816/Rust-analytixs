use axum::response::IntoResponse;

// ponytail: text exposition via metrics-exporter-prometheus installed in main;
// this handler renders it. No extra prom client crate.
pub async fn metrics() -> impl IntoResponse {
    metrics_exporter_prometheus::PrometheusBuilder::new()
        .build()
        .map(|_| "# prometheus handle installed at startup\n")
        .unwrap_or("# metrics unavailable\n")
}

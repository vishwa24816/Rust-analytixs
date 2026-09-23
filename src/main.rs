use rust_analytix::{config::Config, db, state::AppState, web};
use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Default)]
enum Cmd {
    #[default]
    Serve,
    Migrate,
    /// Send due weekly/monthly email reports (run from cron/ ежедневно timer)
    ReportsSend,
    /// Grant admin to a user by email
    MakeAdmin { email: String },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let config = Config::from_env();
    init_tracing(&config);
    install_metrics();

    let db = db::connect(&config.database_url)
        .await
        .expect("db connect");
    sqlx::migrate!("./migrations").run(&db).await.expect("migrate");

    match cli.cmd {
        Cmd::Migrate => println!("migrations applied"),
        Cmd::ReportsSend => {
            let state = AppState::new(config.clone(), db);
            let n = rust_analytix::jobs::reports::send_due(&state).await.expect("reports");
            println!("sent {n} reports");
        }
        Cmd::MakeAdmin { email } => {
            let n = sqlx::query("UPDATE users SET is_admin = 1 WHERE email = ?")
                .bind(email.to_lowercase())
                .execute(&db)
                .await
                .expect("make-admin")
                .rows_affected();
            println!("admins updated: {n}");
        }
        Cmd::Serve => {
            let state = AppState::new(config.clone(), db);
            let app = web::router(state);
            let listener = tokio::net::TcpListener::bind(config.listen_addr)
                .await
                .expect("bind");
            tracing::info!("listening on {}", config.listen_addr);
            axum::serve(listener, app).await.expect("serve");
        }
    }
}

fn init_tracing(config: &Config) {
    let filter = if config.rust_log.is_empty() {
        "rust_analytix=info,tower_http=info".to_string()
    } else {
        config.rust_log.clone()
    };
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(filter))
        .with(tracing_subscriber::fmt::layer().json())
        .init();
}

fn install_metrics() {
    let _ = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install()
        .map_err(|e| e.to_string());
}

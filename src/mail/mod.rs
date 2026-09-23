//! Outbound mail. Without SMTP_URL configured, messages are logged
//! (dev mode) instead of sent — no silent drops.

use crate::config::Config;

#[derive(Clone)]
pub struct Mailer {
    config: Config,
}

impl Mailer {
    pub fn new(config: &Config) -> Self {
        Self { config: config.clone() }
    }

    async fn send(&self, to: &str, subject: &str, html: String, text: String) {
        if self.config.smtp_url.is_empty() {
            tracing::info!(to, subject, "mail skipped (no SMTP_URL); body logged");
            tracing::debug!(html, "mail body");
            return;
        }
        let body = lettre::Message::builder()
            .from(self.config.mail_from.parse().unwrap_or_else(|_| {
                "Rust Analytix <hello@rust-analytix.local>".parse().unwrap()
            }))
            .to(to.parse().unwrap_or_else(|_| {
                tracing::warn!(to, "invalid recipient, dropping mail");
                "invalid@localhost".parse().unwrap()
            }))
            .subject(subject)
            .multipart(lettre::message::MultiPart::alternative_plain_html(text, html))
            .unwrap();
        match lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::from_url(&self.config.smtp_url)
            .map(|b| b.build()) {
            Ok(transport) => {
                use lettre::AsyncTransport;
                if let Err(e) = transport.send(body).await {
                    tracing::error!(error = %e, to, "mail send failed");
                }
            }
            Err(e) => tracing::error!(error = %e, "bad SMTP_URL"),
        }
    }

    pub async fn send_verify(&self, to: &str, link: &str) {
        let html = minijinja::render!(
            "<p>Confirm your email: <a href=\"{{ link }}\">{{ link }}</a></p>",
            link => link
        );
        self.send(
            to,
            "Confirm your email",
            html,
            format!("Confirm your email: {link}"),
        )
        .await;
    }

    pub async fn send_reset(&self, to: &str, link: &str) {
        let html = minijinja::render!(
            "<p>Reset your password: <a href=\"{{ link }}\">{{ link }}</a></p>",
            link => link
        );
        self.send(
            to,
            "Reset your password",
            html,
            format!("Reset your password: {link}"),
        )
        .await;
    }

    pub async fn send_invite(&self, to: &str, domain: &str, role: &str, link: &str) {
        let html = minijinja::render!(
            "<p>You've been invited to <b>{{ domain }}</b> as {{ role }}. <a href=\"{{ link }}\">Accept invitation</a></p>",
            domain => domain, role => role, link => link
        );
        self.send(
            to,
            &format!("You've been invited to {domain}"),
            html,
            format!("You've been invited to {domain} as {role}: {link}"),
        )
        .await;
    }

    pub async fn send_report(&self, to: &str, domain: &str, frequency: &str, stats: &serde_json::Value) {
        let visitors = stats.get("visitors").map(|v| v.to_string()).unwrap_or_default();
        let pageviews = stats.get("pageviews").map(|v| v.to_string()).unwrap_or_default();
        let html = minijinja::render!(
            "<h1>{{ domain }} {{ frequency }} report</h1><p>Visitors: {{ visitors }}</p><p>Pageviews: {{ pageviews }}</p>",
            domain => domain, frequency => frequency, visitors => visitors, pageviews => pageviews
        );
        self.send(
            to,
            &format!("[{domain}] {frequency} report"),
            html,
            format!("{domain} {frequency} report: {visitors} visitors, {pageviews} pageviews"),
        )
        .await;
    }
}

//! Paddle Classic webhook: RSA-SHA1 over PHP-serialized sorted params
//! (mirrors upstream `PaddleController.verify_signature`).
//! Alerts handled: subscription_created/updated/cancelled/payment_succeeded.

use std::collections::BTreeMap;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rsa::{pkcs1::DecodeRsaPublicKey, pkcs8::DecodePublicKey, Pkcs1v15Sign, RsaPublicKey};
use sha1::{Digest, Sha1};

use crate::{db::DbPool, error::AppError};

static PADDLE_PEM: &str = include_str!("../../billing/paddle.pem");
static PADDLE_SANDBOX_PEM: &str = include_str!("../../billing/paddle_sandbox.pem");

/// PHP serialize an array of sorted string pairs, Paddle style:
/// a:N:{i:0;s:LEN:"key";s:LEN:"val";...} with 0-based integer keys.
pub fn php_serialize_sorted(params: &BTreeMap<String, String>) -> String {
    let mut out = format!("a:{}:{{", params.len());
    for (i, (k, v)) in params.iter().enumerate() {
        out.push_str(&format!("i:{i};s:{}:\"{k}\";s:{}:\"{v}\";", k.len(), v.len()));
    }
    out.push('}');
    out
}

pub fn verify(params: &BTreeMap<String, String>, sandbox: bool) -> Result<(), AppError> {
    let sig_b64 = params.get("p_signature").ok_or(AppError::BadRequest("missing signature".into()))?;
    let sig = B64.decode(sig_b64).map_err(|_| AppError::BadRequest("bad signature encoding".into()))?;
    let mut msg_params = params.clone();
    msg_params.remove("p_signature");
    let msg = php_serialize_sorted(&msg_params);
    let pem = if sandbox { PADDLE_SANDBOX_PEM } else { PADDLE_PEM };
    // ponytail: Paddle ships PKCS#8 ("PUBLIC KEY"); accept PKCS#1 too
    let key = RsaPublicKey::from_public_key_pem(pem)
        .or_else(|_| RsaPublicKey::from_pkcs1_pem(pem))
        .map_err(|e| AppError::BadRequest(format!("bad paddle key: {e}")))?;
    let digest = Sha1::digest(msg.as_bytes());
    key.verify(Pkcs1v15Sign::new_unprefixed(), &digest, &sig)
        .map_err(|_| AppError::BadRequest("invalid paddle signature".into()))
}

pub async fn handle(db: &DbPool, params: &BTreeMap<String, String>) -> Result<(), AppError> {
    let alert = params.get("alert_name").cloned().unwrap_or_default();
    match alert.as_str() {
        "subscription_created" | "subscription_updated" => upsert_subscription(db, params, "active").await,
        "subscription_cancelled" => upsert_subscription(db, params, "deleted").await,
        "subscription_payment_succeeded" => {
            // keep active; record next bill date when present
            upsert_subscription(db, params, "active").await
        }
        _ => Err(AppError::BadRequest(format!("unknown alert {alert}"))),
    }
}

async fn upsert_subscription(db: &DbPool, p: &BTreeMap<String, String>, status: &str) -> Result<(), AppError> {
    let email = p.get("email").cloned().unwrap_or_default().to_lowercase();
    let uid: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind(&email)
        .fetch_optional(db)
        .await?;
    let Some(uid) = uid else {
        return Err(AppError::BadRequest("unknown user".into()));
    };
    let empty = String::new();
    let product_id = p.get("subscription_plan_id").unwrap_or(&empty);
    // ensure a row exists (trial defaults), then overwrite from the alert
    crate::billing::gate::subscription_for(db, &uid).await?;
    if let Some(plan) = crate::billing::plans::by_product_id(product_id) {
        sqlx::query(
            "UPDATE subscriptions SET paddle_subscription_id = ?, plan_kind = ?, plan_generation = ?, monthly_pageview_limit = ?, site_limit = ?, team_member_limit = ?, status = ?, next_bill_date = ?, updated_at = datetime('now') WHERE user_id = ?",
        )
        .bind(p.get("subscription_id").unwrap_or(&empty))
        .bind(&plan.kind)
        .bind(plan.generation)
        .bind(plan.monthly_pageview_limit)
        .bind(plan.site_limit)
        .bind(plan.team_member_limit.unwrap_or(0))
        .bind(status)
        .bind(p.get("next_bill_date").unwrap_or(&empty))
        .bind(&uid)
        .execute(db)
        .await?;
    } else {
        sqlx::query(
            "UPDATE subscriptions SET paddle_subscription_id = ?, status = ?, updated_at = datetime('now') WHERE user_id = ?",
        )
        .bind(p.get("subscription_id").unwrap_or(&empty))
        .bind(status)
        .bind(&uid)
        .execute(db)
        .await?;
    }
    crate::audit::record(db, None, "billing.webhook", "subscription", &uid).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_serialize_matches_paddle_format() {
        let mut m = BTreeMap::new();
        m.insert("alert_name".to_string(), "subscription_created".to_string());
        m.insert("email".to_string(), "a@x.co".to_string());
        assert_eq!(
            php_serialize_sorted(&m),
            r#"a:2:{i:0;s:10:"alert_name";s:20:"subscription_created";i:1;s:5:"email";s:6:"a@x.co";}"#
        );
    }

    #[test]
    fn garbage_signature_rejected() {
        let mut m = BTreeMap::new();
        m.insert("alert_name".to_string(), "subscription_created".to_string());
        m.insert("p_signature".to_string(), B64.encode([0u8; 256]));
        assert!(verify(&m, false).is_err());
    }
}

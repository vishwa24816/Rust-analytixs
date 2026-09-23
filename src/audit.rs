//! Audit trail: who did what, to which entity.

use crate::db::DbPool;

pub async fn record(
    db: &DbPool,
    actor: Option<&str>,
    action: &str,
    entity: &str,
    entity_id: &str,
) {
    let _ = sqlx::query(
        "INSERT INTO audit_log (id, actor_id, action, entity, entity_id) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(actor)
    .bind(action)
    .bind(entity)
    .bind(entity_id)
    .execute(db)
    .await;
}

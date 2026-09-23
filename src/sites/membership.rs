use serde::{Deserialize, Serialize};

use crate::{db::DbPool, error::AppError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Owner,
    Admin,
    Viewer,
    Billing,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Admin => "admin",
            Role::Viewer => "viewer",
            Role::Billing => "billing",
        }
    }
    fn rank(&self) -> u8 {
        match self {
            Role::Owner => 3,
            Role::Admin => 2,
            Role::Viewer | Role::Billing => 1,
        }
    }
    pub fn at_least(&self, need: Role) -> bool {
        self.rank() >= need.rank()
    }
}

/// Returns the caller's role on a site, or None.
pub async fn role_of(db: &DbPool, site_id: &str, user_id: &str) -> Result<Option<Role>, AppError> {
    let r: Option<String> =
        sqlx::query_scalar("SELECT role FROM memberships WHERE site_id = ? AND user_id = ?")
            .bind(site_id)
            .bind(user_id)
            .fetch_optional(db)
            .await?;
    Ok(r.as_deref().and_then(parse))
}

fn parse(s: &str) -> Option<Role> {
    match s {
        "owner" => Some(Role::Owner),
        "admin" => Some(Role::Admin),
        "viewer" => Some(Role::Viewer),
        "billing" => Some(Role::Billing),
        _ => None,
    }
}

/// Gate helper: 403 when the caller lacks the required role.
pub async fn require(db: &DbPool, site_id: &str, user_id: &str, need: Role) -> Result<(), AppError> {
    match role_of(db, site_id, user_id).await? {
        Some(r) if r.at_least(need) => Ok(()),
        _ => Err(AppError::Forbidden("insufficient role".into())),
    }
}

/// Adapters so handlers read fluently.
pub trait RoleCheck {
    fn at_least(&self, need: Role) -> bool;
}
impl RoleCheck for Role {
    fn at_least(&self, need: Role) -> bool {
        Role::at_least(self, need)
    }
}

-- P7: billing subscriptions, scheduled reports, audit log, admin flag
ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0;
CREATE TABLE IF NOT EXISTS subscriptions (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  paddle_subscription_id TEXT NOT NULL DEFAULT '',
  plan_kind TEXT NOT NULL DEFAULT 'trial',
  plan_generation INTEGER NOT NULL DEFAULT 5,
  monthly_pageview_limit INTEGER NOT NULL DEFAULT 10000,
  site_limit INTEGER NOT NULL DEFAULT 10,
  team_member_limit INTEGER NOT NULL DEFAULT 10,
  status TEXT NOT NULL DEFAULT 'trial' CHECK (status IN ('trial','active','past_due','paused','deleted')),
  trial_expiry TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now', '+30 days')),
  next_bill_date TEXT,
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
) STRICT;
CREATE TABLE IF NOT EXISTS reports (
  id TEXT PRIMARY KEY,
  site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
  owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  frequency TEXT NOT NULL CHECK (frequency IN ('weekly','monthly')),
  recipients TEXT NOT NULL DEFAULT '[]',
  last_sent_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
) STRICT;
CREATE TABLE IF NOT EXISTS audit_log (
  id TEXT PRIMARY KEY,
  actor_id TEXT,
  action TEXT NOT NULL,
  entity TEXT NOT NULL DEFAULT '',
  entity_id TEXT NOT NULL DEFAULT '',
  meta TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
) STRICT;
CREATE INDEX IF NOT EXISTS idx_audit_actor ON audit_log(actor_id);

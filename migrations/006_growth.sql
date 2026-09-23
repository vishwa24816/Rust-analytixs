-- P6: goals, funnels, segments, annotations, imports, google tokens
CREATE TABLE IF NOT EXISTS goals (
  id TEXT PRIMARY KEY,
  site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('page','custom','revenue')),
  event_name TEXT NOT NULL,
  page_path TEXT NOT NULL DEFAULT '',
  currency TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now')),
  UNIQUE (site_id, kind, event_name, page_path)
) STRICT;
CREATE TABLE IF NOT EXISTS funnels (
  id TEXT PRIMARY KEY,
  site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
) STRICT;
CREATE TABLE IF NOT EXISTS funnel_steps (
  funnel_id TEXT NOT NULL REFERENCES funnels(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  goal_id TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
  PRIMARY KEY (funnel_id, position)
) STRICT;
CREATE TABLE IF NOT EXISTS segments (
  id TEXT PRIMARY KEY,
  site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
  owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  filters TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
) STRICT;
CREATE TABLE IF NOT EXISTS annotations (
  id TEXT PRIMARY KEY,
  site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
  date TEXT NOT NULL,
  content TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
) STRICT;
CREATE INDEX IF NOT EXISTS idx_annotations_site_date ON annotations(site_id, date);
CREATE TABLE IF NOT EXISTS imports (
  id TEXT PRIMARY KEY,
  site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
  source TEXT NOT NULL CHECK (source IN ('csv','ga4','universal','search_console')),
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','running','done','failed')),
  label TEXT NOT NULL DEFAULT '',
  error TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
) STRICT;
-- Google OAuth tokens for Search Console / GA imports (per user)
CREATE TABLE IF NOT EXISTS google_tokens (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  refresh_token TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
) STRICT;

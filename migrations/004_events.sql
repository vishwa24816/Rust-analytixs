-- P3: analytics events (replaces ClickHouse events/sessions) + materialized sessions
CREATE TABLE IF NOT EXISTS events (
  id TEXT PRIMARY KEY,
  site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
  ts TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now')),
  session_id TEXT NOT NULL,
  name TEXT NOT NULL,
  pathname TEXT NOT NULL DEFAULT '/',
  hostname TEXT NOT NULL DEFAULT '',
  referrer TEXT NOT NULL DEFAULT '',
  referrer_source TEXT NOT NULL DEFAULT '',
  country TEXT,
  device TEXT NOT NULL DEFAULT 'Desktop',
  browser TEXT NOT NULL DEFAULT '',
  os TEXT NOT NULL DEFAULT '',
  utm_source TEXT NOT NULL DEFAULT '',
  utm_medium TEXT NOT NULL DEFAULT '',
  utm_campaign TEXT NOT NULL DEFAULT '',
  utm_content TEXT NOT NULL DEFAULT '',
  utm_term TEXT NOT NULL DEFAULT '',
  props TEXT NOT NULL DEFAULT '{}',
  revenue_cents INTEGER,
  scroll_depth INTEGER,
  engagement_ms INTEGER,
  interactive INTEGER NOT NULL DEFAULT 1
) STRICT;
CREATE INDEX IF NOT EXISTS idx_events_site_ts ON events(site_id, ts);
CREATE INDEX IF NOT EXISTS idx_events_session ON events(site_id, session_id, ts);
CREATE INDEX IF NOT EXISTS idx_events_name ON events(site_id, name, ts);

-- ponytail: maintained at ingest (30-min stitch); cheap realtime + bounce math
CREATE TABLE IF NOT EXISTS sessions_mat (
  session_id TEXT NOT NULL,
  site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
  started_at TEXT NOT NULL,
  last_at TEXT NOT NULL,
  pageviews INTEGER NOT NULL DEFAULT 0,
  events_count INTEGER NOT NULL DEFAULT 0,
  is_bounce INTEGER NOT NULL DEFAULT 1,
  entry_page TEXT NOT NULL DEFAULT '',
  exit_page TEXT NOT NULL DEFAULT '',
  referrer TEXT NOT NULL DEFAULT '',
  country TEXT,
  device TEXT NOT NULL DEFAULT 'Desktop',
  browser TEXT NOT NULL DEFAULT '',
  os TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (site_id, session_id)
) STRICT;
CREATE INDEX IF NOT EXISTS idx_sessions_site_last ON sessions_mat(site_id, last_at);

-- P8: shared links, site-scoped plugin tokens, favicon cache,
-- custom-prop allowlist, tracker script configuration
CREATE TABLE IF NOT EXISTS shared_links (
  id TEXT PRIMARY KEY,
  site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  password_hash TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
) STRICT;
CREATE TABLE IF NOT EXISTS plugin_tokens (
  id TEXT PRIMARY KEY,
  site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  key_hash TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
) STRICT;
CREATE TABLE IF NOT EXISTS favicons (
  site_id TEXT PRIMARY KEY REFERENCES sites(id) ON DELETE CASCADE,
  bytes BLOB NOT NULL,
  content_type TEXT NOT NULL DEFAULT 'image/svg+xml',
  fetched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
) STRICT;
CREATE TABLE IF NOT EXISTS site_props (
  site_id TEXT NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
  prop TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (site_id, prop)
) STRICT;
CREATE TABLE IF NOT EXISTS tracker_config (
  site_id TEXT PRIMARY KEY REFERENCES sites(id) ON DELETE CASCADE,
  config TEXT NOT NULL DEFAULT '{}',
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d %H:%M:%S','now'))
) STRICT;

# Rust Analytix

Privacy-friendly version of Google Analytics in a single Rust binary: tracker ingestion, stats engine, dashboard, billing, and email reports — backed only by **SQLite**. No Postgres, no ClickHouse, no Redis, no Node build step.

This is a full port of the Google Analytics feature set to **Rust + vanilla HTML/JS + SQLite**.

## Features

- **Tracker ingestion** — `POST /api/event` compatible with the official tracker payload (`pageview`, custom events, engagement, revenue, props, autotracked outbound/file/hash events). Serves 1027 prebuilt tracker variants at `/js/*`. Bot + DNT filtering, shield rules, per-IP rate limiting, day-salted session stitching. Visitor IPs are never stored.
- **Stats engine** — aggregate, timeseries, breakdown, realtime, compare-vs-previous-period, CSV export, conversion funnels, journey (entry → next pages), goal conversions. v1 query-string API, v2 JSON query API, and an upstream-compatible `POST /api/stats/:domain/query` alias.
- **Dashboard** — dependency-free ES modules: top stats with deltas, canvas graph (metric + interval pickers, hover readout), 8 breakdown cards with tabs, click-to-filter pills, filter suggestions, saved segments, annotations, funnels, journey explorer, custom-prop explorer, conversions with CR, search keywords, world choropleth map, 5s realtime ticker, custom date ranges, fullscreen drilldown modals (search + pagination + CSV).
- **Auth** — Argon2 passwords, sessions (Bearer or HttpOnly cookie), TOTP 2FA, API keys (`plausible_*`), Google OAuth login (+offline scope for Search Console), email verify/reset, invitation mails.
- **Sites & teams** — sites, roles (owner/admin/viewer/billing), invitations, teams, shield rules (ip/country/page/hostname), public share links (slug ± password) with slug-scoped stats.
- **Growth** — page/custom/revenue goals, ordered funnels with dropoff, segments, annotations, CSV event import (background job + status), Search Console keywords via stored Google tokens.
- **Billing & ops** — plan catalog (upstream `plans_v5.json`), 30-day trials, site/member/volume gates (402 on locked sites), Paddle Classic webhooks (RSA-SHA1, sandbox supported), weekly/monthly email reports (`reports-send`), audit log, admin endpoints + `make-admin`, `DISABLE_REGISTRATION` flag.
- **Plugins API** — site-scoped tokens (`plausible_site_*`), capabilities, shared links, goals, funnels (read), custom-prop allowlist, tracker-script configuration, favicon fetch + cache. Swagger UI at `/api-docs/`.

## Quickstart (local)

Requirements: Rust 1.82+ (SQLite is bundled — no database to install).

```sh
cp .env.example .env        # adjust APP_URL / DATABASE_URL
cargo run -- migrate        # apply SQL migrations
cargo run -- serve          # listen on 127.0.0.1:8000
```

Open http://127.0.0.1:8000/ → Register → Login → add a site → paste the snippet:

```html
<script defer data-domain="YOUR-DOMAIN" src="http://127.0.0.1:8000/js/plausible.js"></script>
```

Then open `http://127.0.0.1:8000/dashboard/YOUR-DOMAIN?period=day`.

> Windows note: the workspace path contains a space, which breaks the GNU
> `dlltool` linker step. `.cargo/config.toml` already redirects build
> artifacts to `%TEMP%\opencode\plausible-rs-target`, so plain `cargo`
> commands just work.

## Quickstart (podman)

```sh
podman build -f Containerfile -t rust-analytix .
mkdir -p data
podman run --rm -p 8000:8000 -v ./data:/data:Z \
  -e DATABASE_URL=sqlite:/data/app.db?mode=rwc \
  -e APP_URL=http://127.0.0.1:8000 \
  --env-file .env rust-analytix
```

See `podman.md` for migrations, the reports timer, and first-admin setup.

## Configuration

All config is environment (see `.env.example`):

| Variable | Default | Purpose |
|---|---|---|
| `LISTEN_ADDR` | `127.0.0.1:8000` | bind address |
| `DATABASE_URL` | `sqlite:./data/app.db?mode=rwc` | single SQLite file (WAL) |
| `APP_URL` | `http://127.0.0.1:8000` | public URL (links, OAuth redirect, CSRF origin) |
| `RUST_LOG` | `rust_analytix=info,tower_http=info` | JSON tracing filter |
| `SMTP_URL` | _(empty)_ | SMTP connection URL; empty = log mail instead of sending |
| `MAIL_FROM` | `Rust Analytix <hello@rust-analytix.local>` | sender |
| `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` | _(empty)_ | enables `/oauth/google` + Search Console (empty = 400 "not configured") |
| `PADDLE_SANDBOX` | _(empty)_ | `1` = verify webhooks with the sandbox key |
| `DISABLE_REGISTRATION` | _(empty)_ | `1`/`true` = block `/register` |
| `TRACKER_DIR` | `tracker/js` | where compiled tracker scripts live |
| `PUBLIC_DIR` | `public` | dashboard static assets |

CLI: `rust_analytix serve|migrate|reports-send|make-admin <email>`.

## API overview

Auth: `Authorization: Bearer <session|plausible_*|plausible_site_*>`, session cookie, or nothing (public sites / shared links). Full machine-readable spec at `/api-docs/openapi.json` (Swagger UI at `/api-docs/`).

- Tracker: `POST /api/event` (202 = stored or silently dropped: bot/DNT/unknown domain/shield), `GET /js/:script`
- Stats v1: `GET /api/v1/stats/{aggregate,breakdown,timeseries,csv,realtime/visitors}` — `site_id, period(day|7d|30d|month|6mo|12mo|year|custom|all|realtime), date?, from/to?, filters? (visit:browser==Chrome;...), metrics?, property?, limit?, page?, search?, interval?, compare=previous_period`
- Stats v2: `POST /api/v2/query` and `POST /api/stats/:domain/query` — `{site_id, period, metrics[], dimensions[≤1], filters[]}`
- Dimensions: `event:page|event:name|event:props:X|visit:source|visit:referrer|visit:utm_*|visit:country|visit:browser|visit:os|visit:device|visit:entry_page|visit:exit_page`
- Metrics: `visitors|visits|pageviews|events|bounce_rate|visit_duration|views_per_visit|time_on_page`
- Management: `/api/sites`, `/api/teams` (+ members), goals, funnels (+`/stats`), journey, segments, annotations, imports (CSV upload), search-console keywords, reports, billing plans/subscription/webhook, audit, admin, plugins v1, shared links (`/share/:slug`, `/api/share/:slug/stats/:endpoint`), favicon (`/api/sites/:id/icon`), suggestions (`/api/sites/:id/suggestions`)

## Project structure

```
rust version/
  Cargo.toml            # axum, tokio, sqlx(sqlite), argon2, lettre, minijinja, ...
  migrations/           # 001 core users → 008 plugins (squashed from 241 upstream)
  billing/              # plans_v5.json, paddle.pem (+sandbox) — upstream assets
  tracker/js/           # 1027 compiled tracker variants (built via node compile.js upstream)
  src/
    main.rs             # CLI: serve|migrate|reports-send|make-admin
    config.rs state.rs error.rs db.rs audit.rs google.rs
    auth/               # user, password(argon2), session, totp, api_key, token, oauth_google
    sites/              # site, membership(roles), team, invitation, shield,
                        # goals, funnels, segments, annotations, plugins, favicon
    billing/            # plans, gate(trials/limits/locks), webhook(RSA-SHA1)
    ingest/             # event_dto, validate(bots/DNT), ua, sessionize, writer, rate_limit
    stats/              # period, interval, filters(v1+v2), metrics, query, runner
    imports/            # csv importer (batched, backdated)
    mail/               # lettre + minijinja (verify/reset/invite/report)
    jobs/               # reports sender
    web/                # router + csrf + auth/sites/stats/ingest/dashboard/
                        # growth/plugins/billing/docs(Swagger UI)
  public/               # dashboard.html, index.html, app.css, openapi.json,
                        # world-map.json (pre-rendered), js/* (20 ES modules, no build)
  templates/auth/       # login.html, register.html (embedded)
  tests/                # stats_golden (3) + plugins_e2e (full HTTP suite)
  benches/              # ingest normalize (~4µs/event)
  Containerfile podman.md .env.example plan.md
```

## Development

```sh
cargo check            # fast typecheck
cargo test             # unit (2) + e2e (1) + golden (3)
cargo bench --bench stats_bench
```

Conventions that keep this codebase correct — please keep them:

1. **Writes: `execute` + re-`SELECT`, never `INSERT...RETURNING` on the pool.** sqlx 0.7 leaves `RETURNING` writes invisible to other pooled connections for ~50ms (proven by bisection). Exception: inside an explicit `tx` (commits at the end — safe).
2. **Session metrics go through the session-grain subquery** (`query.rs`): the events↔sessions join fans out, so bounce/duration must aggregate per-session first.
3. **`events.ts` is `'YYYY-MM-DD HH:MM:SS'`** (space, not ISO-`T`) — string range compares depend on it (see `005_events_ts.sql`).
4. **No IP is ever stored** (privacy by design); `country` rules exist but need a geo DB to fire.
5. **No new npm/JS build step** — dashboard JS is hand-written ES modules; the only generated assets are `tracker/js/*` and `public/world-map.json` (see P3/P5 notes in `plan.md`).

## Dashboard usage

Periods (realtime/day/7d/30d/month/12mo/custom range) + compare toggle persist in the URL, so every view is bookmarkable. Rows are click-to-filter; every card has a **more →** drilldown (search + pagination + CSV export). Filters accept `==, !=, =~, !~` with `;` (AND) and `|` (OR). Realtime badge polls every 5s. Public sites and `/share/:slug` links work without login.

## Differences from upstream Google Analytics

- Single SQLite file replaces Postgres + ClickHouse; background jobs are `tokio::spawn` + CLI timers instead of Oban; LiveView/React replaced by SSR shell + vanilla ES modules (no HMR, no d3 — canvas graph, static SVG map).
- GA4/UA bulk import: same record pipeline as CSV once Google tokens exist; out-of-scope hosted bits (Paddle checkout UI, Teams SSO) are API-shaped but not reimplemented.
- Token prefixes stay `plausible_*` / `plausible_site_*` for tracker/API compatibility; everything user-visible is Rust Analytix.

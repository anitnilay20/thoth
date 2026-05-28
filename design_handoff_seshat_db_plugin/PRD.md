# PRD — Seshat: Database Plugin for Thoth

**Status:** Draft v1.0
**Owner:** Plugin team
**Last updated:** May 2026
**Discussion:** [link to discussion]
**Design prototype:** `Seshat.html` (this bundle)

---

## 1. Summary

**Seshat** is a first-party Thoth plugin that turns the JSON viewer into a full database client — connect to any database, browse schemas, run queries, view results. It targets developers who already use Thoth for ad-hoc JSON inspection and want one app for *both* document inspection and database work, rather than juggling Thoth + DBeaver / TablePlus / pgcli.

Named after **Seshat**, the Egyptian deity of records, writing, and measurement — sister-figure to Thoth (god of wisdom). The naming continues Thoth's Egyptian-deity lineage and reinforces the plugin model: extra capabilities, same scribe.

---

## 2. Problem

Developers working with web/data products typically need **three** related but separate tools:

1. **JSON viewer** — for files, logs, API responses. (Thoth solves this.)
2. **SQL client** — for production/staging databases. (DBeaver, TablePlus, pgcli…)
3. **Document/KV browser** — for Mongo, Redis. (Compass, RedisInsight…)

Each tool reinvents the same UI primitives — connection manager, schema tree, query editor, results grid — and none of them are great at JSON. When the database returns a `jsonb` column, the SQL client renders it as a string blob and developers paste it into a *separate* tool (Thoth) to inspect. The handoff is friction.

Thoth's superpower is JSON. Putting a database client *inside* Thoth means:
- JSON / JSONB columns get the full tree viewer for free.
- One install, one keychain, one history surface.
- Power users get a unified command palette across files **and** databases.

---

## 3. Goals & Non-Goals

### Goals (v1.0)
- **G1.** Connect to ≥6 SQL engines (Postgres, MySQL, SQLite, SQL Server, BigQuery, Snowflake) and ≥3 non-SQL stores (MongoDB, Redis, ClickHouse).
- **G2.** Browse schemas: databases → schemas → tables/views → columns/indexes/FKs.
- **G3.** Write & run SQL with syntax highlighting, schema-aware autocomplete, and results grid.
- **G4.** Inspect any `json` / `jsonb` / `bson` cell with Thoth's native tree viewer.
- **G5.** Multi-tab workspace (≥10 concurrent open queries / tables).
- **G6.** Persistent query history & saved queries.
- **G7.** Command-palette-first navigation (⌘K).

### Stretch goals (v1.1+)
- ER diagram with curved FK edges, drag-to-rearrange.
- Natural-language → SQL via Claude.
- EXPLAIN ANALYZE flamegraph visualization.
- Import (CSV, JSON, dumps) / export (CSV, JSON, Parquet, SQL).
- SSH tunneling for connections.
- Inline foreign-key navigation in the results grid.

### Non-goals (v1.0)
- ❌ Database administration (user/role management, backups, replication setup).
- ❌ Schema migrations (use Liquibase / Flyway / Alembic).
- ❌ Performance profiling beyond per-query EXPLAIN.
- ❌ Mobile or web client. Desktop-only, native Thoth.
- ❌ Editing connections of teammates (single-user scope; future: workspaces).

---

## 4. Target Users

| Persona | Frequency | Primary jobs-to-be-done |
|---|---|---|
| **Backend engineer** | Daily | Spot-check prod data, debug failing queries, inspect jsonb columns |
| **Data analyst** | Daily | Ad-hoc analytics queries, copy results, hand off SQL |
| **Full-stack developer** | A few times/week | Switch between dev DB and prod read-replica, ER-diagram a new feature |
| **Platform engineer** | Weekly | Audit schemas, inspect Redis keys, check Mongo collections |

All four already use Thoth for JSON; **none** of them want a 12-button toolbar.

---

## 5. User Stories

1. **As a backend engineer**, when I get a production incident, I want to switch from `error.log.json` (open in Thoth) to running `SELECT * FROM users WHERE id = …` against prod-read-replica **without leaving the app**, so I can correlate the log to DB state in under a minute.

2. **As an analyst**, when I have a 50-line query saved in a file, I want to open it in Seshat, hit ⌘↵, and see the results with `mrr_usd` rendered as currency and `metadata` (jsonb) as a clickable JSON pill that opens Thoth's tree viewer.

3. **As a developer onboarding to a service**, I want to click "ER diagram" and see how `users`, `organizations`, `subscriptions`, `invoices` relate, so I don't have to read migration files.

4. **As a power user**, I want ⌘K to find *anything* — a table, a saved query, a connection — without remembering which sidebar tab it's under.

5. **As a careful operator**, I want a clear visual signal that I'm connected to **prod** vs **staging** (color, env chip, status dot) so I don't run a destructive UPDATE against the wrong database.

---

## 6. Functional Requirements

### 6.1 Connections
- **FR-C1.** New-connection wizard with engine picker (3-col grid, 13 engines) and credential form.
- **FR-C2.** Test-connection button with latency report.
- **FR-C3.** Connections grouped by `env` (prod / stage / dev) with a colored status stripe on each card.
- **FR-C4.** Status dot (connected / connecting / disconnected) on every connection chip, refreshed every 30s.
- **FR-C5.** Credentials stored in OS keychain (macOS Keychain, Windows Credential Manager, libsecret on Linux).
- **FR-C6.** Optional SSH tunnel config (host, port, key path). *Stretch.*

### 6.2 Schema browser
- **FR-S1.** Lazy tree: `database → schema → table | view | matview → columns | indexes | constraints | FKs`.
- **FR-S2.** Per-table row count shown inline (formatted `1.2M`, `48k`, etc.).
- **FR-S3.** Inline filter searches all tables and columns under the active connection.
- **FR-S4.** Hover row actions: open data, open structure, "ask AI about this table".
- **FR-S5.** Per-column icon: 🔑 PK, 🔗 FK, ◦ regular.

### 6.3 SQL editor
- **FR-E1.** Syntax highlighting for SQL dialect of the active connection.
- **FR-E2.** Schema-aware autocomplete: tables, columns, keywords. Popover shows match kind (COL / TBL / KW).
- **FR-E3.** Run shortcuts: ⌘↵ run, ⌘⇧E EXPLAIN, `/` opens AI prompt.
- **FR-E4.** Editor footer shows: line/col, engine label, indent setting, autocomplete status.
- **FR-E5.** Tab key inserts 2 spaces.
- **FR-E6.** Dirty indicator on tab when query has unsaved changes.

### 6.4 Results grid
- **FR-R1.** Sticky header with column name, type chip, and sort affordance.
- **FR-R2.** Row numbers, sticky-left.
- **FR-R3.** Type-aware cell rendering:
  - `bigint` / `integer` / `numeric` → right-aligned, mono, `--syn-number` color
  - `jsonb` / `json` → blue pill with click-to-preview popover (Thoth tree viewer)
  - `text` → mono, `--text` color
  - `date` / `timestamptz` → mono, `--syn-string` color
  - `NULL` → italic `NULL` in `--text-disabled`
  - Currency-tagged columns → `$` prefix, 2 decimals
  - FK columns → dotted-underline link, clickable to jump to referenced row
  - Enum-tagged columns → tinted pill chip
- **FR-R4.** Multi-tab below the grid: Results · Messages · Explain · Stats · Chart.
- **FR-R5.** Stats tab: per-numeric-column min/max/avg/sum + 16-bin sparkline histogram.
- **FR-R6.** Chart tab: configurable bar/line plot (axis pickers + save).
- **FR-R7.** Explain tab: ranked operator list with per-op timing bars.

### 6.5 Multi-tab workspace
- **FR-T1.** Tab kinds: `sql` (editor + results), `table` (data viewer), `structure` (DDL), `er` (diagram), `connections` (manager).
- **FR-T2.** Tab close (×), reorder via drag (stretch), `+` button to open a new SQL tab.
- **FR-T3.** ⌘T = new SQL tab. ⌘W = close current tab.
- **FR-T4.** Tab title shows tab-kind icon + filename / schema.table; dirty dot when unsaved.

### 6.6 Command palette (⌘K)
- **FR-P1.** Fuzzy search over: Actions · Connections · Tables (current conn) · Saved queries.
- **FR-P2.** Keyboard navigation (↑↓, ↵, Esc).
- **FR-P3.** Keyboard shortcuts appear inline next to each command (⌘↵, ⌘T, etc.).

### 6.7 Saved queries & history
- **FR-H1.** Saved queries: name, folder, attached connection, star.
- **FR-H2.** History: append-only, last 1,000 queries, with timestamp + connection + ms + row-count + status.
- **FR-H3.** History entry click → opens a new SQL tab pre-filled with that query.

### 6.8 ER diagram
- **FR-D1.** Curved bezier FK edges with arrowheads.
- **FR-D2.** Hover-highlight related tables.
- **FR-D3.** Zoom (50–200%) and pan.
- **FR-D4.** Auto-layout button + manual drag (stretch).
- **FR-D5.** Export to SVG.

### 6.9 AI assistance *(stretch but designed)*
- **FR-A1.** "Ask AI" overlay accessible via `/` key or toolbar.
- **FR-A2.** Prompt grounded in the active connection's schema (column types, indexes, FKs).
- **FR-A3.** Returns SQL + estimated cost; user clicks "Insert into editor".
- **FR-A4.** Suggestion chips for common templates ("count users by plan", "failed webhooks", etc.).

---

## 7. Non-Functional Requirements

| | Requirement |
|---|---|
| **Performance** | Open ≥1M-row table without UI freeze (virtualized grid). First 100 rows visible within 200 ms of result arrival. |
| **Memory** | Query result cap: 100 MB or 100k rows (whichever first), with "Load more" continuation. |
| **Cold start** | Plugin loads in <300 ms within Thoth. |
| **Theme** | Catppuccin Mocha (dark, default) + Latte (light), 100% match to Thoth's host theme. |
| **Density** | 22 px row height in compact, 28 px in comfortable. 4 px grid throughout. |
| **Accessibility** | Keyboard parity for every mouse action. Focus rings visible at all times. Color is never the *only* indicator of state. |
| **Privacy** | No telemetry without opt-in. Credentials never logged. |
| **Offline** | Plugin works fully offline once connection is open (no calls home). AI features are the only network dependency beyond the DB itself. |

---

## 8. Out of Scope (v1.0)

| | Why |
|---|---|
| Schema migrations / versioning | Use Liquibase, Flyway, Alembic, etc. |
| User/role administration | Workflow-specific; out of v1 scope. |
| Realtime change streams / CDC | Different mental model; future. |
| Spreadsheet-style cell editing with formulas | Plain edit-and-commit only. |
| Multi-user / shared workspaces | Single-user v1. |
| Mobile / web | Thoth is desktop. |

---

## 9. Success Metrics

| Metric | Baseline | 6-month target |
|---|---|---|
| Plugin install rate among Thoth users | 0% | **30%** |
| WAU among installs | n/a | **70%** |
| Queries run per WAU per week | n/a | **40+** |
| % of sessions that touch a jsonb column | n/a | **40%** *(unique value-prop)* |
| Median time from app-launch to first query result | n/a | **<10 s** |
| Crash-free sessions | n/a | **≥99.5%** |

---

## 10. Risks & Open Questions

| | Risk | Mitigation |
|---|---|---|
| **R1** | Driver matrix is large (13 engines). | Phase by tier: P0 = Postgres, MySQL, SQLite, Redis. P1 = SQL Server, Snowflake, BigQuery, Mongo. P2 = Oracle, ClickHouse, DuckDB, Cassandra, MariaDB. |
| **R2** | Long-running queries block UI. | All driver calls on tokio executor; UI marshals via channels. |
| **R3** | Credentials sync between dev/prod risks accidents. | Always show env color/chip; require typed confirmation on destructive ops against prod. |
| **R4** | Schema browser may DoS large DBs (500k tables). | Lazy load + server-side filter; never list all tables eagerly. |
| **R5** | AI hallucinations on schema. | Ground every prompt in actual `information_schema` snapshot; show grounding chip in the UI. |

**Open questions**
- Does Thoth's existing plugin system support persistent per-plugin state? (Need for saved queries, history.)
- Keychain abstraction: roll our own or use `keyring-rs`?
- Connection sync between machines: out of scope, but should we leave a hook?

---

## 11. Phasing

| Milestone | Scope | Target |
|---|---|---|
| **M0 — Spike** | Prove Thoth plugin API can host a Rust+egui side-panel that renders a Postgres result. | 2 weeks |
| **M1 — Walking skeleton** | Connections (Postgres only), Schema browser, SQL editor, Results grid, JSON-cell preview, history. | 6 weeks |
| **M2 — Multi-engine** | Add MySQL, SQLite, Redis. Keychain integration. Tabs. | 4 weeks |
| **M3 — Polish** | Command palette, structure view, saved queries, status bar, Tweaks-style settings. | 3 weeks |
| **M4 — Cloud + warehouses** | Snowflake, BigQuery, ClickHouse, SQL Server. | 4 weeks |
| **M5 — Stretch** | ER diagram, EXPLAIN viz, AI prompt, import/export. | 6 weeks |
| **GA** | Public release as bundled Thoth plugin. | ~6 months total |

---

## 12. Appendix

### A. Branding alternatives considered
- **Apis** — sacred bull deity; nice "API" pun
- **Ibis** — Thoth's sacred bird; short
- **Scribe** — what Thoth was
- **Papyrus** — medium of records
- **Codex** — book; least Egyptian

→ **Seshat** chosen for the literal goddess-of-records semantic.

### B. Reference designs studied
- DBeaver — gold standard for breadth; UI is dated.
- TablePlus — gold standard for visual restraint.
- VS Code — activity-bar pattern + command palette.
- Postico, Sequel Ace — Postgres/MySQL native UIs.
- DataGrip — power-user keyboard ergonomics.

### C. Glossary
- **Host** — Thoth (the JSON viewer the plugin runs inside).
- **Plugin pane** — the sidebar slot owned by Seshat.
- **Sub-nav** — Seshat's internal navigation among its 6 sections.
- **Workspace tab** — top-level tab in Thoth's main area.

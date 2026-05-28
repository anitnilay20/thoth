# Handoff: Seshat — Database Plugin for Thoth

> **What this is:** a complete spec for implementing the Seshat database plugin inside the Thoth desktop app.
> **What this is not:** shippable code. The HTML prototype is a fidelity reference; production should be Rust + egui to match the host.

---

## Overview

**Seshat** is a database-client plugin for **Thoth** (a Rust + egui JSON/NDJSON viewer). It adds: connections, schema browsing, SQL editor + results grid, table structure/DDL view, ER diagram, query history, and a command palette. Designed to feel native to Thoth — same Catppuccin theme, same 4px grid, same dense type — while being clearly identifiable as a *plugin* (not a swallowed sub-app).

See **`PRD.md`** for product context, scope, and phasing.

---

## About the design files

The files in this bundle are **design references created in HTML** — a clickable React prototype demonstrating intended look and behavior. They are **not** production code to copy directly.

**Production target:** Rust + egui (immediate-mode GUI), matching Thoth's host stack. The plugin should be loadable through Thoth's plugin API (see `docs/PLUGIN_SYSTEM.md` in the Thoth repo).

If you implement the prototype in a non-Rust environment (electron-style host, web companion), keep the same component decomposition described below and lift exact tokens from `thoth-tokens.css`.

---

## Fidelity

**High-fidelity.** Colors, type, spacing, and interactions are final. The grid layout, row heights (22px compact / 28px comfortable), 4px-grid paddings, exact hex values, font stack, and animation timings should all be reproduced. Where the prototype simulates an async behavior (e.g. "Run" briefly shows a 900ms spinner before fake results), production must implement the real async with the same UI states.

---

## Files in this bundle

| File | Purpose |
|---|---|
| `Seshat.html` | Main entry — loads all `.jsx` and `.css` |
| `app.jsx` | Root app, state orchestration, keyboard shortcuts, Tweaks |
| `chrome.jsx` | TitleBar, ActivityBar, TabBar, StatusBar, PluginSubNav (4 layouts) |
| `panes.jsx` | Sidebar panes: Connections, Schema, Saved, History, Import |
| `editor.jsx` | SQL editor, tokenizer, run bar, autocomplete preview, AI overlay |
| `results.jsx` | Results grid + Messages, Explain, Stats, Chart tabs; cell-preview popover |
| `views.jsx` | TableDataView, TableStructureView, ERDiagramView, ConnectionManagerView, NewConnectionDialog |
| `palette.jsx` | Command palette (⌘K) |
| `data.js` | Mock data: connections, schemas, sample query results, history, ER nodes |
| `thoth-tokens.css` | Design tokens — paste-this-in CSS variables for Mocha + Latte |
| `tweaks-panel.jsx` | Tweaks shell (used for the live demo only — not production) |
| `assets/thoth_icon_256.png` | Host icon — used in the plugin title bar |
| `PRD.md` | Product requirements doc |

---

## Architecture

### Top-level layout

```
┌─ TitleBar (32px) ───────────────────────────────────────────────────┐
│ 🪶 Thoth › 🧩 Seshat plugin · 🟢 prod-postgres   [⌘K] [- □ ×]       │
├─ ActivityBar (52px) ─┬─ Sidebar (resizable 220-420px) ─┬─ Workspace ┤
│ Thoth icons (dim)    │ Plugin sub-nav (one of 4 modes) │ TabBar     │
│  ─ Recent            │   A — inner rail (40px col)     ├────────────┤
│  ─ Clipboard         │   B — top tabs                  │ Editor     │
│  ─ Search            │   C — dropdown switcher         │  (or table │
│ ─ Divider ─          │   D — hidden                    │   / DDL /  │
│ 🧩 Seshat (active)   │ Pane content                    │   ER /     │
│                      │   (schema / conns / saved /     │   manager) │
│                      │    history / er / import)       ├────────────┤
│                      │                                 │ Results    │
├──────────────────────┴─────────────────────────────────┴────────────┤
│ StatusBar (24px): 🟢 conn · env · latency · ms · rows · UTF-8 · LF  │
└─────────────────────────────────────────────────────────────────────┘
```

### Component tree

```
App
├── TitleBar
├── ActivityBar          ← Thoth host icons + ONE plugin button
├── Sidebar (when open)
│   ├── ThothHostPane    ← when a Thoth host icon is active
│   └── PluginSubNav + active pane
│       ├── (A) SubRail | (B) TopTabs | (C) SubDropdown | (D) none
│       ├── ConnectionsPane
│       ├── SchemaPane          ← default
│       ├── SavedPane
│       ├── HistoryPane
│       ├── ERPane (teaser)
│       └── ImportPane
├── Main workspace
│   ├── TabBar
│   └── Active tab body
│       ├── SqlEditor + ResultsPanel (kind: 'sql')
│       ├── TableDataView (kind: 'table')
│       ├── TableStructureView (kind: 'structure')
│       ├── ERDiagramView (kind: 'er')
│       └── ConnectionManagerView (kind: 'connections')
├── StatusBar
├── Overlays
│   ├── CommandPalette (⌘K)
│   ├── AiPromptOverlay (/)
│   └── NewConnectionDialog
└── Tweaks (dev-only)
```

### State (production should mirror this)

```
activeConnId      : string | null            -- which saved connection is current
pane              : string | null            -- which sidebar pane is open ('schema', 'thoth-recent', etc.)
tabs              : Array<TabState>          -- open workspace tabs
activeTabId       : string
executing         : bool                     -- is the current SQL tab running?
queryMs           : number | null            -- timing of last result
paletteOpen       : bool
aiOpen            : bool
newConnOpen       : bool

// Tweaks (persisted to disk)
theme             : 'dark' | 'light'
density           : 'compact' | 'comfortable'
showResults       : bool
sidebarWidth      : number (220..420)
sidebarCollapsed  : bool
subnav            : 'sub-rail' | 'top-tabs' | 'dropdown' | 'minimal'
```

```
TabState =
  | { id, kind: 'sql',        title, query, hasRun, dirty }
  | { id, kind: 'table',      title, conn, schema, tableName }
  | { id, kind: 'structure',  title, conn, schema, tableName }
  | { id, kind: 'er',         title, conn }
  | { id, kind: 'connections',title }
```

---

## Design tokens

All values come from `thoth-tokens.css`. **Do not invent new colors** — use these `var(--*)` names or copy hex from the table.

### Colors — Mocha (dark, default)

| Token | Hex | Use |
|---|---|---|
| `--base` | `#1e1e2e` | Main editor / results background |
| `--mantle` | `#181825` | Panels, sidebar, toolbar |
| `--crust` | `#11111b` | Status bar, title bar, activity bar |
| `--surface0` | `#313244` | Widget backgrounds, hover, cards |
| `--surface1` | `#45475a` | Widget borders, indent guides |
| `--surface2` | `#585b70` | Widget pressed/selected |
| `--text` | `#cdd6f4` | Primary text |
| `--overlay1` | `#7f849c` | Secondary text |
| `--overlay2` | `#9399b2` | Tertiary text, header chrome |
| `--text-disabled` | `#6c7086` | Disabled |
| `--primary` | `#cba6f7` | Mauve — plugin accent, run button, palette highlight |
| `--secondary` | `#b4befe` | Lavender — function names |
| `--accent` | `#89b4fa` | Blue — selection stroke, FK links |
| `--syn-key` | `#89b4fa` | JSON keys, identifiers |
| `--syn-string` | `#a6e3a1` | Strings, table icons, dates |
| `--syn-number` | `#fab387` | Numbers, currency |
| `--syn-boolean` | `#cba6f7` | Booleans |
| `--syn-null` | `#7f849c` | NULL |
| `--syn-bracket` | `#9399b2` | Punct, brackets |
| `--success` | `#a6e3a1` | OK status, "connected" dot |
| `--warning` | `#f9e2af` | PK glyph, slow query bars |
| `--error` | `#f38ba8` | Error state, prod env tag |
| `--info` | `#74c7ec` | FK glyph, JSON pill |
| `--selection-bg` | `rgba(127,132,156,0.30)` | Selected row |
| `--selection-stroke` | `#89b4fa` | Selected row left border |
| `--sidebar-hover` | `rgba(108,112,134,0.20)` | Sidebar row hover |
| `--indent-guide` | `#45475a` | 1px tree indent line |

### Colors — Latte (light)
See `thoth-tokens.css` for the full Latte palette. Activated by setting `data-theme="light"` on `<html>`.

### Typography

```
--font-ui   = -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Ubuntu,
              "Helvetica Neue", Arial, sans-serif
--font-mono = "JetBrains Mono", "Fira Code", "Cascadia Code", "SF Mono",
              "Consolas", "Monaco", monospace
```

| Token | Size | Use |
|---|---|---|
| `--fs-xs` | 11px | Sidebar headers, micro labels, kbd hints |
| `--fs-sm` | 12px | Status bar, captions, toolbar buttons |
| `--fs-md` | 13px | JSON content, body, editor, results cells |
| `--fs-lg` | 14px | Section headers, dialog body |
| `--fs-xl` | 16px | Card titles, table-detail header |
| `--fs-2xl` | 20px | Connection-manager page title |
| `--fs-3xl` | 28px | Reserved (hero/marketing only) |

Line heights: `--lh-tight: 1.2`, `--lh-body: 1.45`.
Weights: 400 / 500 / 600 / 700.

**Sidebar section headers** use a distinctive style: 11px / 700 / `text-transform: uppercase` / `letter-spacing: 0.06em` / color `--sidebar-header` (`#9399b2` dark, `#7c7f93` light). Available as the class `.t-section`.

### Spacing — strict 4px grid

```
--space-1 = 4px    --space-5 = 24px
--space-2 = 8px    --space-6 = 32px
--space-3 = 12px   --space-7 = 48px
--space-4 = 16px
```

### Component heights

```
--h-row       = 22px    JSON row, sidebar row, tree row (compact)
--h-status    = 24px    Status bar
--h-menu      = 28px    Buttons, context menu items
--h-titlebar  = 32px    Title bar
--h-toolbar   = 40px    Toolbar / editor toolbar
```

Results-grid row height is 30px in compact mode (results need slightly more breathing room than the JSON tree). In comfortable density, +6px.

### Radius

```
--radius-sm = 4px    Buttons, tags, dropdowns, toolbar pills, table row
--radius-md = 8px    Settings dialogs, modals (connection dialog, AI overlay)
--radius-lg = 12px   Cards (connection cards)
```

Window chrome (title bar, status bar, activity bar) uses **0px** — full-bleed.

### Shadows

```
--shadow-menu  = 0 4px 12px rgba(0, 0, 0, 0.50)  Context menus, popovers, autocomplete
--shadow-modal = 0 8px 24px rgba(0, 0, 0, 0.80)  Modal dialogs
```

No shadows on cards or buttons. Elevation is communicated via background steps.

### Motion

| Token | Duration | Use |
|---|---|---|
| `--d-fast` | 100ms | Hover, color transitions |
| `--d-base` | 150ms | Expand/collapse, tab switch |
| `--d-slow` | 200ms | Panel slide, sidebar collapse |

Easing: `--ease-out = cubic-bezier(0.2, 0.8, 0.2, 1)`. No bouncy springs.

---

## Screens

### 1. TitleBar (`chrome.jsx::TitleBar`)
- **Height:** 32px. Background `--crust`. Bottom border `--mantle`.
- **Left:** 16x16 Thoth icon (3px radius) + "Thoth" (13/600/`--text`) + chevron `›` + plugin chip + connection chip.
- **Plugin chip:** padding `2px 8px`, radius 4px, background `linear-gradient(135deg, rgba(203,166,247,0.18), rgba(180,190,254,0.10))`, border `1px solid rgba(203,166,247,0.35)`. Contents: 10px puzzle-piece icon (`--primary`) + "Seshat" (11/600/`--primary`) + "PLUGIN" (10px/uppercase/0.06em/`--overlay1`).
- **Connection chip (when connected):** `--surface0` bg, `--surface1` border, 12/500 text. Contents: status dot (8px circle, color = `--success` connected / `--warning` connecting / `--text-disabled` offline; box-shadow `0 0 6px rgba(166,227,161,0.55)` when connected) + engine glyph (18px chip with 2-letter monogram in engine color) + connection name + caret. Click opens command palette filtered to Connections.
- **Right:** "Go to anything" palette hint button (mono-styled, with ⌘K kbd badge) + window controls (minus / square / x, 13/11/13px, `--overlay1`).

### 2. ActivityBar (`chrome.jsx::ActivityBar`)
- **Width:** 52px. Background `--crust`. Right border `--mantle`.
- **Top section — Thoth host:** 3 dimmed icons (folders / clipboard-text / magnifying-glass). 36px tall each, 16px glyph, color `--text-disabled` (hover → `--overlay2`). Active = `--primary`. Top-border divider after.
- **Plugin label:** small "PLUGINS" caption (8px/700/uppercase/0.08em/`--overlay1`).
- **Plugin button:** 40px tall, margin 0 8px, radius 6px. When active: background `linear-gradient(135deg, rgba(203,166,247,0.20), rgba(180,190,254,0.10))` + 1px border `rgba(203,166,247,0.45)` + 2px left stripe `--primary` (-8px from button). Glyph: `database`, 20px, color follows active state.
- **Plugin name label:** below the button, 8px/600/uppercase/0.08em, `--primary` when active else `--text-disabled`.
- **Bottom:** version chip ("v1.0.0", 9px mono `--text-disabled`) above a top-bordered cap. Below that: gear settings button (18px, `--overlay1`).

### 3. Plugin sub-nav (`chrome.jsx::PluginSubNav`)
Four mutually-exclusive layouts, picked via tweak. **Default: `sub-rail`**.

#### A. SubRail (default)
- Inner column inside the sidebar, width 40px, background `--crust`, right border `--surface0`.
- 6 buttons stacked vertically, 36px tall. Active = `--primary` glyph + `--surface0` bg + 2px left stripe `--primary`.

#### B. TopTabs
- Horizontal strip at top of sidebar, 32px tall, background `--crust`. 6 buttons of equal flex, each ~10px font, glyph + (no label) to save space. Active = `--primary` glyph + 2px top stripe `--primary`.

#### C. SubDropdown
- Single 32-tall button at top, padding 8 around. Crust bg, `--surface0` border, 4px radius. Inside: 12px glyph (`--primary`) + label (12/500/`--text`) + caret (10px right). Click opens absolute-positioned menu below with all 6 entries.

#### D. Minimal
- Nothing rendered. User navigates via ⌘K and workspace tabs.

### 4. Sidebar panes

All panes share:
- `PaneHeader` — `padding: 10px 14px 6px`, bottom border `--surface0`. Title styled as `.t-section`. Optional right-side action button + kbd hint.
- `SearchField` — margin 8 10, padding 4 8, height 24, `--crust` bg + `--surface0` 1px border + 4px radius. 11px search glyph + transparent input.
- `PaneRow` — 24px tall by default, padding-left = `10 + indent*14`px, 13px text, hover bg `--sidebar-hover`. Selected: `--selection-bg` + 2px left stripe `--selection-stroke`.

#### ConnectionsPane
- Grouped by environment: `prod` (red header), `stage` (yellow), `dev` (green). Headers: 10px mono / 700 / uppercase / 0.1em. Row count appears right of the divider line.
- Each row: 32px tall. Engine glyph (18px) + status-dot (8px circle) + name (12/500) + host:port (10px mono `--overlay1`) + latency (10px mono, right-aligned).
- "+ New connection" dashed-border CTA at the bottom.

#### SchemaPane
- Header shows `Schema · {db}` + refresh icon, kbd hint `⌘P`.
- Tree: schemas (folder icon, caret) → tables (kind icon, name, row count, hover-actions) → columns (PK 🔑 / FK 🔗 / regular ◦, mono-styled name and type).
- Hover actions on table rows: 18x18px micro-buttons — "table" (open data), "list-numbers" (structure), "sparkle" (ask AI). Default `--overlay1`, hover `--surface0` bg.

#### SavedPane
- Folder rows + nested query rows (26px tall). Starred queries get a 11px star glyph `--warning`.

#### HistoryPane
- Each entry: 8px 14px padding, bottom border `--surface0`. Top line: status icon (check-circle `--success` / x-circle `--error`) + conn (11px mono) + time (10px right-aligned). Middle: 2-line clamped mono-formatted query. Bottom: ms + rows.

#### ImportPane
- 5 action cards, 10px padding, `--surface0` bg, 6px radius. Icon (18px) + title (12/500) + subtitle (11/`--overlay1`) + right caret.

#### ThothHostPane
- Renders when active pane id starts with `thoth-`. Top section: 18px Thoth icon chip + "THOTH HOST" caption + "Back to plugin" button (`--primary` text). Body: feature label + description. For `thoth-recent`, also lists 4 mock recent JSON file rows. Footer card: 1px dashed `--surface2` border, info icon, "You're viewing a Thoth host feature" reminder.

### 5. TabBar (`chrome.jsx::TabBar`)
- Height 34px. Background `--crust`. Bottom border `--surface0`.
- Each tab: padding `0 12px`, 12/500 text. Tab icon (kind-specific) + title + (dirty dot if dirty) + 11px close X (opacity 0.5 idle, 1 on hover with `--surface0` bg). Active tab: `--base` bg + 1px top stripe `--primary` + `--text` color.
- "+" button at the end opens a new SQL tab.

### 6. SQL editor (`editor.jsx::SqlEditor`)
- **Run bar (40px):** Run button (mauve fill, ⌘↵ kbd in dark badge), Explain button (`--surface0` bg with chart-bar icon, ⌘⇧E kbd), divider, "Ask AI" button (mauve text + border, `/` kbd), spacer, Save / Format / Share micro-buttons (transparent + `--surface0` border).
- **Gutter (46px):** right-bordered, `--text-disabled` line numbers, 12px mono.
- **Editor:** transparent textarea overlaid on a syntax-highlighted `<pre>`. Tab inserts 2 spaces.
- **Autocomplete popover:** absolute bottom-right of editor area, 280px wide. Header: 10px uppercase "Suggestions · matching {tail}". Each row: kind chip (COL / TBL / KW with appropriate color), highlighted match prefix (`--warning`), suffix, right-aligned type tag.
- **Footer (22px):** mono 11px — line/col, engine, indent setting, autocomplete status indicator (`●` `--success`).

### 7. Results panel (`results.jsx::ResultsPanel`)
- **Tab bar (30px):** Results / Messages / Explain / Stats / Chart. Same active treatment as workspace tabs. Right side: query timing (`{ms} ms` `--success` + `{rows} rows`).
- **Grid:** sticky header (28px, `--mantle` bg, 11/600 text + 9px mono type tags), row numbers (44px, sticky-left, `--mantle` bg). Rows 30px, alternating row stripe `rgba(255,255,255,0.012)`, hover `--sidebar-hover`, selected `--selection-bg` + left stripe.
- **Cell types:**
  - **JSON:** blue pill with brackets-curly icon. Click opens popover (380px, `--mantle` bg, `--surface1` border, 8px radius, `--shadow-menu`). Pretty-printed in 12px mono.
  - **FK:** dotted-underline link (`--info`), click jumps to referenced row.
  - **Currency:** `$` + 2-decimal format, `--syn-number`.
  - **Tier enum:** rounded pill chip (radius 10), tinted by value.
  - **NULL:** italic "NULL" in `--text-disabled`.
- **EXPLAIN tab:** stat row at top (Total / Planning / Execution / Buffers / Plan). Then a card with operator rows: name (360px), rows (96px right-aligned), cost (100px), bar (flex, color = `--success` <50ms / `--info` <100ms / `--warning` ≥100ms), ms right-aligned.
- **Stats tab:** auto-fill grid of 220px-min cards per numeric column. Card: column name + sum (18/600 mono) + 2-col min/max/avg/n + 28-tall 16-bin sparkline (`--secondary`, opacity gradient).
- **Chart tab:** 4 config pills at top (X / Y / Group / Type). Then horizontal bar rows: label (180px) + bar (flex, mauve→lavender gradient) + value (90px right-aligned).

### 8. TableDataView / TableStructureView (`views.jsx`)
**Data view:** Toolbar with table icon + breadcrumb (`schema.table_name`) + row count + WHERE / ORDER BY filter pills + Add row / Refresh / Export ghost buttons. Reuses `ResultsPanel` for the grid.

**Structure view:** 14/20px header with 40px icon chip + breadcrumb + 4 stat blocks (Rows / Columns / Indexes / Size). Tab strip below: Columns / Indexes / Constraints / Foreign Keys / DDL / Triggers. Active tab gets a 2px bottom stripe `--primary`.

- **Columns tab:** 7-column grid with PK/FK glyph, name, type, nullable flag, default, constraint badges (PRIMARY KEY / UNIQUE / FK chips), edit icon. Header in `--surface0` bg, 10/700/uppercase.
- **DDL tab:** mono code in `--mantle` card with syntax highlighting (same tokenizer as editor).

### 9. ERDiagramView (`views.jsx::ERDiagramView`)
- Toolbar: graph icon + "ER Diagram" label + node/edge count + zoom pill (- 100% +) + Auto-layout / SVG buttons.
- Canvas: 1080×820 SVG, background = radial-gradient dot grid (`var(--surface0)` 1px dots on 24px cells).
- Edges: Cubic bezier `M{x1},{y1} C{mx},{y1} {mx},{y2} {x2},{y2}`, stroke `--surface2` (1.5px) → `--primary` (2px) on hover, arrowhead marker.
- Nodes: 240px wide, `--mantle` bg, 6px radius, `--surface1` border. Header (32px): `--surface0` bg + table icon + table name (12 mono/600). Rows (22px each): PK 🔑 / FK 🔗 / regular ◦ glyph + column name (11 mono).

### 10. ConnectionManagerView
Auto-fill grid (min 280px), 12px gap.
- **Card:** 16px padding, 12px radius, `--mantle` bg, `--surface0` border. 3px top stripe in connection's env color. Engine glyph (32px) + name (14/600) + engine label + env (11/`--overlay1`) + status dot. Mono footer: host:port / db / user + TLS chip. Bottom row: latency + "Open →" (`--info`).
- **Add card:** dashed-border CTA, plus-circle (24px `--primary`) + "New connection" + "13 engines supported".

### 11. NewConnectionDialog
- 640px modal, 10px radius, `--shadow-modal`. Two-step.
- **Step 0:** 3-col engine grid. Each card: 12px padding, `--surface0` bg, `--surface1` border (hover = engine color). Click → step 1.
- **Step 1:** form fields stacked. Field label: 11/600/uppercase/0.06em/`--overlay1`. Input: `--base` bg, `--surface1` border, 30px height, mono font for technical fields. SSL toggle. Advanced details disclosure.
- **Footer:** Back, Test connection (with state machine: pending = spinner / ok = `--success` border + check), Cancel, Connect (mauve fill).

### 12. CommandPalette (`palette.jsx`)
- 680×540 modal, 10px radius, `--shadow-modal`. Top: 14px search icon + input + ESC kbd hint.
- Grouped sections: **Actions / Connections / Tables / Saved queries**. Group header 10/700/uppercase/0.08em.
- Row: 6/16 padding. 14px icon + label + subtitle (11/`--overlay1`) + optional right element (status dot) + kbd hint. Selected (sel state or hover): `--surface0` bg + 2px left stripe `--primary`.
- Footer (28px): kbd legend (↑↓ navigate · ↵ select) + "Powered by Seshat" right-aligned.

### 13. AiPromptOverlay (`editor.jsx`)
- 720×auto modal. Top: 32px gradient icon (`linear-gradient(135deg, --primary, --secondary)`) + "Ask AI to write SQL" title + grounding subtitle + Esc kbd.
- Textarea (min 60 height) + suggestion chip cluster (12px radius pills, `--surface0` bg).
- When thinking: spinner + "Reading schema · planning joins · generating SQL…".
- When generated: success check + meta line ("Generated N lines · K joins · estimated ms") + highlighted SQL in `--base` code block.
- Footer: Cancel · Generate (mauve) or Insert into editor (green) when done.

### 14. StatusBar (`chrome.jsx::StatusBar`)
- Height 24px, `--crust` bg, top border `--surface0`. 12px text.
- Left-to-right: status dot + conn name + host:port + env + lightning icon + latency + (last query ms `--success`) + (row count) + (selection summary `--info`) + spacer + "UTF-8" + "LF" + Results-panel toggle button (caret-up/down + "Results" label).
- Separators: `│` `--overlay1` at 35% opacity.

---

## Interactions & behaviors

### Keyboard shortcuts (global)

| Shortcut | Action |
|---|---|
| `⌘K` / `⌘P` | Open command palette |
| `⌘T` | New SQL editor tab |
| `⌘N` | New connection dialog |
| `⌘W` | Close current tab |
| `⌘↵` | Run current query (when SQL tab is active) |
| `⌘⇧E` | EXPLAIN current query |
| `⌘⇧T` | Toggle light / dark theme |
| `/` | Open AI prompt (in editor) |
| `Tab` | Insert 2 spaces (in editor) |
| `↑ ↓` / `↵` / `Esc` | Palette / dialog navigation |

### Connection chip click → command palette filtered to "Connections" group.

### Schema tree
- Click a schema folder → expand/collapse.
- Click a table name → expand to columns (if loaded) OR open data tab.
- Hover row → reveal 3 action micro-buttons.
- Each action opens a new workspace tab and switches focus to it.

### Editor
- Type → autocomplete popover appears after 2 chars of a word-prefix.
- `⌘↵` → run query, opens results panel below (if not already open), grid streams in.
- `/` at empty editor → AI overlay opens with focus.
- Selecting text and pressing run executes selection (stretch).

### Results grid
- Click cell → select row, status bar shows row summary.
- Click JSON pill → cell-preview popover (anchored to cell).
- Click FK chip → toast: "Would jump to {table}.{col} = {value}". (Production: open new tab pre-filtered to that row.)
- Click column header → sort cycle: asc → desc → none.

### ER diagram
- Hover a table → its FK edges highlight `--primary` with 2px stroke.
- Zoom pill: 50–200% in 10% steps.
- Pan via background drag (stretch).

### Async patterns

| Action | Pending UI | Completed UI |
|---|---|---|
| Run query | Run button → "Running…" + spinner; results panel shows centered spinner + indeterminate progress bar | Results stream in; status bar shows `{ms} ms` |
| Test connection | Button shows spinner + "Testing…" | Border → `--success`, label → "Connection OK · 38 ms" |
| AI generate | "Reading schema · planning joins · generating SQL…" with spinner | SQL appears in highlighted code block + meta line + "Insert into editor" CTA |

All async should be cancellable (Escape in dialogs; stretch: cancel-running-query in toolbar).

---

## Implementation notes

### Driver matrix (Rust)
- **Postgres:** `tokio-postgres` + `sqlx` for schema introspection.
- **MySQL / MariaDB:** `mysql_async`.
- **SQLite:** `rusqlite` (with `bundled` feature).
- **SQL Server:** `tiberius`.
- **Snowflake:** community crate or REST API.
- **BigQuery:** REST API via `gcp-bigquery-client`.
- **ClickHouse:** `clickhouse-rs`.
- **DuckDB:** `duckdb-rs`.
- **MongoDB:** `mongodb` crate.
- **Redis:** `redis-rs`.
- **Cassandra:** `scylla` (CQL).
- **Oracle:** `oracle` crate.

Wrap each driver behind a `trait DbAdapter` so the UI talks to one interface regardless of engine.

### Schema introspection
Cache `information_schema` (or engine equivalent) per connection. Invalidate on user-triggered refresh; never poll. Lazy-load below the schema level — never list 500k tables eagerly.

### Result paging
Cap initial render at 100 rows. Show "Load more" CTA. Virtualize the grid above 1k rows (egui's `TableBuilder::vertical_scroll_area` with row-height = const).

### Credentials
Use `keyring-rs` for OS keychain integration. Never serialize passwords to plain JSON. Store the *connection profile* (host, port, db, user) on disk; password key in keychain.

### Plugin packaging
Check `docs/PLUGIN_SYSTEM.md` in the Thoth repo for the actual plugin API. Likely entry points:
- `register_plugin()` → registers Seshat with the host
- `render(ctx, frame)` → called when the plugin's pane is active
- `on_command(cmd)` → palette commands from the host

### Embedding Thoth's JSON tree viewer in result cells
This is the killer feature — when a `jsonb` cell is clicked, the popover should render the value with **Thoth's existing JSON tree component**, not a re-implementation. Expose `thoth::ui::JsonTree` (or equivalent) as a public API the plugin can call.

---

## Assets

| Asset | Source | Use |
|---|---|---|
| `assets/thoth_icon_256.png` | Thoth repo (`/assets/`) | Host icon in title bar |

No new brand assets required for the plugin itself. The plugin chip uses Phosphor's `puzzle-piece` glyph to signal "plugin".

Engine glyphs are 18px rounded chips with the 2-letter monogram (PG / MY / SF / etc.) in the engine's accent color — generated, not files.

---

## Iconography

**Phosphor Icons, regular weight, via CDN:**
```html
<script src="https://unpkg.com/@phosphor-icons/web@2.1.1"></script>
<i class="ph ph-folder-open"></i>
```

For Rust/egui: pull SVGs from `@phosphor-icons/core/assets/regular/{name}.svg` and embed at build time. Maintain a 1:1 mapping with the HTML prototype.

**Sizes:** 10–11px for inline (kbd badges, status chips), 12–14px for actions (toolbar, palette rows), 16–18px for navigation (rail), 20px for hero glyphs (rail when active).

**Color:** match text context (`--text` for default, `--overlay1` for secondary, semantic color for status).

**No emoji.** No new SVG icons. No mixing of Phosphor weights.

---

## Open questions for engineering

1. Does Thoth's plugin API expose its **JSON tree viewer** as a reusable component? If not, can it?
2. Is there a **theme bridge** so the plugin auto-tracks Thoth's Mocha/Latte preference? (Otherwise: subscribe to a host event.)
3. Should the plugin store **history & saved queries** in its own SQLite DB next to Thoth's config, or via a host-provided KV API?
4. **Performance:** what's the largest single result the prototype must handle? (PRD says 1M rows / 100MB — confirm with stakeholders.)
5. **Driver licensing:** Oracle's official driver has restrictive licensing; do we ship community or skip Oracle in v1?

---

## Done definition (v1.0)

- [ ] All 6 sub-sections render with real data from at least Postgres + SQLite.
- [ ] Multi-tab workspace persists across restarts.
- [ ] Command palette covers Actions + Connections + Tables + Saved.
- [ ] Editor: tokenizer + autocomplete + ⌘↵ run.
- [ ] Results grid: all cell types + jsonb popover using Thoth's JSON tree.
- [ ] Theme toggles in lockstep with Thoth's host theme.
- [ ] No 60fps drop on 1M-row scroll.
- [ ] Keychain integration on all three OSes.
- [ ] Crash-free for 100k synthetic queries in CI.

---

*This handoff supersedes any ad-hoc Slack screenshots. When in doubt, the prototype HTML wins on visuals; the PRD wins on scope. Engineering's discretion on architecture.*

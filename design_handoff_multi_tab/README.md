# Handoff: Multi-tab Support for Thoth

## Overview

This package designs **multi-tab editing with VSCode-style split panes** for Thoth — the Rust + egui JSON/NDJSON viewer. The current app shows one file at a time; this design lets a user open many files **and plugin panels** as tabs, drag them between groups, and drop them on the edge of any pane to split the workspace into independently-scrollable groups (horizontal and vertical, nested arbitrarily). It mirrors the behavior of VSCode's editor groups so the mental model is immediately familiar.

The design covers:
- Tabs for both files (JSON / NDJSON) and plugins (Welcome, Settings, Schema Validator, Diff Viewer, JSONPath).
- Drag-to-reorder within a tab strip, drag-between-groups, and drag-to-edge to split a group.
- Live drop-zone overlay (5 zones: 4 edges + center) with a label that names the action (`Split Right`, `Add to Group`, etc.).
- Right-click context menu with the full set of close/split/pin actions.
- Pinned tabs (auto-sorted to the left of the strip) and modified indicators (dot replaces × until hover).
- A reducer-based split tree that auto-collapses empty groups.
- Resizable splitters between sibling panes.

## About the Design Files

Everything in this folder is a **design reference written in HTML + React** — a working prototype demonstrating intended visuals, behavior, and state transitions. **Do not ship the HTML.** The task is to recreate this design in Thoth's actual codebase (Rust + egui — see `https://github.com/anitnilay20/thoth`) using egui's immediate-mode idioms, the existing `ThemeColors` module, and the `src/components/common/` library.

The React `useReducer` + tree model documented below maps cleanly onto a `pub struct EditorLayout` held in app state; the drag/drop interactions translate to egui's `Response::dragged()` / `Response::hovered()` / `egui::DragValue` patterns. Sections labeled **Implementation note** call out specific egui translations.

## Fidelity

**High-fidelity.** Final Catppuccin Mocha + Latte palettes, exact spacing on the 4 px grid, real interactions (drag-and-drop, edge detection, splitter resize, context menu, animations). Use these values verbatim. The one substitution is **Phosphor Icons** in the prototype — the desktop app already maps these to its own glyph set (see the design system's Iconography mapping). Match by semantic name, not by font.

## File / View Layout

The app is one window. From top to bottom, full-width:

```
┌──────────────────────────────────────────────────────────────────────┐
│ Title bar              32 px   crust                                  │
├──┬───────────────────────────────────────────────────────────────────┤
│A │  Editor area                                                       │
│c │  ┌──────────────┬───────────────┐                                  │
│t │  │ Tab strip 35 │ Tab strip 35  │   ← independent strips per group │
│i │  ├──────────────┼───────────────┤                                  │
│v │  │              │               │                                  │
│i │  │  Group 1     │   Group 2     │   ← split tree, any depth        │
│t │  │   content    │    content    │                                  │
│y │  │              │               │                                  │
├──┴──┴──────────────┴───────────────┴───────────────────────────────┐  │
│ Status bar             24 px   crust                                 │
└──────────────────────────────────────────────────────────────────────┘
   48 px
```

### Title bar — 32 px

- Background `--crust` (`#11111b` dark / `#dce0e8` light)
- 16 px Thoth icon + `Thoth — <active tab title>` in `--fs-md` / weight 500
- Window controls (minus / square / x) right-aligned, `--overlay1`
- Border-bottom 1 px `--mantle`

### Activity bar — 48 px wide, left side

A vertical rail with one button per opener (Files, Welcome, Schema Validator, Diff, JSONPath, Settings) and a theme toggle pinned to the bottom.

- Background `--mantle`, border-right 1 px `--surface0`
- Each button is 32 × 32, 4 px radius, 18 px Phosphor icon at `--overlay2`
- Hover: `--sidebar-hover` background, icon goes to `--text`
- The **Files** button reveals a flyout (200 px wide, `--surface0`, 4 px radius, `--shadow-menu`) listing every known file with its type tag — clicking opens that file in the focused group.

### Editor area

The editor area is a **recursive split tree**. Every leaf is a **Group** (its own tab strip + active tab's content + drop overlay). Every internal node is a **Split** (a `row` or `col` flex with N children and explicit size ratios). Splitters between siblings are 4 px wide, `--surface0`, with `--selection-stroke` on hover; dragging them adjusts the ratio.

#### Tab strip — 35 px

- Background `--mantle`, border-bottom 1 px `--surface0`
- Active tab: `--base` background, 2 px top accent strip in `--<accent>` (mauve for plugins, blue for files), 0 vertical padding on the strip, label `--text`
- Inactive tab: `--mantle` background, label `--overlay1`, hover lifts to `--surface0`
- Pinned tab: rotated push-pin icon at 11 px, `--overlay2`
- Tab structure: `[pin?] [type icon 14 px] [label] [close × / modified •]`
- Close button: `var(--surface1)` on hover, 18 × 18, 3 px radius
- Modified state: solid 8 px dot `--text` replaces the × until row hover
- Active tab in **unfocused** group: top accent strip becomes `--surface2` (dim) instead of the accent color

#### Drop overlay (only visible while a tab is being dragged)

When any tab is being dragged, every group pane shows a transparent capture surface over its content. As the cursor moves, one of five zones is highlighted:

| Zone   | Rule                                                        | Action                       |
| ------ | ----------------------------------------------------------- | ---------------------------- |
| left   | `< 22%` from left edge AND closest edge                      | Split this group leftward    |
| right  | `> 78%` from left edge AND closest edge                      | Split this group rightward   |
| top    | `< 22%` from top AND closest edge                            | Split this group upward      |
| bottom | `> 78%` from top AND closest edge                            | Split this group downward    |
| center | inside the 22% inset on all sides                            | Add to this group's tab strip |

Highlight rendering:
- Background `rgba(203,166,247,0.16)` (mauve @ 16%)
- 2 px dashed border `--primary`
- Centered pill label (`Split Right`, `Split Down`, `Split Up`, `Split Left`, `Add to Group`) in `--primary` background / `--crust` text, 4 px radius, `--shadow-menu`
- Position/size animates with `var(--d-fast)` to whichever zone is current

When dropped on an edge: the target group is replaced in the tree by a new Split node containing `[targetGroup, newGroup]` (or `[newGroup, targetGroup]` for left/top). The split's direction is `row` for left/right, `col` for top/bottom. If the source group is now empty, walk up and collapse it (replace its parent split with the surviving sibling).

#### Tab strip drop (within a strip)

When dragging over a tab strip, the strip computes an **insertion index** by comparing cursor X against tab midpoints. A 2 px vertical line in `--primary` is drawn at that boundary while hovering. Dropping moves the tab to that index — either reordering within the same group or migrating from another group.

### Context menu (right-click a tab)

Width 220 px min, `--surface0` background, `--surface1` 1 px border, 4 px radius, `--shadow-menu`. 6 px / 14 px row padding. Hover row: `--selection-bg`.

```
Close                       ⌘W
Close Others
Close to the Right
Close All
────────────────────────
Pin Tab        (toggles to "Unpin Tab")
Mark as Modified  (toggles to "Mark as Saved")
────────────────────────
Split Right                 ⌘\
Split Down
```

Behavior:
- **Close** — remove tab from group; if active, select the next neighbor (or previous if it was last). Collapse empty group.
- **Close Others** — keep this tab + any pinned tabs; close the rest.
- **Close to the Right** — close everything to the right except pinned tabs.
- **Close All** — close everything except pinned tabs; collapse empty group.
- **Pin / Unpin** — toggle `pinned`, then sort group so pinned come first (stable).
- **Mark as Modified / Saved** — toggle `modified`.
- **Split Right / Split Down** — same as edge drop: tab leaves source group, a new group is created, parent split is built.

### Content panels

The active tab in each group dispatches by `kind`:

- `kind: 'file'` → JSON tree viewer (see `tabs/json-tree.jsx`). Monospace font, 22 px row, 16 px indent guides in `--indent-guide`, syntax colors from the `--syn-*` tokens.
- `kind: 'plugin'` → plugin component, one per `key`. See `tabs/plugins.jsx` for the five built-in plugins:
  - **Welcome** — quick-start grid: Start / Recent files on the left, keyboard tips on the right.
  - **Settings** — 4-section nav (Appearance, Editor, Plugins, Shortcuts) with form rows separated by 1 px `--surface0` lines.
  - **Schema Validator** — Target + Schema selectors → list of validation results (icon + JSONPath + message), with error/warning counts in the header subtitle.
  - **Diff Viewer** — two-column line-by-line diff. Left side tints `--error` for changed lines, right side tints `--success`.
  - **JSONPath** — query input + result block, with clickable suggestion chips.

Every plugin uses a shared `PluginShell` wrapper: title (28 px, weight 700, colored by the plugin's accent token) over a 1 px `--surface0` divider, then a 24 px-padded body.

### Status bar — 24 px

- Background `--crust`, `--fs-sm`, padded 12 px horizontally
- `<groups count>` groups · `<tabs count>` tabs open · `<active file type>` · `Ready` (`--success`, with lightning icon)
- Right-aligned hint: `Drag any tab to a pane edge to split.` in `--overlay1`

## Interactions & Behavior

### Drag and drop

The prototype uses HTML5 native DnD plus a **window-scoped drag handle** (`window.__thothDrag = { tabId, fromGroupId }`) because `dataTransfer` can't be read in `dragover` events. In egui you don't need this shim — egui's drag state is reachable from any widget.

Drag lifecycle:

1. `dragStart` on a tab → set `__thothDrag`, allow `move`. egui equivalent: store `dragged_tab_id` on a `PointerState`-derived field.
2. `dragOver` on a strip → compute insertion index from cursor X. egui: iterate tab rects in the strip layout pass, find which half of which tab is under the pointer.
3. `dragOver` on a pane → compute zone via the 22% rule. egui: read the pane's `Rect` and `pointer_pos()`, do the math, paint the highlight + label with `Painter`.
4. `drop` → dispatch the appropriate reducer action (see below). Clear the drag handle.

### Reducer actions

The state is `{ root: Node, focusedGroupId }` where `Node` is `Group | Split`. All actions deep-clone the tree. The full set is in `tabs/state.jsx`:

| Action          | Payload                                                | Effect                                                                                |
| --------------- | ------------------------------------------------------ | ------------------------------------------------------------------------------------- |
| `select`        | `groupId, tabId`                                       | Set `activeId` in group, focus group.                                                 |
| `focus`         | `groupId`                                              | Mark group as focused (changes tab top-accent color).                                 |
| `open`          | `kind, key, groupId?`                                  | If a tab with that `(kind,key)` exists in any group, focus it. Otherwise append to target group (focused or first). |
| `close`         | `groupId, tabId`                                       | Remove tab; pick neighbor as new active; collapse group if empty.                     |
| `closeOthers`   | `groupId, tabId`                                       | Keep this tab + pinned tabs.                                                          |
| `closeRight`    | `groupId, tabId`                                       | Drop everything to the right (except pinned).                                         |
| `closeAll`      | `groupId`                                              | Drop everything except pinned; collapse if empty.                                     |
| `pin`           | `groupId, tabId`                                       | Toggle `pinned`; stable-sort pinned to the front.                                     |
| `reorder`       | `groupId, tabId, toIndex`                              | Reorder within a group. Re-sort pinned.                                               |
| `moveTab`       | `tabId, fromGroupId, toGroupId, toIndex`               | Move tab cross-group; collapse source if empty.                                       |
| `split`         | `targetGroupId, tabId, fromGroupId, edge`              | Remove tab from source; create new group with the tab; replace target with a Split. No-op if dragging the only tab of a group into a split of itself. |
| `resize`        | `splitId, sizes[]`                                     | Update a Split's size ratios (must sum to 1; the reducer normalizes).                 |
| `toggleModified`| `groupId, tabId`                                       | Toggle `modified` (drives the dot indicator).                                         |
| `reset`         | —                                                      | Restore initial layout.                                                               |

#### `collapseIfEmpty` invariant

After any action that removes tabs, walk the tree: if a Group has zero tabs, splice it out of its parent Split. If the parent Split is left with one child, replace the Split with that child. The root, if it becomes empty, is replaced with a fresh empty Group.

#### `open` de-duplication

If the user clicks "Open `users.json`" but a tab for `users.json` already exists in *any* group, focus that existing tab rather than creating a new one. This matches VSCode's default behavior for non-pinned editors.

### Animations

| Property                     | Duration  | Easing       |
| ---------------------------- | --------- | ------------ |
| Tab bg on hover              | 100 ms    | linear       |
| Tab accent strip color       | 100 ms    | linear       |
| Close button opacity         | 100 ms    | linear       |
| Drop-zone overlay reposition | 100 ms    | linear       |
| Splitter color on hover      | 100 ms    | linear       |
| Settings toggle thumb        | 100 ms    | `--ease-out` |

No bouncy springs. Per the Thoth brand, motion is opt-out — wrap in a `settings.ui.enable_animations` check.

### Keyboard

Spec'd in the Shortcuts pane and tooltips; the prototype doesn't bind them all yet, but the actions exist on the reducer and are trivial to wire up.

| Shortcut       | Action                       |
| -------------- | ---------------------------- |
| `⌘O`           | Open file (focus Files flyout) |
| `⌘T`           | New tab (open Welcome)       |
| `⌘W`           | Close active tab             |
| `⌘\`           | Split active tab right       |
| `⌘K ⌘\`        | Split active tab down        |
| `⌘K ⌘→` / `←`  | Focus next / prev group      |
| `⌘F`           | Search in active file        |
| `⌘⇧T`          | Toggle Mocha / Latte         |
| `⌘K ⇧Enter`    | Pin active tab               |

## State Management

### Types

```ts
type TabKind = 'file' | 'plugin';

interface Tab {
  id: string;            // unique across tree
  kind: TabKind;
  key: string;           // filename for files; plugin id for plugins
  pinned: boolean;
  modified: boolean;
}

interface Group {
  type: 'group';
  id: string;
  tabs: Tab[];
  activeId: string | null;
}

interface Split {
  type: 'split';
  id: string;
  dir: 'row' | 'col';    // row = side-by-side, col = stacked
  sizes: number[];       // ratios that sum to 1, same length as children
  children: Node[];      // length ≥ 2 (after collapse pass)
}

type Node = Group | Split;

interface LayoutState {
  root: Node;
  focusedGroupId: string;
}
```

### Rust / egui translation

A natural mapping:

```rust
struct Tab { id: TabId, kind: TabKind, key: String, pinned: bool, modified: bool }
enum TabKind { File, Plugin }
enum Node {
    Group { id: GroupId, tabs: Vec<Tab>, active_id: Option<TabId> },
    Split { id: SplitId, dir: SplitDir, sizes: Vec<f32>, children: Vec<Node> },
}
struct EditorLayout { root: Node, focused: GroupId, dragging: Option<DragHandle> }
struct DragHandle { tab_id: TabId, from_group: GroupId }
```

All reducer actions become `impl EditorLayout` methods that take `&mut self`. Use `egui::Id`-derived ids so they survive across frames. egui's `Response::dragged()` + `pointer.hover_pos()` replaces the HTML5 DnD events. For the live drop-zone highlight, paint with `Painter::rect_filled` + `Painter::text` inside the dragged group's `Rect` — egui will redraw on every frame the pointer moves, no animation tweens needed.

## Design Tokens

All tokens come from `assets/colors_and_type.css` (mirror of the Thoth design system file). Use the **CSS variable names** when discussing, but lift the underlying hex values to Rust constants in `src/theme.rs` (which already exists in the Thoth codebase — extend it, don't fork).

### Colors used in this design (Mocha / dark — Latte values in CSS file)

| Token                | Hex        | Usage in design                                            |
| -------------------- | ---------- | ---------------------------------------------------------- |
| `--base`             | `#1e1e2e`  | Active tab background, editor content area                 |
| `--mantle`           | `#181825`  | Inactive tabs, tab strip, activity bar                      |
| `--crust`            | `#11111b`  | Title bar, status bar                                       |
| `--surface0`         | `#313244`  | Tab hover, dividers, context menu bg, settings card bg     |
| `--surface1`         | `#45475a`  | Close-button hover, context menu border, indent guides     |
| `--surface2`         | `#585b70`  | Active-but-unfocused tab accent strip                       |
| `--text`             | `#cdd6f4`  | Primary text, active tab label                              |
| `--overlay1`         | `#7f849c`  | Secondary text, inactive tab label, status-bar hint        |
| `--overlay2`         | `#9399b2`  | Activity bar default icon, brackets, pin glyph             |
| `--primary` (mauve)  | `#cba6f7`  | Plugin tab accent, drop-zone highlight + label              |
| `--accent` (blue)    | `#89b4fa`  | File tab accent, JSON keys, links                           |
| `--selection-bg`     | `rgba(127,132,156,0.30)` | Settings nav active, context menu hover row    |
| `--selection-stroke` | `#89b4fa`  | Splitter hover, settings nav left border                    |
| `--success`          | `#a6e3a1`  | "Ready" in status bar, diff right-side tint                 |
| `--warning`          | `#f9e2af`  | Validator warning rows                                      |
| `--error`            | `#f38ba8`  | Validator error rows, diff left-side tint                   |
| `--info`             | `#74c7ec`  | Schema Validator plugin accent                              |
| `--syn-key`          | `#89b4fa`  | JSON keys in tree                                           |
| `--syn-string`       | `#a6e3a1`  | JSON strings                                                |
| `--syn-number`       | `#fab387`  | JSON numbers                                                |
| `--syn-boolean`      | `#cba6f7`  | JSON booleans                                               |
| `--syn-null`         | `#7f849c`  | JSON null                                                   |
| `--syn-bracket`      | `#9399b2`  | `{ } [ ] :`                                                 |

### Spacing — 4 px grid

`--space-1`–`--space-7` = 4 / 8 / 12 / 16 / 24 / 32 / 48 px. Every padding/margin in this design is a multiple of 4.

### Typography

- UI: `--font-ui` (system stack). Scale: `--fs-xs` 11, `--fs-sm` 12, `--fs-md` 13, `--fs-lg` 14, `--fs-xl` 16, `--fs-2xl` 20.
- Mono: `--font-mono` (JetBrains Mono → Fira Code → Cascadia Code → SF Mono → Consolas → Monaco). Used for JSON tree rows, keyboard hints, JSONPath input, diff lines.
- Weights: 400 / 500 / 600 / 700.

### Component heights (fixed)

- Title bar 32 · Tab strip 35 · Status bar 24 · Tree row 22 · Context menu row ~28 · Activity rail button 32 · Splitter 4

### Radius

- `--radius-sm` (4 px) — tabs' close button, buttons, badges, context menu, flyout
- `--radius-md` (8 px) — modal-ish containers (validator/diff result panels)
- `--radius-lg` (12 px) — plugin cards in Settings → Plugins

### Shadow

- `--shadow-menu` `0 4px 12px rgba(0,0,0,0.5)` — context menu, flyouts, drop-zone pill label
- No shadows on tabs, buttons, or the content panes (depth via background layering only).

## Assets

- `assets/thoth_icon_256.png` — app icon (256 px), used in title bar at 16 × 16 with 3 px corner radius. Original from the Thoth repo.
- **Icons** — Phosphor regular (outline) weight throughout. Map by semantic name:

| Where                | Icon name        |
| -------------------- | ---------------- |
| File tab (JSON)      | `brackets-curly` |
| File tab (NDJSON)    | `rows`           |
| Welcome plugin       | `house`          |
| Settings plugin      | `gear`           |
| Schema Validator     | `check-circle`   |
| Diff plugin          | `git-diff`       |
| JSONPath plugin      | `magnifying-glass` |
| Tab close            | `x`              |
| Tab pin              | `push-pin-simple` (rotated 45°) |
| Activity: files      | `files`          |
| Activity: theme      | `sun` / `moon`   |
| Validator error row  | `x-circle`       |
| Validator warning    | `warning`        |
| Status: groups       | `stack`          |
| Status: ready        | `lightning`      |

Thoth's existing iconography section in the design system already documents these mappings — match by semantic name when porting.

## Files

```
design_handoff_multi_tab/
├── README.md                              ← this file
├── Thoth Tabs.html                        ← HTML entry; loads all scripts in order
├── assets/
│   ├── colors_and_type.css                ← All design tokens. Reference only.
│   └── thoth_icon_256.png                 ← App icon.
└── tabs/
    ├── data.js                            ← Sample THOTH_FILES + THOTH_PLUGINS registry
    ├── state.jsx                          ← Reducer + tree helpers + useLayout() hook
    ├── layout.jsx                         ← TabStrip, Tab, DropOverlay, ContextMenu,
    │                                        GroupPane, SplitNode, SplitContainer,
    │                                        ResizeHandle (the meat of the design)
    ├── json-tree.jsx                      ← JSON tree viewer for file tabs
    ├── plugins.jsx                        ← The five plugin panels + shared shell
    ├── app.jsx                            ← TitleBar, ActivityBar, StatusBar, root composition
    └── tweaks-panel.jsx                   ← Prototype-only Tweaks panel (ignore for shipping)
```

### Reading order for implementation

1. `tabs/state.jsx` — the state machine. Read first; this is the spec for tree shape and every action.
2. `tabs/layout.jsx` — the visual + interactive layer. `Tab`, `TabStrip`, `DropOverlay`, `GroupPane`, `SplitContainer`, `ResizeHandle`. Most of the design lives here.
3. `tabs/app.jsx` — the app shell. Shows how the rail / title / status pieces compose around the split tree.
4. `tabs/plugins.jsx` — the five plugin views. Each is small and self-contained.
5. `tabs/json-tree.jsx` — the existing JSON tree, slightly adapted for CSS-variable theming.
6. `tabs/data.js` — sample data shapes (helpful for stubbing during implementation).

# Thoth Design System
> Complete UI/UX Guidelines & Visual Specifications

**Version:** 1.0
**Style:** VS Code Inspired
**Last Updated:** 2025-11-10

---

## Table of Contents

1. [Design Philosophy](#design-philosophy)
2. [Visual Mockups](#visual-mockups)
3. [Color System](#color-system)
4. [Component Specifications](#component-specifications)
5. [Typography](#typography)
6. [Spacing & Layout](#spacing--layout)
7. [Interactions & Animations](#interactions--animations)
8. [Accessibility](#accessibility)
9. [Implementation Roadmap](#implementation-roadmap)
10. [Code Examples](#code-examples)

---

## Design Philosophy

### Core Principles

1. **Information Density**: Show maximum useful information without clutter
2. **Visual Hierarchy**: Important actions visible, advanced features accessible
3. **Consistency**: Predictable patterns across all UI elements
4. **Performance**: Smooth interactions, no lag in rendering
5. **Accessibility**: Clear contrast, readable fonts, keyboard shortcuts

### Design Inspiration

- **VS Code**: Best-in-class code editor UI (clean hierarchy, excellent keyboard navigation)
- **Chrome DevTools**: JSON tree viewer (smooth expand/collapse, great syntax highlighting)
- **Postman**: API/JSON inspection (clean layout, professional feel)
- **Sublime Text**: Minimalist elegance (uncluttered, performance-focused)

---

## Visual Mockups

### Full Application Layout (1400x900)

```
┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
┃ Thoth — data.json                                                      ⊡ ⊗ ┃ Title bar (32px)
┣━━┯━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┫
┃  │ 📂 📄 🪟   JSON ▼   🔍 Search...                          Aa   ⚙  🌙  ┃ Toolbar (40px)
┃  │ ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔┃
┃📁│                                                                        ┃
┃  │  ▾ [0]: {                                                             ┃
┃📋│  │   "id": 1001                                                       ┃
┃  │  │   "name": "John Doe"                                               ┃
┃🔍│  │   "email": "john@example.com"                                      ┃
┃  │  │   "active": true                                                   ┃
┃⚙ │  │   ▸ "address": {...}                                               ┃
┃  │  │   ▾ "roles": [                                                     ┃
┃  │  │   │   [0]: "admin"                                                 ┃
┃48│  │   │   [1]: "developer"                                             ┃
┃px│  │   │   [2]: "reviewer"                                              ┃
┃  │  │   ]                                                                ┃
┃S │  }                                                                    ┃
┃i │                                                                        ┃
┃d │  ▸ [1]: {...}                                                         ┃
┃e │                                                                        ┃
┃b │  ▸ [2]: {...}                                                         ┃
┃a │                                                                        ┃
┃r │  ▸ [3]: {...}                                                         ┃
┃  │                                                                        ┃
┣━━┷━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┫
┃ 📄 data.json  │  1,234 items  │  JSON  │  ⚡ Ready                        ┃ Status bar (24px)
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

### Layout Breakdown

**Components (Top to Bottom):**
1. Title bar with window controls
2. Compact toolbar with integrated search
3. Collapsible sidebar (48px collapsed, 240px expanded)
4. Main JSON viewer area
5. Status bar with file information

### Search Active State

```
┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
┃ Thoth — data.json                                          ⊡ ⊗ ┃
┣━━┯━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┫
┃  │ 📂 📄 🪟  JSON ▼  🔍 john                      [Aa] ⚙ 🌙 ┃
┃  │                   ▔▔▔▔▔▔▔     ↑ Active search            ┃
┣━━┷━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┫
┃📁│ 🔍 3 matches found                                        ┃
┃  │ ─────────────────────────────────────────────────────────┃
┃📋│                                                           ┃
┃  │  ▾ [0]: {                                                ┃
┃🔍│  │   "id": 1001                                          ┃
┃  │  │   "name": "John Doe"           ← Highlighted yellow  ┃
┃⚙ │  │   "email": "john@example.com"  ← Highlighted yellow  ┃
┃  │  │   "active": true                                      ┃
┃  │  │   ▸ "address": {...}                                  ┃
┃  │  }                                                       ┃
┃  │                                                           ┃
┃  │  ▾ [5]: {                                                ┃
┃  │  │   "name": "Johnny Smith"       ← Highlighted yellow  ┃
┃  │  }                                                       ┃
┣━━┷━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┫
┃ 📄 data.json  │  3 of 1,234 items  │  JSON  │  🔍 Filtered ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

---

## Color System

### Dark Theme (Primary)

#### Background Layers
```
Primary background:   #1e1e1e  ████████  Main editor area
Secondary background: #252526  ████████  Sidebar, panels
Tertiary background:  #2d2d30  ████████  Toolbar, headers
Hover overlay:        #ffffff0d ████████  5% white overlay
Active overlay:       #ffffff14 ████████  8% white overlay
Selected:             #0e639c4d ████████  30% blue overlay
```

#### Borders & Dividers
```
Subtle divider:  #3e3e42  Separators, panel borders
Panel border:    #454545  Emphasized borders
```

#### Text Colors
```
Primary text:    #cccccc  Main content
Secondary text:  #999999  Labels, secondary info
Disabled text:   #666666  Inactive elements
Accent text:     #4ec9b0  Links, highlights (cyan/teal)
```

#### Syntax Highlighting
```
Keys/Properties:       #9cdcfe  ████████  "name", "email"
String values:         #ce9178  ████████  "John Doe"
Numbers:               #b5cea8  ████████  1001, 42
Booleans:              #569cd6  ████████  true, false
Null/undefined:        #808080  ████████  null, undefined
Brackets/Punctuation:  #d4d4d4  ████████  { } [ ] :
```

#### Interactive Elements
```
Primary button:         #0e639c  ████████
Primary button hover:   #1177bb  ████████
Danger button:          #f14c4c  ████████
Success:                #89d185  ████████
Warning:                #cca700  ████████
Status bar background:  #007acc  ████████  VS Code blue
```

### Light Theme

#### Background Layers
```
Primary background:   #ffffff
Secondary background: #f3f3f3
Tertiary background:  #e8e8e8
Hover overlay:        rgba(0, 0, 0, 0.03)
Active overlay:       rgba(0, 0, 0, 0.06)
```

#### Text Colors
```
Primary text:    #333333
Secondary text:  #666666
Disabled text:   #999999
Accent text:     #0e639c
```

#### Syntax Highlighting
```
Keys/Properties:       #001080  (dark blue)
String values:         #a31515  (red)
Numbers:               #098658  (green)
Booleans:              #0000ff  (blue)
Null/undefined:        #808080  (gray)
Brackets/Punctuation:  #000000
```

### Color Accessibility

**Contrast Ratios:**
- Text on background: 4.5:1 minimum (WCAG AA)
- Interactive elements: 3:1 minimum
- Disabled elements: 2:1 minimum

**Color Blindness Considerations:**
- Don't rely solely on color for meaning
- Use icons + text labels
- Test with simulators (deuteranopia, protanopia)

---

## Component Specifications

### 1. Title Bar

```
┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
┃ Thoth — data.json                    ⊡ ⊗ ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

**Specifications:**
- Height: 32px
- Background: #2d2d30 (Dark) / #e8e8e8 (Light)
- Text: #cccccc (Dark) / #333333 (Light)
- Font: 14px, System font
- Pattern: "Thoth — {filename}" or "Thoth — JSON & NDJSON Viewer"

### 2. Toolbar

```
┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
┃ 📂 📄 🪟 │ JSON ▼ │ 🔍 Search...            Aa │  ⚙  🌙 ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

**Specifications:**
- Height: 40px
- Background: #2d2d30
- Border bottom: 1px #3e3e42
- Icon size: 16x16px
- Spacing: 8px between groups, 4px between items

**Elements (Left to Right):**

1. **📂 Open** - File picker dialog
   - Tooltip: "Open file (Ctrl+O)"
   - Action: Opens file dialog for JSON/NDJSON

2. **📄 Clear** - Clear current file
   - Tooltip: "Clear file (Ctrl+W)"
   - Action: Closes current file

3. **🪟 New Window** - Spawn new instance
   - Tooltip: "New window (Ctrl+N)"
   - Action: Spawns independent Thoth process

4. **│** - Separator (8px space)

5. **JSON ▼** - File type dropdown
   - Options: JSON, NDJSON
   - Width: 80px

6. **│** - Separator (8px space)

7. **🔍 Search box** - Integrated search
   - Width: 200px (expandable)
   - Placeholder: "Search..."
   - Hint text: #999999

8. **Aa** - Match case toggle
   - Tooltip: "Match case"
   - Toggle button

9. **Spacer** - Flexible space pushing right elements

10. **⚙** - Settings
    - Tooltip: "Settings (Ctrl+,)"
    - Badge: Red dot if update available

11. **🌙** - Theme toggle
    - Tooltip: "Toggle theme"
    - Shows: 🌙 (dark) or ☀ (light)

**Button States:**
```
Normal:   Background: transparent
Hover:    Background: rgba(255,255,255,0.1)
Active:   Background: rgba(255,255,255,0.15)
Disabled: Opacity: 0.4
```

### 3. Sidebar

**Collapsed State (48px):**
```
┏━━┓
┃📁┃  ← Recent Files
┃  ┃
┃📋┃  ← Clipboard History
┃  ┃
┃🔍┃  ← Search Panel
┃  ┃
┃⚙ ┃  ← Settings
┃  ┃
┗━━┛
```

**Specifications:**
- Width: 48px (collapsed), 240px (expanded)
- Background: #252526
- Border right: 1px #3e3e42
- Icon size: 20x20px
- Icon padding: 14px (centers in 48px)
- Hover: rgba(255,255,255,0.05) background

**Expanded State (240px):**
```
┏━━━━━━━━━━━━━━━━━━━━━━┓
┃ 📁 RECENT FILES       ┃
┃ ─────────────────────┃
┃ data.json          ✕ ┃
┃ users.ndjson       ✕ ┃
┃ config.json        ✕ ┃
┃                      ┃
┃ [Open File...]       ┃
┃                      ┃
┣━━━━━━━━━━━━━━━━━━━━━━┫
┃ 🔍 SEARCH            ┃
┣━━━━━━━━━━━━━━━━━━━━━━┫
┃ ⚙ SETTINGS           ┃
┗━━━━━━━━━━━━━━━━━━━━━━┛
```

**Section Headers:**
- Font: 11px, uppercase, bold
- Color: #999999
- Padding: 8px
- Border bottom: 1px #3e3e42

**List Items:**
- Font: 13px
- Padding: 4px 8px
- Hover: rgba(255,255,255,0.05)
- Selected: #094771
- Close button: Visible on hover

### 4. JSON Viewer

```
┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
┃                                              ┃
┃  ▾ [0]: {                                    ┃
┃  │   "id": 1001                              ┃
┃  │   "name": "John Doe"                      ┃
┃  │   "email": "john@example.com"             ┃
┃  │   "active": true                          ┃
┃  │   ▸ "address": {...}                      ┃
┃  │   ▾ "roles": [                            ┃
┃  │   │   [0]: "admin"                        ┃
┃  │   │   [1]: "developer"                    ┃
┃  │   ]                                       ┃
┃  }                                           ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

**Specifications:**
- Background: #1e1e1e
- Text: #cccccc
- Row height: 22px
- Indent: 16px per level
- Font: Monospace (JetBrains Mono, Fira Code, Cascadia Code), 13px

**Expand/Collapse Icons:**
- Collapsed: ▸
- Expanded: ▾
- Size: Same as text (13px)
- Color: #d4d4d4

**Indent Guides:**
- Color: rgba(255,255,255,0.1)
- Width: 1px
- Position: Every 16px at indent boundaries
- Draws from parent to last child

**Row States:**
```
Normal:      Background: transparent
Hover:       Background: rgba(255,255,255,0.05)
Selected:    Background: rgba(14,99,156,0.3)
             Left border: 2px #007acc
Alternating: Background: rgba(255,255,255,0.02) (optional)
```

**Keyboard Focus:**
- Left border: 2px #007acc
- Dotted outline: 1px (accessibility)

### 5. Context Menu

```
┏━━━━━━━━━━━━━━━━━━━━━━━┓
┃ 📋 Copy Key       ^C  ┃
┃ 📋 Copy Value    ^⇧C  ┃
┣━━━━━━━━━━━━━━━━━━━━━━━┫
┃ 📋 Copy Object   ^⌥C  ┃
┃ 📋 Copy Path     ^⇧P  ┃
┣━━━━━━━━━━━━━━━━━━━━━━━┫
┃ ↔ Expand All          ┃
┃ ↔ Collapse All        ┃
┗━━━━━━━━━━━━━━━━━━━━━━━┛
```

**Specifications:**
- Background: #2d2d30
- Text: #cccccc
- Hover: #094771
- Border: 1px rgba(255,255,255,0.1)
- Border radius: 4px
- Padding: 4px
- Item height: 28px
- Font: 13px
- Shadow: 0 4px 12px rgba(0,0,0,0.5)

**Dividers:**
- Color: #3e3e42
- Height: 1px
- Margin: 4px 0

### 6. Status Bar

```
┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
┃ 📄 data.json │ 1,234 items │ JSON │ ⚡ Ready ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
```

**Specifications:**
- Height: 24px
- Background: #007acc (VS Code blue)
- Text: #ffffff
- Font: 12px, System font
- Padding: 0 12px
- Item separator: │ with 8px spacing

**Status Indicators:**
```
⚡ Ready      - Idle state (white)
⏳ Loading... - Loading file (yellow #cca700)
⚠ Error      - Error state (red #f14c4c)
🔍 Searching... - Active search (cyan #4ec9b0)
🔍 Filtered  - Search results shown (cyan #4ec9b0)
```

**Sections (Left to Right):**
1. Filename with icon
2. Item count (total or filtered)
3. File type
4. Status indicator

### 7. Settings Panel

```
┌────────────────────────────────────┐
│  Settings                       ✕ │
├────────────────────────────────────┤
│ ┌─ Appearance ─────────────────┐  │
│ │ Theme: ● Dark  ○ Light       │  │
│ │ Font Size: [13] px           │  │
│ │ Show Indent Guides: ☑        │  │
│ └───────────────────────────────┘  │
│                                    │
│ ┌─ Editor ─────────────────────┐  │
│ │ Indent Size: [16] px         │  │
│ │ Auto Expand Depth: [2]       │  │
│ │ Alternating Row BG: ☑        │  │
│ └───────────────────────────────┘  │
│                                    │
│ ┌─ Updates ────────────────────┐  │
│ │ Check for updates: ☑         │  │
│ │ Current version: 0.2.4       │  │
│ │ [Check for Updates]          │  │
│ └───────────────────────────────┘  │
│                                    │
│         [Apply]  [Cancel]          │
└────────────────────────────────────┘
```

**Specifications:**
- Width: 480px
- Backdrop: rgba(0,0,0,0.6) (blocks interaction)
- Background: #252526
- Border: 1px #3e3e42
- Border radius: 8px
- Padding: 16px
- Shadow: 0 8px 24px rgba(0,0,0,0.8)

**Section Groups:**
- Border: 1px #3e3e42
- Border radius: 4px
- Padding: 12px
- Margin bottom: 16px
- Header: 12px, bold, #999999

---

## Typography

### Font Stack

**UI Text:**
```css
font-family: -apple-system, BlinkMacSystemFont,
             "Segoe UI", "Roboto", "Ubuntu",
             "Helvetica Neue", Arial, sans-serif;
```

**Code/JSON (Monospace):**
```css
font-family: "JetBrains Mono", "Fira Code",
             "Cascadia Code", "SF Mono",
             "Consolas", "Monaco", monospace;
```

### Size Scale

```
┌────────────────────────────────┐
│ Title (16px, Semibold)         │ Window title
│                                │
│ Section Header (14px, Bold)    │ Panel headers
│                                │
│ Toolbar (14px, Regular)        │ Button labels
│                                │
│ JSON Content (13px, Mono)      │ Main content
│ Body Text (13px, Regular)      │ General UI
│                                │
│ Status Bar (12px, Regular)     │ Footer text
│ Sidebar Header (11px, Bold)    │ Small headers
└────────────────────────────────┘
```

### Text Hierarchy

**Window Title:** 16px, Semibold, System font
**Section Headers:** 14px, Bold, System font
**Toolbar Labels:** 14px, Regular, System font
**JSON Content:** 13px, Regular, Monospace
**Body Text:** 13px, Regular, System font
**Status Bar:** 12px, Regular, System font
**Sidebar Headers:** 11px, Bold, Uppercase, System font

---

## Spacing & Layout

### Grid System (4px Base Unit)

```
Padding/Margin Scale:
├─ 4px  (xs): Tight spacing, inline elements
├─ 8px  (sm): Comfortable spacing, buttons
├─ 12px (md): Section padding, comfortable
├─ 16px (lg): Component separation, indent
└─ 24px (xl): Major section breaks
```

### Component Heights

```
├─ 22px: JSON rows, list items
├─ 24px: Status bar
├─ 28px: Small buttons, context menu items
├─ 32px: Title bar, normal buttons
└─ 40px: Toolbar, comfortable click targets
```

### Window Sizing

```
Minimum:    800 x 600
Default:    1200 x 800
Optimal:    1400 x 900+
```

### Responsive Breakpoints

```
< 1000px: Collapse sidebar by default
         Move search to second row or modal

< 800px:  Hide sidebar completely
          Compact toolbar further
          Stack toolbar items
```

---

## Interactions & Animations

### Animation Timings

```
Expand/Collapse tree:   150ms ease-out
Hover effects:          100ms ease
Panel slide in/out:     200ms ease-in-out
Search highlighting:    50ms  (instant feel)
Tooltip appear:         300ms delay, 100ms fade-in
Modal backdrop:         150ms fade-in
```

### Hover Effects

**Buttons:**
- Transition: background-color 100ms ease
- Normal → Hover: transparent → rgba(255,255,255,0.1)
- Active: rgba(255,255,255,0.15)

**JSON Rows:**
- Transition: background-color 100ms ease
- Normal → Hover: transparent → rgba(255,255,255,0.05)

**Sidebar Items:**
- Transition: background-color 100ms ease
- Show close button on hover (fade-in 100ms)

### Click/Active States

**Buttons:**
- Scale: 0.98 (subtle press effect)
- Transition: 50ms ease-out

**Toggle Buttons:**
- Instant state change (no animation)
- Color transition: 100ms ease

### Keyboard Shortcuts

**File Operations:**
- `Ctrl/Cmd + O`: Open file
- `Ctrl/Cmd + W`: Clear/Close
- `Ctrl/Cmd + N`: New window
- `Ctrl/Cmd + ,`: Settings

**Navigation:**
- `Ctrl/Cmd + F`: Focus search
- `Ctrl/Cmd + G`: Next match
- `Ctrl/Cmd + Shift + G`: Previous match
- `Escape`: Clear search / Close panels
- `Tab`: Navigate forward
- `Shift + Tab`: Navigate backward

**Tree Operations:**
- `→` (Arrow Right): Expand node
- `←` (Arrow Left): Collapse node
- `Ctrl/Cmd + →`: Expand all children
- `Ctrl/Cmd + ←`: Collapse all
- `Space`: Toggle expand/collapse
- `Enter`: Select/Activate

**Clipboard:**
- `Ctrl/Cmd + C`: Copy selection
- `Ctrl/Cmd + Shift + C`: Copy value only
- `Ctrl/Cmd + Alt + C`: Copy as JSON object
- `Ctrl/Cmd + Shift + P`: Copy path

### Mouse Interactions

```
Single click:   Select row
Double click:   Expand/collapse (if expandable)
Right click:    Context menu
Hover:          Highlight row
Drag:           Scroll viewport (future)
Wheel:          Scroll content
```

---

## Accessibility

### Contrast Ratios (WCAG 2.1)

**Level AA Compliance:**
- Normal text: 4.5:1 minimum
- Large text (18px+): 3:1 minimum
- UI components: 3:1 minimum
- Disabled elements: 2:1 minimum

**Testing Tools:**
- WebAIM Contrast Checker
- Chrome DevTools Color Picker
- axe DevTools

### Focus Indicators

**Keyboard Focus:**
- Border: 2px solid #007acc
- Outline: 1px dotted (accessibility mode)
- Offset: 2px
- Border radius: 2px

**Focus Order:**
1. Toolbar buttons (left to right)
2. File type dropdown
3. Search input
4. Match case toggle
5. Settings button
6. Theme toggle
7. JSON tree (keyboard navigation)
8. Status bar (if interactive elements)

**Focus Trap:**
- Modal dialogs trap focus
- Escape key closes modal
- Focus returns to triggering element

### Screen Reader Support

**ARIA Labels:**
- All icon buttons have aria-label
- Interactive elements have role attributes
- Tree structure uses aria-expanded
- Search results announce count

**Announcements:**
- File loaded: "Loaded {filename}, {count} items"
- Search complete: "Found {count} matches"
- Error: "Error: {message}"

### Keyboard Navigation

- All features accessible via keyboard
- Visible focus indicators
- Logical tab order
- Escape key dismisses modals/overlays
- Arrow keys navigate tree

---

## Implementation Roadmap

### Phase 1: Quick Wins (1-2 days)
**High impact, low effort improvements**

✅ **Colors & Syntax**
- Update syntax highlighting palette
- Improve contrast ratios
- Add better hover states

✅ **Icons & Indicators**
- Change expand icons: `+/-` → `▸/▾`
- Add tooltips to all toolbar buttons
- Improve button focus states

✅ **Spacing**
- Increase indent from 12px to 16px
- Standardize padding to 4px grid
- Improve row heights (20px → 22px)

### Phase 2: Layout Improvements (3-5 days)
**Medium effort, significant UX improvement**

🔲 **Toolbar Redesign**
- Compact to single 40px row
- Icon-only buttons with tooltips
- Integrate search into toolbar
- Remove bottom search panel

🔲 **Status Bar**
- Add 24px status bar at bottom
- Show file info, count, status
- Update dynamically

🔲 **Color System**
- Implement full color palette
- Support light/dark themes properly
- Add theme switching

### Phase 3: Advanced Features (1-2 weeks)
**Higher effort, major feature additions**

🔲 **Sidebar**
- Add collapsible 48px sidebar
- Recent files list
- Expandable to 240px
- Search panel integration

🔲 **Visual Enhancements**
- Add indent guide lines
- Alternating row backgrounds (optional)
- Smooth animations
- Better loading states

🔲 **Keyboard Shortcuts**
- Implement all shortcuts
- Add keyboard navigation
- Shortcut hints in menus

### Phase 4: Polish & Details (ongoing)
**Refinement and edge cases**

🔲 **Responsiveness**
- Handle narrow windows
- Mobile/touch support (future)
- Zoom levels

🔲 **Performance**
- Optimize large file rendering
- Virtual scrolling improvements
- Lazy rendering

🔲 **Accessibility**
- ARIA labels
- Screen reader testing
- High contrast mode

---

## Code Examples

### egui Implementation Snippets

#### 1. Toolbar with Integrated Search

```rust
egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);

        // Left: Primary actions
        if ui.add(egui::Button::new("📂").frame(false))
            .on_hover_text("Open file (Ctrl+O)")
            .clicked() {
            // Open file dialog
        }

        if ui.add(egui::Button::new("📄").frame(false))
            .on_hover_text("Clear file (Ctrl+W)")
            .clicked() {
            // Clear file
        }

        if ui.add(egui::Button::new("🪟").frame(false))
            .on_hover_text("New window (Ctrl+N)")
            .clicked() {
            // Spawn new window
        }

        ui.separator();

        // File type selector
        egui::ComboBox::from_id_source("file_type")
            .selected_text(format!("{:?}", file_type))
            .width(80.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(file_type, FileType::Json, "JSON");
                ui.selectable_value(file_type, FileType::Ndjson, "NDJSON");
            });

        ui.separator();

        // Integrated search
        ui.add(egui::TextEdit::singleline(search_query)
            .hint_text("🔍 Search...")
            .desired_width(200.0));

        ui.checkbox(match_case, "Aa")
            .on_hover_text("Match case");

        // Right side
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new("🌙").frame(false))
                .on_hover_text("Toggle theme")
                .clicked() {
                *dark_mode = !*dark_mode;
            }

            if ui.add(egui::Button::new("⚙").frame(false))
                .on_hover_text("Settings (Ctrl+,)")
                .clicked() {
                *show_settings = !*show_settings;
            }
        });
    });
});
```

#### 2. Custom Row Styling

```rust
// Apply custom row background
let row_bg = if selected {
    Color32::from_rgba_unmultiplied(14, 99, 156, 77) // Selected
} else if hovered {
    Color32::from_rgba_unmultiplied(255, 255, 255, 13) // Hover
} else if index % 2 == 0 && alternating {
    Color32::from_rgba_unmultiplied(255, 255, 255, 5) // Alternating
} else {
    Color32::TRANSPARENT
};

egui::Frame::new().fill(row_bg).show(ui, |ui| {
    // Row content
});
```

#### 3. Syntax Highlighting

```rust
fn get_syntax_color(token: TextToken, visuals: &egui::Visuals) -> Color32 {
    if visuals.dark_mode {
        match token {
            TextToken::Key => Color32::from_rgb(156, 220, 254),      // #9cdcfe
            TextToken::String => Color32::from_rgb(206, 145, 120),   // #ce9178
            TextToken::Number => Color32::from_rgb(181, 206, 168),   // #b5cea8
            TextToken::Boolean => Color32::from_rgb(86, 156, 214),   // #569cd6
            TextToken::Null => Color32::from_rgb(128, 128, 128),     // #808080
            TextToken::Bracket => Color32::from_rgb(212, 212, 212),  // #d4d4d4
        }
    } else {
        match token {
            TextToken::Key => Color32::from_rgb(0, 16, 128),         // #001080
            TextToken::String => Color32::from_rgb(163, 21, 21),     // #a31515
            TextToken::Number => Color32::from_rgb(9, 134, 88),      // #098658
            TextToken::Boolean => Color32::from_rgb(0, 0, 255),      // #0000ff
            TextToken::Null => Color32::from_rgb(128, 128, 128),     // #808080
            TextToken::Bracket => Color32::BLACK,
        }
    }
}
```

#### 4. Indent Guides

```rust
// Draw vertical indent guides
fn draw_indent_guides(ui: &mut egui::Ui, indent_level: usize, row_rect: Rect) {
    let indent_size = 16.0;
    let guide_color = Color32::from_rgba_unmultiplied(255, 255, 255, 26); // 10% opacity

    for level in 1..=indent_level {
        let x = row_rect.left() + (level as f32 * indent_size);
        let line_start = egui::pos2(x, row_rect.top());
        let line_end = egui::pos2(x, row_rect.bottom());

        ui.painter().line_segment(
            [line_start, line_end],
            egui::Stroke::new(1.0, guide_color)
        );
    }
}
```

#### 5. Status Bar

```rust
egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
    ui.style_mut().visuals.override_text_color = Some(Color32::WHITE);

    let status_bg = Color32::from_rgb(0, 122, 204); // #007acc
    let frame = egui::Frame::new()
        .fill(status_bg)
        .inner_margin(egui::Margin::symmetric(12.0, 6.0));

    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("📄 {}", filename));
            ui.separator();
            ui.label(format!("{} items", item_count));
            ui.separator();
            ui.label(file_type);
            ui.separator();

            let (icon, text) = match status {
                Status::Ready => ("⚡", "Ready"),
                Status::Loading => ("⏳", "Loading..."),
                Status::Error => ("⚠", "Error"),
                Status::Searching => ("🔍", "Searching..."),
            };
            ui.label(format!("{} {}", icon, text));
        });
    });
});
```

#### 6. Better Expand/Collapse Icons

```rust
// Use triangular icons instead of +/-
let toggle_icon = if is_expanded { "▾" } else { "▸" };
let icon_color = Color32::from_rgb(212, 212, 212); // #d4d4d4

if ui.add(
    egui::Label::new(
        egui::RichText::new(toggle_icon)
            .color(icon_color)
            .monospace()
    )
    .sense(egui::Sense::click())
).clicked() {
    // Toggle expansion
}
```

#### 7. Hover Tooltips

```rust
// Add tooltips to all interactive elements
let response = ui.button("📂");
response.on_hover_text_at_pointer("Open file (Ctrl+O)");

// With custom styling
let response = ui.button("⚙");
response.on_hover_ui_at_pointer(|ui| {
    ui.style_mut().visuals.override_text_color = Some(Color32::WHITE);
    ui.label("Settings");
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Ctrl+,").weak());
});
```

---

## Summary

This design system provides a comprehensive blueprint for making Thoth look and feel like a professional, modern developer tool. The VS Code-inspired layout maximizes content area while maintaining clean visual hierarchy and excellent usability.

**Key Takeaways:**
- **Compact toolbar** with integrated search saves vertical space
- **Collapsible sidebar** for navigation without clutter
- **Professional color palette** with accessibility-first approach
- **Keyboard-first** design with shortcuts for all actions
- **4px grid system** ensures consistent spacing
- **Phased implementation** allows incremental improvements

**Next Steps:**
1. Start with Phase 1 (quick wins) to immediately improve aesthetics
2. Implement Phase 2 (layout) for major UX improvements
3. Add Phase 3 features (sidebar, polish) over time

This design will make Thoth competitive with best-in-class JSON viewers while maintaining the performance and simplicity that make it great.

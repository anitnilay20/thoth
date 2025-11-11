# Keyboard Shortcuts

Thoth supports keyboard shortcuts for efficient navigation and operation. All shortcuts are customizable through the settings file.

## Implementation Status

✅ **Fully Implemented** - Working in current version
🚧 **Partially Implemented** - Configured but needs UI integration
📋 **Planned** - Defined for future implementation

## Default Shortcuts

### File Operations ✅

| Action     | macOS | Windows/Linux | Description                | Status     |
| ---------- | ----- | ------------- | -------------------------- | ---------- |
| Open File  | `⌘O`  | `Ctrl+O`      | Open a JSON or NDJSON file | ✅ Working |
| Clear File | `⌘W`  | `Ctrl+W`      | Close the current file     | ✅ Working |
| New Window | `⌘N`  | `Ctrl+N`      | Open a new Thoth window    | ✅ Working |

### UI Controls ✅

| Action       | macOS | Windows/Linux  | Description                     | Status     |
| ------------ | ----- | -------------- | ------------------------------- | ---------- |
| Settings     | `⌘,`  | `Ctrl+,`       | Open/close settings panel       | ✅ Working |
| Toggle Theme | `⌘⇧T` | `Ctrl+Shift+T` | Switch between dark/light theme | ✅ Working |
| Escape       | `Esc` | `Esc`          | Close settings panel            | ✅ Working |

### Navigation 🚧

| Action         | macOS | Windows/Linux  | Description                   | Status  |
| -------------- | ----- | -------------- | ----------------------------- | ------- |
| Focus Search   | `⌘F`  | `Ctrl+F`       | Focus the search input        | 🚧 TODO |
| Next Match     | `⌘G`  | `Ctrl+G`       | Jump to next search match     | 🚧 TODO |
| Previous Match | `⌘⇧G` | `Ctrl+Shift+G` | Jump to previous search match | 🚧 TODO |

**Note**: Navigation shortcuts are configured but require search box focus management implementation.

### Tree Operations 📋

| Action        | Shortcut        | Description                | Status     |
| ------------- | --------------- | -------------------------- | ---------- |
| Expand Node   | `→`             | Expand the selected node   | 📋 Planned |
| Collapse Node | `←`             | Collapse the selected node | 📋 Planned |
| Expand All    | `⌘→` / `Ctrl+→` | Expand all child nodes     | 📋 Planned |
| Collapse All  | `⌘←` / `Ctrl+←` | Collapse all child nodes   | 📋 Planned |

**Note**: Tree operations require JSON viewer keyboard navigation integration.

### Clipboard Operations 📋

| Action      | macOS | Windows/Linux  | Description             | Status     |
| ----------- | ----- | -------------- | ----------------------- | ---------- |
| Copy Key    | `⌘C`  | `Ctrl+C`       | Copy the selected key   | 📋 Planned |
| Copy Value  | `⌘⇧C` | `Ctrl+Shift+C` | Copy the selected value | 📋 Planned |
| Copy Object | `⌘⌥C` | `Ctrl+Alt+C`   | Copy entire JSON object | 📋 Planned |
| Copy Path   | `⌘⇧P` | `Ctrl+Shift+P` | Copy the JSON path      | 📋 Planned |

**Note**: Clipboard operations require JSON viewer selection state integration. Currently available via right-click context menu.

## Customizing Shortcuts

Keyboard shortcuts can be customized by editing the settings file located at:

- **macOS/Linux**: `~/.config/thoth/settings.toml`
- **Windows**: `%APPDATA%\thoth\settings.toml`

### Configuration Format

Shortcuts are defined in the `[shortcuts]` section of the settings file:

```toml
[shortcuts]
open_file = { key = "O", ctrl = false, alt = false, shift = false, command = true }
clear_file = { key = "W", ctrl = false, alt = false, shift = false, command = true }
new_window = { key = "N", ctrl = false, alt = false, shift = false, command = true }
settings = { key = "Comma", ctrl = false, alt = false, shift = false, command = true }
focus_search = { key = "F", ctrl = false, alt = false, shift = false, command = true }
next_match = { key = "G", ctrl = false, alt = false, shift = false, command = true }
prev_match = { key = "G", ctrl = false, alt = false, shift = true, command = true }
toggle_theme = { key = "T", ctrl = false, alt = false, shift = true, command = true }
escape = { key = "Escape", ctrl = false, alt = false, shift = false, command = false }

# Tree operations
expand_node = { key = "ArrowRight", ctrl = false, alt = false, shift = false, command = false }
collapse_node = { key = "ArrowLeft", ctrl = false, alt = false, shift = false, command = false }
expand_all = { key = "ArrowRight", ctrl = false, alt = false, shift = false, command = true }
collapse_all = { key = "ArrowLeft", ctrl = false, alt = false, shift = false, command = true }

# Clipboard operations
copy_key = { key = "C", ctrl = false, alt = false, shift = false, command = true }
copy_value = { key = "C", ctrl = false, alt = false, shift = true, command = true }
copy_object = { key = "C", ctrl = false, alt = true, shift = false, command = true }
copy_path = { key = "P", ctrl = false, alt = false, shift = true, command = true }
```

### Modifier Keys

- **`command`**: Primary modifier (⌘ on macOS, Ctrl on Windows/Linux) - **Use this for cross-platform shortcuts**
- **`ctrl`**: Control key (always Ctrl, even on macOS)
- **`alt`**: Alt/Option key
- **`shift`**: Shift key

### Supported Key Names

**Letters**: `A` through `Z`

**Numbers**: `0` through `9`

**Special Keys**:

- `Escape`, `Enter`, `Tab`, `Space`, `Backspace`, `Delete`
- `ArrowLeft`, `ArrowRight`, `ArrowUp`, `ArrowDown`
- `F1` through `F12`

**Punctuation**:

- `Comma` (,), `Period` (.), `Slash` (/), `Backslash` (\)
- `Semicolon` (;), `Quote` ('), `Backtick` (\`)
- `Minus` (-), `Equal` (=)
- `BracketLeft` ([), `BracketRight` (])

### Example Customizations

#### Use Vim-style navigation

```toml
expand_node = { key = "L", ctrl = false, alt = false, shift = false, command = false }
collapse_node = { key = "H", ctrl = false, alt = false, shift = false, command = false }
```

#### Use F-keys for common actions

```toml
open_file = { key = "F1", ctrl = false, alt = false, shift = false, command = false }
settings = { key = "F2", ctrl = false, alt = false, shift = false, command = false }
```

#### Alternative search shortcuts

```toml
focus_search = { key = "S", ctrl = false, alt = false, shift = false, command = true }
next_match = { key = "N", ctrl = false, alt = false, shift = false, command = true }
prev_match = { key = "P", ctrl = false, alt = false, shift = false, command = true }
```

## Implementation Details

### Architecture

The keyboard shortcut system is built using native egui functionality with zero external dependencies:

1. **`shortcuts.rs`**: Core shortcut types and configuration
   - `Shortcut`: Individual shortcut configuration
   - `KeyboardShortcuts`: Complete shortcut set
   - Cross-platform formatting and parsing

2. **`app/shortcut_handler.rs`**: Shortcut detection and action mapping
   - `ShortcutAction`: Enum of all possible actions
   - `ShortcutHandler`: Detects pressed shortcuts and returns actions

3. **`settings.rs`**: Persistent configuration
   - Shortcuts are part of the main settings TOML file
   - Automatically loaded on startup
   - User-customizable

### Cross-Platform Support

The system uses `egui::Modifiers::COMMAND` which automatically maps to:

- **⌘ (Command)** on macOS
- **Ctrl** on Windows and Linux

This ensures shortcuts feel native on each platform while using a single configuration.

### Adding New Shortcuts

To add a new keyboard shortcut:

1. Add the shortcut to `KeyboardShortcuts` in `src/shortcuts.rs`:

```rust
pub struct KeyboardShortcuts {
    // ... existing shortcuts ...
    pub my_new_shortcut: Shortcut,
}

impl Default for KeyboardShortcuts {
    fn default() -> Self {
        Self {
            // ... existing shortcuts ...
            my_new_shortcut: Shortcut::new("K").command(),
        }
    }
}
```

2. Add the action to `ShortcutAction` in `src/app/shortcut_handler.rs`:

```rust
pub enum ShortcutAction {
    // ... existing actions ...
    MyNewAction,
}
```

3. Add detection in `ShortcutHandler::handle_shortcuts()`:

```rust
if ctx.input_mut(|i| i.consume_shortcut(&shortcuts.my_new_shortcut.to_keyboard_shortcut())) {
    actions.push(ShortcutAction::MyNewAction);
}
```

4. Handle the action in `ThothApp::handle_shortcut_actions()`:

```rust
ShortcutAction::MyNewAction => {
    // Your action implementation
}
```

## Tooltips

All toolbar buttons display their keyboard shortcuts in tooltips when hovering. This helps users discover available shortcuts naturally.

## Notes

- Shortcuts are checked every frame in the order defined
- Multiple shortcuts can be triggered in the same frame
- `consume_shortcut()` prevents the shortcut from being processed twice
- Some shortcuts (tree navigation, clipboard) are passed to the JSON viewer component
- Escape key has special behavior: closes panels if open, clears search otherwise

## Future Enhancements

Potential improvements for the shortcut system:

1. **Visual shortcut editor** in settings panel
2. **Shortcut conflicts detection** and warnings
3. **Shortcut recording** - press keys to set shortcuts
4. **Per-action enable/disable** flags
5. **Shortcut hints overlay** (show all shortcuts on demand)
6. **Import/export** shortcut configurations
7. **Preset configurations** (VS Code-style, Vim-style, etc.)

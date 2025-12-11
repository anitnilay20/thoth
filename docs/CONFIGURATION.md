# Configuration Guide

Thoth uses a comprehensive TOML-based configuration system that allows you to customize nearly every aspect of the application's behavior and appearance.

## Configuration File Location

The configuration file is automatically created on first launch at:

- **Linux/macOS**: `~/.config/thoth/settings.toml`
- **Windows**: `%APPDATA%\thoth\settings.toml`

## Configuration Structure

The configuration is organized into logical sections:

### 1. General Settings

```toml
version = 1           # Configuration version (managed automatically)
dark_mode = true      # Enable dark theme
font_size = 14.0      # UI font size in points (8.0-72.0)
font_family = ""      # Custom font family (optional)
```

### 2. Window Settings

```toml
[window]
default_width = 1200.0   # Default window width in pixels
default_height = 800.0   # Default window height in pixels
```

**Valid ranges:**
- Width: 400-7680 pixels
- Height: 300-4320 pixels

### 3. Update Settings

```toml
[updates]
auto_check = true            # Automatically check for updates
check_interval_hours = 24    # How often to check (1-168 hours)
```

### 4. Performance Settings

Control memory usage and caching behavior:

```toml
[performance]
cache_size = 100             # LRU cache size for parsed JSON (1-10000)
max_file_size_mb = 500       # Maximum file size to load without warning
max_recent_files = 10        # Number of recent files to remember (1-100)
```

**Recommendations:**
- `cache_size`: 100-1000 for most use cases
- Increase cache size for better performance when navigating large files
- Higher cache sizes use more memory

### 5. Viewer Settings

Customize how JSON files are displayed:

```toml
[viewer]
auto_expand_depth = 0        # Auto-expand tree depth on open (0-10)
scroll_margin = 3            # Rows margin before auto-scrolling (0-20)
syntax_highlighting = true   # Enable syntax highlighting
show_line_numbers = false    # Show line numbers
indent_size = 16.0           # Tree indent size in pixels (4.0-64.0)
```

**Auto-expand depth examples:**
- `0` = Everything collapsed (default)
- `1` = Expand root level only
- `2` = Expand two levels deep
- `3` = Expand three levels deep

### 6. UI Settings

Control UI element visibility and layout:

```toml
[ui]
sidebar_width = 350.0           # Default sidebar width (200.0-1000.0)
remember_sidebar_state = true   # Remember sidebar state across sessions
show_status_bar = true          # Show status bar at bottom
show_toolbar = true             # Show toolbar at top
enable_animations = true        # Enable UI animations
```

### 7. Developer Settings

```toml
[dev]
show_profiler = false    # Show performance profiler (requires profiling feature)
```

### 8. Theme Customization

Thoth uses the Catppuccin color scheme with full customization support:

```toml
[theme]
# Base colors
base = "#1e1e2e"
mantle = "#181825"
crust = "#11111b"
text = "#cdd6f4"

# Surface colors
surface0 = "#313244"
surface1 = "#45475a"
surface2 = "#585b70"

# Accent colors
overlay1 = "#7f849c"
key = "#f38ba8"
string = "#a6e3a1"
number = "#fab387"
boolean = "#cba6f7"
bracket = "#89b4fa"

# Status colors
success = "#a6e3a1"
warning = "#f9e2af"
error = "#f38ba8"
info = "#89dceb"

# UI-specific colors
sidebar_hover = "#313244"
sidebar_header = "#7f849c"
indent_guide = "#313244"
selection_stroke = "#89b4fa"
```

**Theme variants:**
- Dark mode uses Catppuccin Mocha
- Light mode uses Catppuccin Latte
- All colors can be customized individually

### 9. Keyboard Shortcuts

All keyboard shortcuts are fully customizable:

```toml
[shortcuts]

[shortcuts.open_file]
key = "O"
command = true    # Cmd on macOS, Ctrl elsewhere
ctrl = false
alt = false
shift = false

[shortcuts.close_file]
key = "W"
command = true

[shortcuts.focus_search]
key = "F"
command = true

[shortcuts.toggle_theme]
key = "T"
command = true
shift = true
```

**Available shortcuts:**
- `open_file` - Open file dialog
- `close_file` - Close current file
- `new_window` - Open new window
- `focus_search` - Focus search input
- `expand_node` - Expand selected node
- `collapse_node` - Collapse selected node
- `copy_key` - Copy selected key
- `copy_value` - Copy selected value
- `toggle_theme` - Switch between dark/light mode
- `toggle_sidebar` - Show/hide sidebar
- `next_match` - Jump to next search result
- `prev_match` - Jump to previous search result
- `escape` - Clear selection/search
- `refresh` - Reload current file

**Key names:**
- Letters: `"A"`, `"B"`, `"C"`, etc.
- Numbers: `"0"`, `"1"`, `"2"`, etc.
- Special: `"ArrowUp"`, `"ArrowDown"`, `"ArrowLeft"`, `"ArrowRight"`
- Special: `"Enter"`, `"Escape"`, `"Tab"`, `"Backspace"`
- Function: `"F1"`, `"F2"`, etc.

## Configuration Validation

Thoth validates all configuration values on load. If invalid values are detected, you'll see a user-friendly error message indicating:

- Which setting is invalid
- The current value
- The valid range or acceptable values
- How to fix the issue

Invalid configurations will prevent the app from starting until corrected.

## Configuration Migration

When you update Thoth, your configuration file is automatically migrated to the latest version. New settings are added with default values, and your customizations are preserved.

The `version` field tracks the configuration schema version and is managed automatically.

## Example: Performance Tuning

### For Large Files (1GB+)

```toml
[performance]
cache_size = 1000           # Increased cache for better performance
max_file_size_mb = 2000     # Allow very large files

[viewer]
auto_expand_depth = 0       # Keep collapsed to avoid slowdown
syntax_highlighting = true  # Keep highlighting for readability
```

### For Many Small Files

```toml
[performance]
cache_size = 500            # Moderate cache
max_recent_files = 50       # Remember more files

[viewer]
auto_expand_depth = 2       # Auto-expand for quick viewing
```

### For Resource-Constrained Systems

```toml
[performance]
cache_size = 50             # Minimal cache
max_file_size_mb = 100      # Smaller files only

[ui]
enable_animations = false   # Disable animations
```

## Resetting Configuration

To reset all settings to defaults:

1. Close Thoth
2. Delete or rename `settings.toml`
3. Restart Thoth (a new config file will be created)

Or use the "Reset to Defaults" button in the Settings UI.

## Configuration Tips

1. **Start with defaults**: The default configuration works well for most use cases
2. **Adjust cache incrementally**: Increase cache_size by 100-200 at a time
3. **Monitor performance**: Use the profiler (`show_profiler = true` with profiling feature)
4. **Backup your config**: Save a copy before making major changes
5. **Use comments**: TOML supports comments with `#` - document your changes!

## Troubleshooting

### Configuration Won't Load

**Error**: "Failed to parse settings file"

**Solutions**:
1. Check for TOML syntax errors (unclosed quotes, missing brackets)
2. Validate with a TOML linter
3. Compare with the default config
4. Reset to defaults if stuck

### Invalid Values

**Error**: "Invalid font_size: 100.0. Must be between 8.0 and 72.0"

**Solutions**:
1. Read the error message carefully
2. Check the valid range in this guide
3. Update the value in settings.toml
4. Restart Thoth

### Missing Settings

If you're missing newer settings after an update:

1. The app auto-adds defaults on load
2. Check the file after launching once
3. New settings appear with default values
4. Customize as needed

## Getting Help

- **Documentation**: See README.md for general usage
- **Issues**: Report configuration problems on GitHub
- **Config file**: Located at `~/.config/thoth/settings.toml`

## Related Documentation

- [README.md](../README.md) - General usage guide
- [CONTRIBUTING.md](../CONTRIBUTING.md) - Development guide
- [CHANGELOG.md](../CHANGELOG.md) - Version history

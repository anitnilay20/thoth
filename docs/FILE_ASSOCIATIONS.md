# File Associations

Thoth supports opening JSON files directly from your operating system. You can double-click `.json`, `.ndjson`, `.jsonl`, and `.geojson` files to open them in Thoth, or set Thoth as your default JSON viewer.

## Command-Line Usage

You can open files directly from the command line:

```bash
thoth document.json
thoth /path/to/data.ndjson
thoth ~/Downloads/geo.geojson
```

Thoth validates the file path and shows helpful error messages if:
- The file doesn't exist
- The path points to a directory instead of a file
- The file extension doesn't match JSON formats (shows warning but still allows opening)

## Platform-Specific Integration

### macOS

After installing Thoth on macOS (via `.dmg` or `.app`), the system automatically registers file associations.

**Setting Thoth as Default App:**
1. Right-click any `.json` file in Finder
2. Select "Get Info" (⌘I)
3. In the "Open with:" section, select "Thoth"
4. Click "Change All..." to apply to all JSON files

**Supported File Types:**
- `.json` - JSON File
- `.ndjson` - NDJSON File
- `.jsonl` - JSON Lines File
- `.geojson` - GeoJSON File

**Open With Menu:**
Right-click any supported file and select "Open With" → "Thoth" to open without changing the default app.

### Windows

After installing Thoth on Windows (via `.msi` installer), the file associations are registered in the Windows Registry.

**Setting Thoth as Default App:**
1. Right-click any `.json` file in File Explorer
2. Select "Open with" → "Choose another app"
3. Select "Thoth" from the list
4. Check "Always use this app to open .json files"
5. Click "OK"

**Supported File Types:**
- `.json` - JSON File
- `.ndjson` - Newline Delimited JSON File
- `.jsonl` - JSON Lines File
- `.geojson` - GeoJSON File

**Context Menu:**
Right-click any supported file and select "Open with" → "Thoth" to open without changing the default app.

### Linux

After installing Thoth on Linux (via `.deb` or `.AppImage`), the `.desktop` file registers the application with the system.

**Setting Thoth as Default App (GNOME/Ubuntu):**
1. Right-click any `.json` file in Files (Nautilus)
2. Select "Properties"
3. Go to the "Open With" tab
4. Select "Thoth" and click "Set as default"

**Setting Thoth as Default App (KDE Plasma):**
1. Right-click any `.json` file in Dolphin
2. Select "Properties"
3. Go to the "General" tab
4. In "Open With", select "Thoth"
5. Click "Apply"

**Supported MIME Types:**
- `application/json` - JSON files (`.json`)
- `application/x-ndjson` - NDJSON files (`.ndjson`, `.jsonl`)
- `application/geo+json` - GeoJSON files (`.geojson`)

**Desktop File Location:**
The desktop entry is installed at `/usr/share/applications/thoth.desktop`

## Troubleshooting

### File Won't Open in Thoth

**macOS:**
- Ensure you have the latest version installed
- Try running from Terminal: `open -a Thoth /path/to/file.json`
- Check System Preferences → Security & Privacy if the app was blocked

**Windows:**
- Ensure the installer completed successfully
- Try running from Command Prompt: `thoth "C:\path\to\file.json"`
- Check if Windows SmartScreen blocked the app

**Linux:**
- Ensure the package is properly installed: `which thoth`
- Try running from terminal: `thoth /path/to/file.json`
- Verify desktop file exists: `ls /usr/share/applications/thoth.desktop`

### "Open With" Menu Doesn't Show Thoth

**macOS:**
Run this command to rebuild the Launch Services database:
```bash
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -kill -r -domain local -domain system -domain user
```

**Windows:**
- Reinstall Thoth to re-register file associations
- Manually associate files via Settings → Apps → Default apps

**Linux:**
Update the desktop database:
```bash
sudo update-desktop-database
```

### Permission Errors When Opening Files

Make sure Thoth has permission to read the file:
```bash
# Check file permissions
ls -l /path/to/file.json

# Make file readable
chmod 644 /path/to/file.json
```

## For Developers

### Building with File Associations

The file associations are configured in `Cargo.toml` under `[package.metadata.packager]`:

```toml
# macOS
[package.metadata.packager.macos]
files = [
  { ext = ["json"], name = "JSON File", role = "Editor", mime = "application/json" },
  # ... more file types
]

# Windows
[package.metadata.packager.windows]
file_associations = [
  { ext = ["json"], name = "JSON File", description = "JSON File", icon = "assets/thoth_icon_256.ico", role = "Editor", mime = "application/json" },
  # ... more file types
]

# Linux
[package.metadata.packager.deb]
desktop_template = "assets/thoth.desktop"
```

### Testing Locally

**Test command-line argument parsing:**
```bash
cargo run -- /path/to/test.json
```

**Build platform packages:**
```bash
# Install cargo-packager
cargo install cargo-packager

# Build for current platform
cargo packager --release

# Build for specific platform (cross-compilation)
cargo packager --release --target x86_64-apple-darwin
```

**Verify file associations after installation:**
- macOS: Check `/Applications/Thoth.app/Contents/Info.plist`
- Windows: Check Registry at `HKEY_CLASSES_ROOT\.json`
- Linux: Check `/usr/share/applications/thoth.desktop`

## Related Documentation

- [Configuration](CONFIGURATION.md) - Application settings
- [Keyboard Shortcuts](KEYBOARD_SHORTCUTS.md) - Navigation shortcuts
- [Release Process](RELEASE_PROCESS.md) - How packages are built and distributed

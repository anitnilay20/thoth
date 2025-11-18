<div align="center">
  <h1>
    <img src="assets/thoth_icon_256.png" alt="Thoth Icon" width="75" style="vertical-align: middle;"/>
    Thoth - JSON & NDJSON Viewer
  </h1>
</div>

<div align="center">
  <img src="assets/app_sreen_recording.gif"/>
</div>

Thoth is a high-performance, feature-rich desktop application for viewing and exploring JSON and NDJSON files with a clean, intuitive interface.

---

## Features

- **Multiple File Format Support**: Handles JSON Objects, JSON Arrays, and NDJSON (Newline-Delimited JSON) files
- **Lazy Loading**: Efficiently handles large files by loading only what's needed
- **Smart File Type Detection**: Automatically detects whether a file is JSON or NDJSON
- **Powerful Search**: Search through complex JSON structures with ease
- **Interactive Exploration**: Expandable tree view to navigate nested structures
- **Copy Support**: Easy copying of JSON paths and values
- **Dark/Light Modes**: Comfortable viewing in any environment
- **Multi-Window Support**: Open multiple independent windows to compare files
- **Keyboard Shortcuts**: Comprehensive keyboard shortcuts for efficient workflow ([see all shortcuts](docs/KEYBOARD_SHORTCUTS.md))
- **Customizable**: All shortcuts and settings configurable via TOML

---

## Installation

### Download Pre-built Binaries

Download the latest release from [GitHub Releases](https://github.com/anitnilay20/thoth/releases).

#### macOS

**Option 1: DMG Installer (Recommended)**

1. Download the appropriate DMG:
   - **Apple Silicon (M1/M2/M3)**: `Thoth-aarch64-apple-darwin.dmg`
   - **Intel**: `Thoth-x86_64-apple-darwin.dmg`
2. Open the DMG and drag Thoth.app to Applications
3. **Important**: Open Terminal and run:
   ```bash
   xattr -cr /Applications/Thoth.app
   ```
4. Launch from Applications

**Option 2: Manual Installation**

1. Download: `thoth-aarch64-apple-darwin.tar.gz` or `thoth-x86_64-apple-darwin.tar.gz`
2. Extract: `tar -xzf thoth-*.tar.gz`
3. Remove quarantine: `xattr -cr Thoth.app`
4. Move to Applications: `mv Thoth.app /Applications/`
5. Double-click to open

> ⚠️ **Gatekeeper Warning**: This app is not code-signed. You must remove the quarantine attribute using the command above before first launch.

#### Windows

**Option 1: MSI Installer (Recommended)**

1. Download `Thoth.msi`
2. Double-click to install
3. Launch from Start Menu

**Option 2: Portable**

1. Download `thoth-x86_64-pc-windows-msvc.zip`
2. Extract and run `thoth.exe`

#### Linux

**Option 1: DEB Package (Recommended for Debian/Ubuntu)**

1. Download `thoth_*_amd64.deb`
2. Install: `sudo dpkg -i thoth_*_amd64.deb`
3. Run: `thoth`

**Option 2: Portable**

1. Download `thoth-x86_64-unknown-linux-gnu.tar.gz`
2. Extract: `tar -xzf thoth-x86_64-unknown-linux-gnu.tar.gz`
3. Make executable: `chmod +x thoth`
4. Run: `./thoth`

### Building from Source

#### Prerequisites

- Rust (latest stable version recommended)
- For building installers: [cargo-packager](https://github.com/crabnebula-dev/cargo-packager)

```bash
# Clone the repository
git clone https://github.com/anitnilay20/thoth.git
cd thoth

# Build and run in debug mode
cargo run

# Build for production
cargo build --release

# The binary will be available in target/release/thoth
```

#### Building Installers

To create platform-specific installers (MSI, DMG, DEB):

```bash
# Install cargo-packager
cargo install cargo-packager --locked

# Build installer for your platform
cargo packager --release

# Installers will be in the dist/ directory:
# - Windows: dist/*.msi
# - macOS: dist/*.dmg and dist/*.app
# - Linux: dist/*.deb
```

See [RELEASE_PROCESS.md](docs/RELEASE_PROCESS.md) for more details on the release and packaging workflow.

---

## Usage

1. Launch the application
2. Use the top bar to open a JSON or NDJSON file (or press `Cmd/Ctrl+O`)
3. Navigate through the file using the tree view
4. Use the search functionality to find specific values (`Cmd/Ctrl+F` to focus)
5. Toggle between dark and light mode as needed (`Cmd/Ctrl+Shift+T`)
6. Open new windows to compare multiple files (`Cmd/Ctrl+N`)

For a complete list of keyboard shortcuts, see the [Keyboard Shortcuts Guide](docs/KEYBOARD_SHORTCUTS.md).

---

## Project Structure

- `src/main.rs`: Application entry point and core logic
- `src/components/`: UI components including the JSON viewer
- `src/file/`: File handling, lazy loading, and type detection
- `src/search/`: Search functionality
- `src/helpers/`: Utility functions and shared code
- `docs/`: Documentation for architecture and design patterns

---

## Documentation

- **[Component Architecture](docs/COMPONENT_ARCHITECTURE.md)**: Detailed guide on Thoth's component system and one-way data binding pattern
- **[Keyboard Shortcuts](docs/KEYBOARD_SHORTCUTS.md)**: Complete reference of all keyboard shortcuts
- **[Design System](docs/DESIGN_SYSTEM.md)**: UI design guidelines and patterns
- **[Profiling](/docs/PROFILING.md)**: Performance profiling and optimization techniques used in Thoth

---

## Architecture

Thoth is built with a modular architecture that emphasizes performance and flexibility:

### Core Components

1. **Application Core (`ThothApp`)**:
   - The central controller that manages the application state
   - Coordinates between UI components and data handling
   - Manages the theme, file paths, and error states

2. **UI Components**:
   - **TopBar**: Handles file opening, type selection, and search inputs
   - **CentralPanel**: Main content area with the JSON viewer
   - **JsonViewer**: Tree-based visualization of JSON structures with expandable nodes
   - **Theme**: Manages dark/light mode and styling

3. **File Handling System**:
   - **Lazy Loading**: Only parses the parts of the file that are being viewed
   - **File Type Detection**: Automatically identifies JSON, JSON arrays, and NDJSON formats
   - **LRU Cache**: Optimizes performance by caching recently accessed nodes

4. **Search Engine**:
   - Parallel processing for fast searching across large files
   - Background scanning to maintain UI responsiveness
   - Uses Rayon for parallel iteration and memchr for optimized substring searching

### Data Flow

```
User Interaction → TopBar → ThothApp → File Loading/Search Operations → CentralPanel → JsonViewer
```

### Performance Optimizations

- **Lazy Parsing**: JSON is only parsed when nodes are expanded
- **Background Processing**: Long operations run in separate threads
- **Memory Efficiency**: Stores file offsets instead of keeping entire files in memory
- **LRU Caching**: Recently accessed nodes are cached for faster repeat access
- **Parallel Search**: Utilizes multiple CPU cores for searching large files

### Error Handling

- Comprehensive error handling using the `anyhow` crate
- Graceful degradation with user-friendly error messages
- Robust file type detection to prevent parsing errors

---

## Technologies

- [Rust](https://www.rust-lang.org/): Primary language
- [egui](https://github.com/emilk/egui): Immediate mode GUI library
- [serde_json](https://github.com/serde-rs/json): JSON serialization/deserialization

---

## Roadmap

Key features planned for future development:

- JSON Path expression support
- Export functionality for nodes and search results
- Multi-file support with tabbed interface
- Schema validation against JSON Schema
- Diff view to compare JSON files
- Search history and improved search capabilities
- Data visualization for numerical values
- JSON editing capabilities
- Cross-platform packages (macOS, Windows, Linux)
- Plugin system for extensibility

---

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## Acknowledgments

- Named after Thoth, the Egyptian deity of wisdom, writing, and knowledge
- Inspired by the need for a fast, native JSON viewer for large files

use std::process::Command;
use std::{env, fs};

fn main() {
    // Re-run if any plugin source or WIT contract changes
    println!("cargo:rerun-if-changed=plugins/");
    println!("cargo:rerun-if-changed=wit/");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let plugins_src = format!("{manifest_dir}/plugins");
    let plugins_dst = format!("{manifest_dir}/assets/plugins");

    let Ok(entries) = fs::read_dir(&plugins_src) else {
        return; // no plugins directory yet — skip silently
    };

    // Check cargo-component availability once before iterating plugins.
    let cargo_component_available = Command::new("cargo-component")
        .arg("--version")
        .output()
        .is_ok();

    for entry in entries.flatten() {
        let plugin_dir = entry.path();
        if !plugin_dir.is_dir() {
            continue;
        }

        let plugin_name = plugin_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let dst_dir = format!("{plugins_dst}/{plugin_name}");
        if let Err(e) = fs::create_dir_all(&dst_dir) {
            println!(
                "cargo:warning=Could not create output directory for '{plugin_name}' ({dst_dir}): {e}"
            );
            continue;
        }

        // ── Theme plugins (no WASM — just copy plugin.toml + theme.json + assets) ──
        if plugin_dir.join("theme.json").exists() {
            // plugin.toml and theme.json are required — skip and clean up if missing.
            if let Err(e) = fs::copy(
                plugin_dir.join("plugin.toml"),
                format!("{dst_dir}/plugin.toml"),
            ) {
                println!(
                    "cargo:warning=Could not copy plugin.toml for '{plugin_name}': {e} — cleaning up"
                );
                let _ = fs::remove_dir_all(&dst_dir);
                continue;
            }
            if let Err(e) = fs::copy(
                plugin_dir.join("theme.json"),
                format!("{dst_dir}/theme.json"),
            ) {
                println!(
                    "cargo:warning=Could not copy theme.json for '{plugin_name}': {e} — cleaning up"
                );
                let _ = fs::remove_dir_all(&dst_dir);
                continue;
            }
            // icon.png is optional.
            let icon_src = plugin_dir.join("icon.png");
            if icon_src.exists() {
                if let Err(e) = fs::copy(&icon_src, format!("{dst_dir}/icon.png")) {
                    println!("cargo:warning=Could not copy icon.png for '{plugin_name}': {e}");
                }
            }
            continue;
        }

        // ── WASM plugins — build with cargo-component then copy artifacts ────────
        if !cargo_component_available {
            println!(
                "cargo:warning=cargo-component not found — skipping plugin '{plugin_name}'. Install with: cargo install cargo-component"
            );
            continue;
        }

        let status = Command::new("cargo")
            .args([
                "component",
                "build",
                "--release",
                "--target",
                "wasm32-wasip1",
                "--manifest-path",
            ])
            .arg(plugin_dir.join("Cargo.toml"))
            .status();

        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                println!("cargo:warning=Plugin '{plugin_name}' build failed with {s}");
                continue;
            }
            Err(e) => {
                println!("cargo:warning=Failed to run cargo-component for '{plugin_name}': {e}");
                continue;
            }
        }

        // In a Cargo workspace the target/ directory is at the workspace root,
        // not inside the individual plugin crate.
        let wasm_src = format!(
            "{manifest_dir}/target/wasm32-wasip1/release/{}.wasm",
            plugin_name.replace('-', "_")
        );

        // Copy .wasm — fatal: without it the plugin is unusable.
        if let Err(e) = fs::copy(&wasm_src, format!("{dst_dir}/plugin.wasm")) {
            println!("cargo:warning=Could not copy {plugin_name}.wasm: {e} — cleaning up");
            let _ = fs::remove_dir_all(&dst_dir);
            continue;
        }

        // Copy plugin.toml — fatal: the host reads it to populate the registry.
        let toml_src = plugin_dir.join("plugin.toml");
        if let Err(e) = fs::copy(&toml_src, format!("{dst_dir}/plugin.toml")) {
            println!(
                "cargo:warning=Could not copy plugin.toml for '{plugin_name}': {e} — cleaning up"
            );
            let _ = fs::remove_dir_all(&dst_dir);
            continue;
        }

        // Copy icon.png if present — optional.
        let icon_src = plugin_dir.join("icon.png");
        if icon_src.exists() {
            if let Err(e) = fs::copy(&icon_src, format!("{dst_dir}/icon.png")) {
                println!("cargo:warning=Could not copy icon.png for '{plugin_name}': {e}");
            }
        }
    }
}

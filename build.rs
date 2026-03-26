use std::process::Command;
use std::{env, fs};

fn main() {
    // Re-run if any plugin source changes
    println!("cargo:rerun-if-changed=plugins/");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let plugins_src = format!("{manifest_dir}/plugins");
    let plugins_dst = format!("{manifest_dir}/assets/plugins");

    let Ok(entries) = fs::read_dir(&plugins_src) else {
        return; // no plugins directory yet — skip silently
    };

    for entry in entries.flatten() {
        let plugin_dir = entry.path();
        if !plugin_dir.is_dir() {
            continue;
        }

        let plugin_name = plugin_dir.file_name().unwrap().to_string_lossy().to_string();

        // Skip silently if cargo-component is not installed
        if Command::new("cargo-component").arg("--version").output().is_err() {
            println!("cargo:warning=cargo-component not found — skipping plugin '{plugin_name}'. Install with: cargo install cargo-component");
            continue;
        }

        // Compile the plugin to WASM
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

        let dst_dir = format!("{plugins_dst}/{plugin_name}");
        fs::create_dir_all(&dst_dir).unwrap();

        // Copy .wasm
        if let Err(e) = fs::copy(&wasm_src, format!("{dst_dir}/plugin.wasm")) {
            println!("cargo:warning=Could not copy {plugin_name}.wasm: {e}");
        }

        // Copy plugin.toml
        let toml_src = plugin_dir.join("plugin.toml");
        if toml_src.exists() {
            fs::copy(toml_src, format!("{dst_dir}/plugin.toml")).ok();
        }

        // Copy icon.png if present
        let icon_src = plugin_dir.join("icon.png");
        if icon_src.exists() {
            fs::copy(icon_src, format!("{dst_dir}/icon.png")).ok();
        }
    }
}

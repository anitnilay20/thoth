//! MCP (Model Context Protocol) server for Thoth.
//!
//! Exposes Thoth's file-loading, search, and data-inspection capabilities
//! as MCP tools over a stdio JSON-RPC transport. This enables AI assistants
//! (Claude, Copilot, Rovo, etc.) to open and query JSON/NDJSON files.

mod server;
mod state;
mod tools;
#[cfg(test)]
mod tests;

pub use server::run_mcp_server;

/// Entry point called from `main()` when the user runs `thoth mcp serve`.
pub fn run_mcp_command(args: &[String]) -> anyhow::Result<()> {
    let subcommand = args.first().map(|s| s.as_str());

    match subcommand {
        Some("serve") => {
            // Build a tokio runtime for the MCP server.
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(run_mcp_server())?;
            Ok(())
        }
        Some("list-tools") => {
            // Quick diagnostic: print tool names to stderr
            eprintln!("Available MCP tools:");
            eprintln!();
            eprintln!("  Phase 1 — Core:");
            eprintln!("  open_file        - Open a JSON/NDJSON file for inspection");
            eprintln!("  close_file       - Close a previously opened file");
            eprintln!("  get_file_info    - Get metadata about an open file");
            eprintln!("  get_record       - Retrieve a record by index");
            eprintln!("  get_record_count - Get the number of records in a file");
            eprintln!("  search           - Search records by text or JSONPath");
            eprintln!();
            eprintln!("  Phase 2 — Data:");
            eprintln!("  get_value_at_path - Extract a nested value using dot-notation path");
            eprintln!("  extract_keys      - List unique keys across records");
            eprintln!("  sample_records    - Return first/last/evenly-spaced sample of records");
            eprintln!("  get_schema        - Infer JSON schema from record samples");
            Ok(())
        }
        _ => {
            eprintln!("Usage: thoth mcp <serve|list-tools>");
            eprintln!();
            eprintln!("Subcommands:");
            eprintln!("  serve       Start the MCP server (stdio transport)");
            eprintln!("  list-tools  List available tools");
            std::process::exit(1);
        }
    }
}

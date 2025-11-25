use crate::error::{Result, ThothError};
use serde_json::Value;

/// Given a full path like "0.user.items[2]" (or "0/_close"), return (root_idx, rel_path).
pub fn split_root_rel(path: &str) -> Result<(usize, &str)> {
    // Strip any "/_close"
    let path = path.strip_suffix("/_close").unwrap_or(path);

    // Leading digits = root index
    let digits_end = path
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(path.len());
    let (root_str, rest) = path.split_at(digits_end);
    let root_idx: usize = root_str
        .parse()
        .map_err(|_| ThothError::InvalidJsonStructure {
            reason: format!("Invalid root index in path: {}", path),
        })?;
    let rel = rest.strip_prefix('.').unwrap_or(rest);
    Ok((root_idx, rel))
}

/// Walk a relative path like "user.items[2].meta" starting at `value`.
fn walk_rel(mut cur: serde_json::Value, mut rel: &str) -> Result<serde_json::Value> {
    while !rel.is_empty() {
        if let Some(rem) = rel.strip_prefix('[') {
            // parse index
            let close = rem
                .find(']')
                .ok_or_else(|| ThothError::InvalidJsonStructure {
                    reason: format!("Unclosed bracket in path: {}", rel),
                })?;
            let idx_str = &rem[..close];
            let idx: usize = idx_str
                .parse()
                .map_err(|_| ThothError::InvalidJsonStructure {
                    reason: format!("Invalid array index: {}", idx_str),
                })?;
            cur = cur
                .get(idx)
                .ok_or_else(|| ThothError::InvalidJsonStructure {
                    reason: format!("Array index {} not found", idx),
                })?
                .clone();
            rel = &rem[close + 1..];
            if rel.starts_with('.') {
                rel = &rel[1..];
            }
        } else {
            // take key until '.' or '['
            let next_sep = rel.find(['.', '[']).unwrap_or(rel.len());
            let key = &rel[..next_sep];
            cur = cur
                .get(key)
                .ok_or_else(|| ThothError::InvalidJsonStructure {
                    reason: format!("Key '{}' not found", key),
                })?
                .clone();
            rel = &rel[next_sep..];
            if rel.starts_with('.') {
                rel = &rel[1..];
            }
        }
    }
    Ok(cur)
}

/// Copy the JSON subtree for `row_path` to the clipboard. Returns true on success.
pub fn get_object_string(root: Value, rel: &str) -> Result<String> {
    let sub = if rel.is_empty() {
        root
    } else {
        walk_rel(root, rel)?
    };

    serde_json::to_string_pretty(&sub).map_err(|e| ThothError::InvalidJsonStructure {
        reason: format!("Failed to serialize JSON: {}", e),
    })
}

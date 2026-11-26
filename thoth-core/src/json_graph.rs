//! Visual JSON graph, mind-map layout, and SVG/Mermaid export (issue #17).
//!
//! Converts an arbitrary `serde_json::Value` into a bounded node-and-edge graph
//! with deterministic 2D coordinates for both top-down tree and radial/horizontal
//! mind-map visualizations, plus clean SVG and Mermaid flowchart export.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Visualization layout style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphLayout {
    #[default]
    Tree,
    MindMap,
}

/// Kind of JSON value represented by a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Object,
    Array,
    String,
    Number,
    Bool,
    Null,
}

/// A positioned node in a [`JsonGraph`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: usize,
    pub parent: Option<usize>,
    pub key: String,
    pub summary: String,
    pub kind: NodeKind,
    pub depth: usize,
    pub x: f32,
    pub y: f32,
}

/// A directed parent → child edge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: usize,
    pub to: usize,
}

/// Bounded graph representation of a JSON document.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub truncated: bool,
}

impl JsonGraph {
    /// Build a graph from `value`, capping traversal at `max_depth` and `max_nodes`.
    pub fn from_value(value: &Value, max_depth: usize, max_nodes: usize, layout: GraphLayout) -> Self {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut truncated = false;

        fn visit(
            val: &Value,
            key: String,
            parent: Option<usize>,
            depth: usize,
            max_depth: usize,
            max_nodes: usize,
            nodes: &mut Vec<GraphNode>,
            edges: &mut Vec<GraphEdge>,
            truncated: &mut bool,
        ) {
            if nodes.len() >= max_nodes {
                *truncated = true;
                return;
            }
            let id = nodes.len();
            let (kind, summary) = summarize(val);
            nodes.push(GraphNode {
                id,
                parent,
                key,
                summary,
                kind,
                depth,
                x: 0.0,
                y: 0.0,
            });
            if let Some(p) = parent {
                edges.push(GraphEdge { from: p, to: id });
            }
            if depth >= max_depth {
                if matches!(val, Value::Object(m) if !m.is_empty())
                    || matches!(val, Value::Array(a) if !a.is_empty())
                {
                    *truncated = true;
                }
                return;
            }
            match val {
                Value::Object(map) => {
                    for (k, v) in map {
                        if nodes.len() >= max_nodes {
                            *truncated = true;
                            break;
                        }
                        visit(v, k.clone(), Some(id), depth + 1, max_depth, max_nodes, nodes, edges, truncated);
                    }
                }
                Value::Array(arr) => {
                    for (idx, v) in arr.iter().enumerate() {
                        if nodes.len() >= max_nodes {
                            *truncated = true;
                            break;
                        }
                        visit(v, format!("[{idx}]"), Some(id), depth + 1, max_depth, max_nodes, nodes, edges, truncated);
                    }
                }
                _ => {}
            }
        }

        visit(
            value,
            "root".to_string(),
            None,
            0,
            max_depth,
            max_nodes.max(1),
            &mut nodes,
            &mut edges,
            &mut truncated,
        );

        let mut graph = Self {
            nodes,
            edges,
            truncated,
        };
        graph.apply_layout(layout);
        graph
    }

    /// Recompute node `(x, y)` coordinates for `layout`.
    pub fn apply_layout(&mut self, layout: GraphLayout) {
        match layout {
            GraphLayout::Tree => {
                let mut per_depth: std::collections::BTreeMap<usize, Vec<usize>> =
                    std::collections::BTreeMap::new();
                for n in &self.nodes {
                    per_depth.entry(n.depth).or_default().push(n.id);
                }
                for (depth, ids) in per_depth {
                    let count = ids.len() as f32;
                    for (idx, id) in ids.into_iter().enumerate() {
                        let offset = (idx as f32 - (count - 1.0) / 2.0) * 180.0;
                        self.nodes[id].x = offset;
                        self.nodes[id].y = depth as f32 * 110.0;
                    }
                }
            }
            GraphLayout::MindMap => {
                // Horizontal mind-map: root on left, children fan out vertically by depth.
                let mut per_depth: std::collections::BTreeMap<usize, Vec<usize>> =
                    std::collections::BTreeMap::new();
                for n in &self.nodes {
                    per_depth.entry(n.depth).or_default().push(n.id);
                }
                for (depth, ids) in per_depth {
                    let count = ids.len() as f32;
                    for (idx, id) in ids.into_iter().enumerate() {
                        let offset = (idx as f32 - (count - 1.0) / 2.0) * 72.0;
                        self.nodes[id].x = depth as f32 * 220.0;
                        self.nodes[id].y = offset;
                    }
                }
            }
        }
    }

    /// Render the graph as a standalone SVG diagram.
    pub fn to_svg(&self) -> String {
        if self.nodes.is_empty() {
            return r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"></svg>"#
                .to_string();
        }
        let pad = 100.0f32;
        let min_x = self.nodes.iter().map(|n| n.x).fold(f32::INFINITY, f32::min) - pad;
        let max_x = self.nodes.iter().map(|n| n.x).fold(f32::NEG_INFINITY, f32::max) + pad + 140.0;
        let min_y = self.nodes.iter().map(|n| n.y).fold(f32::INFINITY, f32::min) - pad;
        let max_y = self.nodes.iter().map(|n| n.y).fold(f32::NEG_INFINITY, f32::max) + pad + 40.0;
        let w = (max_x - min_x).max(240.0);
        let h = (max_y - min_y).max(160.0);

        let mut svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{min_x:.1} {min_y:.1} {w:.1} {h:.1}" width="{w:.0}" height="{h:.0}">"#
        );
        svg.push_str(r#"<style>text{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:11px;fill:#e6edf3}.edge{stroke:#4b5563;stroke-width:1.5;fill:none}.node{stroke:#60a5fa;stroke-width:1.4;rx:6}</style>"#);

        for e in &self.edges {
            let a = &self.nodes[e.from];
            let b = &self.nodes[e.to];
            svg.push_str(&format!(
                r#"<path class="edge" d="M {:.1} {:.1} L {:.1} {:.1}"/>"#,
                a.x + 70.0,
                a.y + 18.0,
                b.x + 70.0,
                b.y + 18.0
            ));
        }

        for n in &self.nodes {
            let fill = match n.kind {
                NodeKind::Object => "#1e293b",
                NodeKind::Array => "#1e3a5f",
                NodeKind::String => "#14532d",
                NodeKind::Number => "#3b0764",
                NodeKind::Bool => "#7c2d12",
                NodeKind::Null => "#374151",
            };
            let label = escape_xml(&format!("{}: {}", n.key, n.summary));
            svg.push_str(&format!(
                r#"<rect class="node" x="{:.1}" y="{:.1}" width="140" height="36" fill="{fill}"/><text x="{:.1}" y="{:.1}">{label}</text>"#,
                n.x,
                n.y,
                n.x + 8.0,
                n.y + 22.0
            ));
        }

        svg.push_str("</svg>");
        svg
    }

    /// Export the graph as a Mermaid `flowchart TD` diagram.
    pub fn to_mermaid(&self) -> String {
        let mut out = String::from("flowchart TD\n");
        for n in &self.nodes {
            let label = format!("{}: {}", n.key, n.summary).replace('"', "'");
            out.push_str(&format!("  n{}[\"{}\"]\n", n.id, label));
        }
        for e in &self.edges {
            out.push_str(&format!("  n{} --> n{}\n", e.from, e.to));
        }
        out
    }
}

fn summarize(v: &Value) -> (NodeKind, String) {
    match v {
        Value::Object(m) => (NodeKind::Object, format!("{{{}}}", m.len())),
        Value::Array(a) => (NodeKind::Array, format!("[{}]", a.len())),
        Value::String(s) => {
            let clipped: String = s.chars().take(16).collect();
            (NodeKind::String, format!("\"{clipped}\""))
        }
        Value::Number(n) => (NodeKind::Number, n.to_string()),
        Value::Bool(b) => (NodeKind::Bool, b.to_string()),
        Value::Null => (NodeKind::Null, "null".to_string()),
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_graph_and_exports_svg_and_mermaid() {
        let doc = json!({
            "user": { "name": "Ada", "roles": ["admin", "dev"] },
            "active": true
        });
        let graph = JsonGraph::from_value(&doc, 4, 50, GraphLayout::Tree);
        assert_eq!(graph.nodes.len(), 7);
        assert_eq!(graph.edges.len(), 6);
        assert!(!graph.truncated);

        let svg = graph.to_svg();
        assert!(svg.starts_with("<svg "));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("user: {2}"));

        let mmd = graph.to_mermaid();
        assert!(mmd.starts_with("flowchart TD"));
        assert!(mmd.contains("n0 --> n1"));
    }

    #[test]
    fn caps_nodes_on_large_structures() {
        let arr: Vec<i32> = (0..100).collect();
        let doc = Value::Array(arr.into_iter().map(Value::from).collect());
        let graph = JsonGraph::from_value(&doc, 4, 15, GraphLayout::MindMap);
        assert_eq!(graph.nodes.len(), 15);
        assert!(graph.truncated);
    }
}

use serde_json::Value;

pub fn format_simple_kv(key: &str, val: &Value) -> String {
    match val {
        Value::String(s) => format!("\"{key}\": \"{s}\""),
        _ => format!("\"{key}\": {}", preview_value(val)),
    }
}

pub fn preview_value(val: &Value) -> String {
    match val {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            // truncate long strings for list view
            const MAX: usize = 120;
            if s.len() > MAX {
                let mut t = s[..MAX].to_string();
                t.push('…');
                format!("\"{t}\"")
            } else {
                format!("\"{s}\"")
            }
        }
        Value::Array(a) => format!("[{}]", a.len()),
        Value::Object(o) => format!("{{{}}}", o.len()),
    }
}

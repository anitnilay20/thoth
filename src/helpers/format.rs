use serde_json::Value;

pub fn format_simple_kv(key: &str, val: &Value) -> String {
    match val {
        Value::String(s) => {
            let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{key}\": \"{escaped}\"")
        }
        _ => format!("\"{key}\": {}", preview_value(val)),
    }
}

pub fn preview_value(val: &Value) -> String {
    match val {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
            // truncate long strings for list view
            const MAX: usize = 120;
            if escaped.len() > MAX {
                let safe_max = escaped
                    .char_indices()
                    .take_while(|(i, _)| *i < MAX)
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(0);
                let mut t = escaped[..safe_max].to_string();
                t.push('…');
                format!("\"{t}\"")
            } else {
                format!("\"{escaped}\"")
            }
        }
        Value::Array(a) => format!("[{}]", a.len()),
        Value::Object(o) => format!("{{{}}}", o.len()),
    }
}

pub fn format_date(date: &str) -> String {
    if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(date) {
        format_date_static(&datetime)
    } else {
        "".to_string()
    }
}

pub fn format_date_static<Tz: chrono::TimeZone>(datetime: &chrono::DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(datetime.with_timezone(&chrono::Utc));

    // Show relative time for recent dates (less than 7 days)
    if duration.num_days() < 7 {
        if duration.num_minutes() < 1 {
            "just now".to_string()
        } else if duration.num_minutes() < 60 {
            let mins = duration.num_minutes();
            format!("{} min{} ago", mins, if mins == 1 { "" } else { "s" })
        } else if duration.num_hours() < 24 {
            let hours = duration.num_hours();
            format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
        } else {
            let days = duration.num_days();
            format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
        }
    } else {
        // For older dates, show formatted date in user's local timezone
        let local = datetime.with_timezone(&chrono::Local);
        local.format("%b %d, %Y at %I:%M %p").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_simple_kv_escapes_quotes_in_string() {
        let val = json!("https://example.com/?q=\"test\"");
        let result = format_simple_kv("url", &val);
        assert!(
            result.contains("\\\"test\\\""),
            "Quotes in value should be escaped, got: {}",
            result
        );
        let quote_positions: Vec<usize> = result
            .char_indices()
            .filter(|&(i, c)| {
                if c != '"' {
                    return false;
                }
                // Count consecutive backslashes before this quote
                let preceding_backslashes = result[..i]
                    .chars()
                    .rev()
                    .take_while(|&ch| ch == '\\')
                    .count();
                preceding_backslashes % 2 == 0
            })
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            quote_positions.len(),
            4,
            "Should have exactly 4 unescaped quotes, got {} in: {}",
            quote_positions.len(),
            result
        );
    }

    #[test]
    fn test_format_simple_kv_escapes_backslashes() {
        let val = json!("path\\to\\file");
        let result = format_simple_kv("path", &val);
        assert!(
            result.contains("path\\\\to\\\\file"),
            "Backslashes should be escaped, got: {}",
            result
        );
    }

    #[test]
    fn test_preview_value_escapes_quotes() {
        let val = json!("say \"hello\"");
        let result = preview_value(&val);
        assert!(
            result.contains("\\\"hello\\\""),
            "Quotes in preview should be escaped, got: {}",
            result
        );
    }

    #[test]
    fn test_preview_value_escapes_backslashes() {
        let val = json!("back\\slash");
        let result = preview_value(&val);
        assert!(
            result.contains("back\\\\slash"),
            "Backslashes in preview should be escaped, got: {}",
            result
        );
    }

    #[test]
    fn test_format_simple_kv_normal_string_unchanged() {
        let val = json!("hello world");
        let result = format_simple_kv("greeting", &val);
        assert_eq!(result, "\"greeting\": \"hello world\"");
    }

    #[test]
    fn test_preview_value_primitives() {
        assert_eq!(preview_value(&json!(null)), "null");
        assert_eq!(preview_value(&json!(true)), "true");
        assert_eq!(preview_value(&json!(42)), "42");
        assert_eq!(preview_value(&json!("hello")), "\"hello\"");
    }
}

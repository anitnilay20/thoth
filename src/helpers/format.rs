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

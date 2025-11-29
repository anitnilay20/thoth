use serde_json::Value;
use std::ops::Range;

use crate::helpers::preview_value;
use crate::search::results::FieldComponent;

#[derive(Debug, Clone)]
pub struct JsonPathQuery {
    original: String,
    segments: Vec<PathSegment>,
    filter: Option<FilterValue>,
}

#[derive(Debug, Clone)]
enum PathSegment {
    Field(String),
    FieldWildcard,
    ArrayIndex(usize),
    ArrayWildcard,
}

#[derive(Debug, Clone)]
enum FilterValue {
    Equals(Value),
}

#[derive(Debug, Clone)]
pub struct JsonPathMatch {
    pub path: String,
    pub component: FieldComponent,
    pub matched_text: Option<String>,
    pub highlight_range: Option<Range<usize>>,
    pub display_value: String,
}

#[derive(Debug, Clone)]
pub enum JsonPathError {
    Empty,
    MissingRoot,
    InvalidToken(String),
    UnterminatedBracket,
    InvalidFilter(String),
}

impl std::fmt::Display for JsonPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonPathError::Empty => write!(f, "JSONPath query cannot be empty"),
            JsonPathError::MissingRoot => {
                write!(
                    f,
                    "JSONPath query must start with '$' to reference the root"
                )
            }
            JsonPathError::InvalidToken(tok) => {
                write!(f, "Invalid JSONPath token near '{tok}'")
            }
            JsonPathError::UnterminatedBracket => {
                write!(f, "JSONPath query has an unterminated bracket segment")
            }
            JsonPathError::InvalidFilter(reason) => {
                write!(f, "Invalid filter expression: {reason}")
            }
        }
    }
}

impl std::error::Error for JsonPathError {}

impl JsonPathQuery {
    pub fn parse(input: &str) -> Result<Self, JsonPathError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(JsonPathError::Empty);
        }

        let (expr_part, filter_part) = split_expression_and_filter(trimmed);
        if expr_part.is_empty() {
            return Err(JsonPathError::MissingRoot);
        }
        if !expr_part.starts_with('$') {
            return Err(JsonPathError::MissingRoot);
        }

        let segments = parse_segments(&expr_part)?;
        let filter = if let Some(raw_filter) = filter_part {
            Some(FilterValue::parse(&raw_filter)?)
        } else {
            None
        };

        Ok(Self {
            original: trimmed.to_string(),
            segments,
            filter,
        })
    }

    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn evaluate(&self, root: &Value, root_path: &str, match_case: bool) -> Vec<JsonPathMatch> {
        let mut current: Vec<(String, &Value)> = vec![(root_path.to_string(), root)];
        for segment in &self.segments {
            let mut next: Vec<(String, &Value)> = Vec::new();
            for (path, value) in &current {
                segment.apply(path, value, &mut next);
            }
            if next.is_empty() {
                return Vec::new();
            }
            current = next;
        }

        let mut matches = Vec::new();
        for (path, value) in current {
            if self.matches_filter(value, match_case) {
                if let Some(entry) = JsonPathMatch::from_value(path, value) {
                    matches.push(entry);
                }
            }
        }
        matches
    }

    fn matches_filter(&self, value: &Value, match_case: bool) -> bool {
        let Some(filter) = &self.filter else {
            return true;
        };
        match filter {
            FilterValue::Equals(expected) => match (expected, value) {
                (Value::String(exp), Value::String(actual)) => {
                    if match_case {
                        actual == exp
                    } else {
                        actual.eq_ignore_ascii_case(exp)
                    }
                }
                _ => value == expected,
            },
        }
    }
}

impl JsonPathMatch {
    fn from_value(path: String, value: &Value) -> Option<Self> {
        match value {
            Value::String(s) => Some(Self {
                path,
                component: FieldComponent::Value,
                matched_text: Some(s.clone()),
                highlight_range: Some(0..s.len()),
                display_value: format!("\"{}\"", s),
            }),
            Value::Number(num) => {
                let text = num.to_string();
                Some(Self {
                    path,
                    component: FieldComponent::Value,
                    matched_text: Some(text.clone()),
                    highlight_range: Some(0..text.len()),
                    display_value: text,
                })
            }
            Value::Bool(flag) => {
                let text = flag.to_string();
                Some(Self {
                    path,
                    component: FieldComponent::Value,
                    matched_text: Some(text.clone()),
                    highlight_range: Some(0..text.len()),
                    display_value: text,
                })
            }
            Value::Null => Some(Self {
                path,
                component: FieldComponent::Value,
                matched_text: Some("null".to_string()),
                highlight_range: Some(0..4),
                display_value: "null".to_string(),
            }),
            other => Some(Self {
                path,
                component: FieldComponent::EntireRow,
                matched_text: None,
                highlight_range: None,
                display_value: preview_value(other),
            }),
        }
    }
}

fn parse_segments(expr: &str) -> Result<Vec<PathSegment>, JsonPathError> {
    let mut chars = expr.chars().peekable();
    // consume '$'
    chars.next();
    let mut segments = Vec::new();

    while let Some(ch) = chars.peek() {
        match ch {
            '.' => {
                chars.next();
                if chars.peek().is_none() {
                    break;
                }
                if let Some('*') = chars.peek() {
                    chars.next();
                    segments.push(PathSegment::FieldWildcard);
                } else {
                    let name = parse_field_name(&mut chars)?;
                    segments.push(PathSegment::Field(name));
                }
            }
            '[' => {
                chars.next();
                consume_whitespace(&mut chars);
                let next = chars
                    .peek()
                    .cloned()
                    .ok_or(JsonPathError::UnterminatedBracket)?;
                match next {
                    '*' => {
                        chars.next();
                        consume_whitespace(&mut chars);
                        expect_char(&mut chars, ']')?;
                        segments.push(PathSegment::ArrayWildcard);
                    }
                    '0'..='9' => {
                        let index = parse_number(&mut chars)?;
                        consume_whitespace(&mut chars);
                        expect_char(&mut chars, ']')?;
                        segments.push(PathSegment::ArrayIndex(index));
                    }
                    '\'' | '"' => {
                        let field = parse_quoted_field(&mut chars)?;
                        consume_whitespace(&mut chars);
                        expect_char(&mut chars, ']')?;
                        segments.push(PathSegment::Field(field));
                    }
                    _ => {
                        return Err(JsonPathError::InvalidToken(expr.to_string()));
                    }
                }
            }
            _ => {
                return Err(JsonPathError::InvalidToken(expr.to_string()));
            }
        }
    }

    Ok(segments)
}

fn parse_field_name<I>(chars: &mut std::iter::Peekable<I>) -> Result<String, JsonPathError>
where
    I: Iterator<Item = char>,
{
    let mut name = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            name.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    if name.is_empty() {
        Err(JsonPathError::InvalidToken("Expected field name".into()))
    } else {
        Ok(name)
    }
}

fn parse_number<I>(chars: &mut std::iter::Peekable<I>) -> Result<usize, JsonPathError>
where
    I: Iterator<Item = char>,
{
    let mut digits = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    digits
        .parse()
        .map_err(|_| JsonPathError::InvalidToken("Invalid array index".into()))
}

fn parse_quoted_field<I>(chars: &mut std::iter::Peekable<I>) -> Result<String, JsonPathError>
where
    I: Iterator<Item = char>,
{
    let quote = chars.next().ok_or(JsonPathError::UnterminatedBracket)?;
    let mut name = String::new();
    while let Some(&ch) = chars.peek() {
        chars.next();
        if ch == quote {
            return Ok(name);
        } else {
            name.push(ch);
        }
    }
    Err(JsonPathError::UnterminatedBracket)
}

fn expect_char<I>(chars: &mut std::iter::Peekable<I>, expected: char) -> Result<(), JsonPathError>
where
    I: Iterator<Item = char>,
{
    match chars.next() {
        Some(ch) if ch == expected => Ok(()),
        _ => Err(JsonPathError::UnterminatedBracket),
    }
}

fn consume_whitespace<I>(chars: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
}

fn split_expression_and_filter(input: &str) -> (String, Option<String>) {
    let mut in_single = false;
    let mut in_double = false;
    for (idx, ch) in input.char_indices() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '=' if !in_single && !in_double => {
                let expr = input[..idx].trim().to_string();
                let mut filter_part = input[idx + 1..].trim().to_string();
                if filter_part.starts_with('=') {
                    filter_part = filter_part[1..].trim_start().to_string();
                }
                return (expr, Some(filter_part));
            }
            _ => {}
        }
    }
    (input.to_string(), None)
}

impl FilterValue {
    fn parse(raw: &str) -> Result<Self, JsonPathError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(JsonPathError::InvalidFilter("Value is empty".into()));
        }

        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
            return Ok(FilterValue::Equals(val));
        }

        // Support single-quoted strings by converting to double quotes
        if trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2 {
            let inner = &trimmed[1..trimmed.len() - 1];
            let normalized = format!("\"{}\"", inner);
            if let Ok(val) = serde_json::from_str::<Value>(&normalized) {
                return Ok(FilterValue::Equals(val));
            }
        }

        // Fallback: treat as plain string literal
        let escaped = trimmed.replace('"', "\\\"");
        let wrapped = format!("\"{}\"", escaped);
        serde_json::from_str::<Value>(&wrapped)
            .map(FilterValue::Equals)
            .map_err(|e| JsonPathError::InvalidFilter(e.to_string()))
    }
}

impl PathSegment {
    fn apply<'a>(&self, current_path: &str, value: &'a Value, out: &mut Vec<(String, &'a Value)>) {
        match self {
            PathSegment::Field(name) => {
                if let Value::Object(map) = value {
                    if let Some(child) = map.get(name) {
                        out.push((format!("{}.{}", current_path, name), child));
                    }
                }
            }
            PathSegment::FieldWildcard => {
                if let Value::Object(map) = value {
                    for (key, child) in map.iter() {
                        out.push((format!("{}.{}", current_path, key), child));
                    }
                }
            }
            PathSegment::ArrayIndex(idx) => {
                if let Value::Array(items) = value {
                    if let Some(child) = items.get(*idx) {
                        out.push((format!("{}[{}]", current_path, idx), child));
                    }
                }
            }
            PathSegment::ArrayWildcard => {
                if let Value::Array(items) = value {
                    for (i, child) in items.iter().enumerate() {
                        out.push((format!("{}[{}]", current_path, i), child));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_basic_path() {
        let query = JsonPathQuery::parse("$.store.book[0].author").unwrap();
        assert_eq!(query.segments.len(), 4);
    }

    #[test]
    fn parses_filter() {
        let query = JsonPathQuery::parse("$.user.name = \"alice\"").unwrap();
        assert!(query.filter.is_some());
    }

    #[test]
    fn evaluates_simple_match() {
        let query = JsonPathQuery::parse("$.user.name = 'alice'").unwrap();
        let value = json!({"user": {"name": "Alice"}});
        let matches = query.evaluate(&value, "0", false);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "0.user.name");
    }

    #[test]
    fn respects_case_option() {
        let query = JsonPathQuery::parse("$.user.name = 'alice'").unwrap();
        let value = json!({"user": {"name": "Alice"}});
        assert!(query.evaluate(&value, "0", false).len() == 1);
        assert!(query.evaluate(&value, "0", true).is_empty());
    }
}

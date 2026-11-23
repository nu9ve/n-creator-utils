//! Observed text on its way to a model (nu9ve-mcp-v1 §4). Moved from the first app's MCP
//! server (docs/suite-mcp.md §7), with C1 controls also removed.

use serde_json::Value;

pub const MAX_TEXT: usize = 300;

fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}')
}

/// Strings lose control and bidirectional characters and keep at most 300 characters;
/// arrays and objects are cleaned inside.
pub fn clean(value: &Value) -> Value {
    match value {
        Value::String(s) => {
            Value::String(s.chars().filter(|c| !hidden(*c)).take(MAX_TEXT).collect())
        }
        Value::Array(a) => Value::Array(a.iter().map(clean).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), clean(v))).collect()),
        other => other.clone(),
    }
}

/// Clean every value under a key ending in `_untrusted`, anywhere in a reply.
pub fn clean_untrusted(value: &Value) -> Value {
    match value {
        Value::Array(a) => Value::Array(a.iter().map(clean_untrusted).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, v)| {
                    let v = if k.ends_with("_untrusted") {
                        clean(v)
                    } else {
                        clean_untrusted(v)
                    };
                    (k.clone(), v)
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn removes_hidden_characters_and_bounds() {
        let long = format!("a\u{202e}b\n{}", "x".repeat(400));
        let cleaned = clean(&json!(long));
        assert!(cleaned.as_str().unwrap().starts_with("abx"));
        assert_eq!(cleaned.as_str().unwrap().chars().count(), MAX_TEXT);
    }

    #[test]
    fn only_untrusted_fields_are_touched() {
        let reply = json!({"name": "a\nb", "items": [{"page_text_untrusted": "c\u{200f}d"}]});
        assert_eq!(
            clean_untrusted(&reply),
            json!({"name": "a\nb", "items": [{"page_text_untrusted": "cd"}]})
        );
    }
}

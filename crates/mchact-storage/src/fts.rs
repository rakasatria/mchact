// crates/mchact-storage/src/fts.rs

/// Sanitize a raw user query for safe use in FTS5 MATCH expressions.
/// Strips all FTS5 operators and quotes each token individually.
/// Returns `None` if the input is empty after sanitization.
pub fn sanitize_fts_query(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .map(|c| match c {
            '"' | '*' | '(' | ')' | '+' | '-' | '^' | '~' | ':' | '{' | '}' | '[' | ']' => ' ',
            _ => c,
        })
        .collect();
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let expr = tokens
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ");
    Some(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        assert_eq!(sanitize_fts_query(""), None);
        assert_eq!(sanitize_fts_query("   "), None);
    }

    #[test]
    fn test_only_special_chars() {
        assert_eq!(sanitize_fts_query("+-*()"), None);
        assert_eq!(sanitize_fts_query("\"\"\""), None);
    }

    #[test]
    fn test_normal_words() {
        assert_eq!(
            sanitize_fts_query("hello world"),
            Some("\"hello\" \"world\"".to_string())
        );
    }

    #[test]
    fn test_strips_operators() {
        assert_eq!(
            sanitize_fts_query("hello AND (world)"),
            Some("\"hello\" \"AND\" \"world\"".to_string())
        );
    }

    #[test]
    fn test_mixed_special_and_words() {
        assert_eq!(
            sanitize_fts_query("deploy*ment +pipeline"),
            Some("\"deploy\" \"ment\" \"pipeline\"".to_string())
        );
    }

    #[test]
    fn test_cjk_characters() {
        assert_eq!(
            sanitize_fts_query("hello \u{4F60}\u{597D}"),
            Some("\"hello\" \"\u{4F60}\u{597D}\"".to_string())
        );
    }
}

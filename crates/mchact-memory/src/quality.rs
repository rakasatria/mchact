// ---------------------------------------------------------------------------
// Quality gates for observation content
// ---------------------------------------------------------------------------

/// Collapse whitespace, trim, and truncate to `max_chars` at a char boundary.
/// Returns `None` if the result is empty after cleaning.
pub fn normalize_content(input: &str, max_chars: usize) -> Option<String> {
    let collapsed: String = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    let truncated: String = collapsed.chars().take(max_chars).collect();
    Some(truncated)
}

/// Reject content that is too short, small talk, uncertain, or has no signal.
///
/// Evaluation order:
/// 1. Trivially short (< 3 chars) → "too short"
/// 2. Exact small-talk phrase match → "small talk"
/// 3. Under minimum length (< 8 chars) → "too short"
/// 4. Uncertain language → "uncertain statement"
/// 5. No alphanumeric signal → "no signal"
pub fn quality_check(content: &str) -> Result<(), &'static str> {
    let char_count = content.chars().count();

    // Reject trivially short content before attempting small-talk matching.
    if char_count < 3 {
        return Err("too short");
    }

    let lower = content.to_lowercase();

    let small_talk = ["hi", "hello", "thanks", "thank you", "ok", "okay", "lol", "haha"];
    for phrase in small_talk {
        if lower == phrase {
            return Err("small talk");
        }
    }

    if char_count < 8 {
        return Err("too short");
    }

    let uncertain = ["maybe", "i think", "not sure", "guess"];
    for phrase in uncertain {
        if lower.contains(phrase) {
            return Err("uncertain statement");
        }
    }

    let has_alphanumeric = content.chars().any(|c| c.is_alphanumeric());
    if !has_alphanumeric {
        return Err("no signal");
    }

    Ok(())
}

/// Detect PII (email addresses) and secrets (API keys, tokens) in content.
pub fn pii_check(content: &str) -> Result<(), &'static str> {
    let secret_prefixes = ["sk-", "pk-", "ghp_", "gho_", "xoxb-", "xapp-", "AKIA", "Bearer "];
    for prefix in secret_prefixes {
        if content.contains(prefix) {
            return Err("contains secret");
        }
    }

    for word in content.split_whitespace() {
        if word.contains('@') && word.contains('.') && word.len() > 5 {
            return Err("contains PII (email)");
        }
    }

    Ok(())
}

/// Guard against prompt injection or poisoning attempts.
/// Returns `true` if the content is safe, `false` if it looks like poisoning.
pub fn poisoning_check(content: &str) -> bool {
    let poison_patterns = [
        "tool calls were broken",
        "auth fails",
        "not following instructions",
        "tool execution failed",
        "api returned error",
    ];

    let lower = content.to_lowercase();

    let is_poison = poison_patterns.iter().any(|p| lower.contains(p));
    if !is_poison {
        return true;
    }

    let corrective_prefixes = ["todo:", "ensure", "fix:", "action:"];
    let starts_corrective = corrective_prefixes
        .iter()
        .any(|p| lower.starts_with(p));

    starts_corrective
}

/// Run all quality gates in sequence and return the first failure.
pub fn validate_observation(content: &str) -> Result<(), &'static str> {
    quality_check(content)?;
    pii_check(content)?;
    if !poisoning_check(content) {
        return Err("poisoning attempt");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // normalize_content

    #[test]
    fn test_normalize_trims_and_collapses() {
        let result = normalize_content("  hello   world  ", 100);
        assert_eq!(result, Some("hello world".to_string()));
    }

    #[test]
    fn test_normalize_truncates() {
        let result = normalize_content("abcdefghij", 5);
        assert_eq!(result, Some("abcde".to_string()));
    }

    #[test]
    fn test_normalize_empty() {
        let result = normalize_content("   ", 100);
        assert_eq!(result, None);
    }

    // quality_check

    #[test]
    fn test_quality_ok_valid() {
        assert_eq!(quality_check("user prefers Rust for backend work"), Ok(()));
    }

    #[test]
    fn test_quality_rejects_short() {
        assert_eq!(quality_check("hi"), Err("too short"));
    }

    #[test]
    fn test_quality_rejects_small_talk() {
        assert_eq!(quality_check("thanks"), Err("small talk"));
    }

    #[test]
    fn test_quality_rejects_uncertain() {
        assert_eq!(
            quality_check("maybe they like Python"),
            Err("uncertain statement")
        );
    }

    #[test]
    fn test_quality_rejects_no_signal() {
        assert_eq!(quality_check("........"), Err("no signal"));
    }

    // pii_check

    #[test]
    fn test_pii_detects_email() {
        assert_eq!(
            pii_check("email is alice@example.com"),
            Err("contains PII (email)")
        );
    }

    #[test]
    fn test_pii_detects_api_key() {
        assert_eq!(
            pii_check("my key is sk-1234567890abcdef"),
            Err("contains secret")
        );
    }

    #[test]
    fn test_pii_clean_content_passes() {
        assert_eq!(pii_check("user prefers dark mode"), Ok(()));
    }

    // poisoning_check

    #[test]
    fn test_poisoning_guard_rejects_broken_behavior() {
        assert!(!poisoning_check(
            "tool calls were broken and auth fails"
        ));
    }

    #[test]
    fn test_poisoning_guard_allows_corrective() {
        assert!(poisoning_check(
            "TODO: ensure auth tokens are refreshed"
        ));
    }
}

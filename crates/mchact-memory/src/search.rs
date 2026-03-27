use crate::types::RankedResult;

// ---------------------------------------------------------------------------
// FTS5 query sanitization
// ---------------------------------------------------------------------------

/// Strip FTS5 operators and quote each token so user input is safe to pass
/// directly to an FTS5 MATCH expression.
///
/// Returns `None` if the sanitized query is empty (nothing useful to search).
pub fn sanitize_fts_query(raw: &str) -> Option<String> {
    // Characters that are special in FTS5 syntax.
    const FTS5_SPECIAL: &[char] = &['"', '\'', '-', '*', '^', '(', ')', ':', '.'];

    let tokens: Vec<String> = raw
        .split_whitespace()
        .filter_map(|word| {
            // Strip leading/trailing special characters.
            let cleaned: String = word
                .chars()
                .filter(|c| !FTS5_SPECIAL.contains(c))
                .collect();

            if cleaned.is_empty() {
                None
            } else {
                // Wrap in double quotes so FTS5 treats it as a phrase token.
                Some(format!("\"{cleaned}\""))
            }
        })
        .collect();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" OR "))
    }
}

// ---------------------------------------------------------------------------
// Reciprocal Rank Fusion
// ---------------------------------------------------------------------------

/// Merge keyword and semantic result lists using Reciprocal Rank Fusion (RRF).
///
/// Each list is `(observation_id, score)` where the score is the rank position
/// (0-based, lower is better). The canonical RRF formula is:
///
///   rrf(d) = Σ  1 / (k + rank(d))
///
/// with k = 60 (standard default that works well in practice).
///
/// The merged list is sorted by descending RRF score and truncated to `limit`.
pub fn rrf_merge(
    keyword_results: &[(i64, f64)],
    semantic_results: &[(i64, f64)],
    limit: usize,
) -> Vec<RankedResult> {
    const K: f64 = 60.0;

    use std::collections::HashMap;

    // Accumulate per-document scores and track per-list ranks.
    let mut scores: HashMap<i64, (f64, Option<i64>, Option<i64>)> = HashMap::new();

    for (rank, (obs_id, _score)) in keyword_results.iter().enumerate() {
        let entry = scores.entry(*obs_id).or_insert((0.0, None, None));
        entry.0 += 1.0 / (K + rank as f64 + 1.0);
        entry.1 = Some(rank as i64);
    }

    for (rank, (obs_id, _score)) in semantic_results.iter().enumerate() {
        let entry = scores.entry(*obs_id).or_insert((0.0, None, None));
        entry.0 += 1.0 / (K + rank as f64 + 1.0);
        entry.2 = Some(rank as i64);
    }

    let mut ranked: Vec<RankedResult> = scores
        .into_iter()
        .map(|(obs_id, (rrf_score, kw_rank, sem_rank))| RankedResult {
            observation_id: obs_id,
            keyword_rank: kw_rank,
            semantic_rank: sem_rank,
            rrf_score,
        })
        .collect();

    // Sort by descending RRF score; break ties by observation_id ascending.
    ranked.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.observation_id.cmp(&b.observation_id))
    });

    ranked.truncate(limit);
    ranked
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- sanitize_fts_query ---

    #[test]
    fn test_sanitize_plain_words() {
        let result = sanitize_fts_query("hello world").unwrap();
        assert_eq!(result, "\"hello\" OR \"world\"");
    }

    #[test]
    fn test_sanitize_strips_operators() {
        // FTS5 operators and special chars should be removed.
        let result = sanitize_fts_query("rust AND \"memory\" OR -bad").unwrap();
        // "AND" and "OR" are plain words after stripping; "bad" gets the dash removed.
        assert!(result.contains("\"rust\""));
        assert!(result.contains("\"memory\""));
        assert!(result.contains("\"bad\""));
    }

    #[test]
    fn test_sanitize_empty_string_returns_none() {
        assert!(sanitize_fts_query("").is_none());
    }

    #[test]
    fn test_sanitize_only_special_chars_returns_none() {
        assert!(sanitize_fts_query("\"\" - * ^").is_none());
    }

    #[test]
    fn test_sanitize_single_token() {
        let result = sanitize_fts_query("cats").unwrap();
        assert_eq!(result, "\"cats\"");
    }

    // --- rrf_merge ---

    #[test]
    fn test_rrf_merge_keyword_only() {
        let kw = vec![(1i64, 0.0), (2i64, 1.0), (3i64, 2.0)];
        let sem: Vec<(i64, f64)> = vec![];

        let results = rrf_merge(&kw, &sem, 3);
        assert_eq!(results.len(), 3);
        // Rank 0 should be top-scored.
        assert_eq!(results[0].observation_id, 1);
        assert!(results[0].rrf_score > results[1].rrf_score);
        assert!(results[0].keyword_rank == Some(0));
        assert!(results[0].semantic_rank.is_none());
    }

    #[test]
    fn test_rrf_merge_semantic_only() {
        let kw: Vec<(i64, f64)> = vec![];
        let sem = vec![(10i64, 0.0), (20i64, 1.0)];

        let results = rrf_merge(&kw, &sem, 5);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].observation_id, 10);
        assert!(results[0].semantic_rank == Some(0));
        assert!(results[0].keyword_rank.is_none());
    }

    #[test]
    fn test_rrf_merge_both_arms_boosts_overlap() {
        // Observation 2 appears in both lists; it should score higher than either alone.
        let kw = vec![(1i64, 0.0), (2i64, 1.0)];
        let sem = vec![(2i64, 0.0), (3i64, 1.0)];

        let results = rrf_merge(&kw, &sem, 10);

        let obs2 = results.iter().find(|r| r.observation_id == 2).unwrap();
        let obs1 = results.iter().find(|r| r.observation_id == 1).unwrap();
        let obs3 = results.iter().find(|r| r.observation_id == 3).unwrap();

        // obs2 appears in both lists so it gets combined score.
        assert!(obs2.rrf_score > obs1.rrf_score);
        assert!(obs2.rrf_score > obs3.rrf_score);
        assert!(obs2.keyword_rank.is_some());
        assert!(obs2.semantic_rank.is_some());
    }

    #[test]
    fn test_rrf_merge_limit_respected() {
        let kw: Vec<(i64, f64)> = (0..20).map(|i| (i as i64, i as f64)).collect();
        let sem: Vec<(i64, f64)> = vec![];

        let results = rrf_merge(&kw, &sem, 5);
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_rrf_merge_empty_inputs() {
        let results = rrf_merge(&[], &[], 10);
        assert!(results.is_empty());
    }
}

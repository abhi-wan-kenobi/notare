//! Reciprocal Rank Fusion, mirroring `apps/desktop/src/search/hybrid.ts`'s
//! `fuseArms` bit-for-bit: each arm contributes `1/(k+rank)` per item, keyed
//! by result id; the best (lowest) rank wins when a key repeats within one
//! arm; ties are broken by key for determinism. RRF is rank-based, so the
//! lexical (SQL `LIKE`) and semantic (cosine distance) arms' incomparable
//! score scales never need normalizing.

use std::collections::HashMap;

pub const RRF_K: f64 = 60.0;

/// One arm's ranked list entry: items in descending relevance (rank 0 = best).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedItem {
    pub key: String,
}

impl RankedItem {
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FusedEntry {
    pub key: String,
    pub score: f64,
}

/// Pure RRF over any number of named arms. `arms` is `(name, ranked items)`
/// pairs; the same key appearing in multiple arms accumulates.
pub fn fuse_arms(arms: &[(&str, &[RankedItem])], k: f64) -> Vec<FusedEntry> {
    // key -> (score, arm_name -> best rank seen for that arm)
    let mut acc: HashMap<String, (f64, HashMap<String, usize>)> = HashMap::new();

    for (arm_name, items) in arms {
        for (rank, item) in items.iter().enumerate() {
            let entry = acc
                .entry(item.key.clone())
                .or_insert_with(|| (0.0, HashMap::new()));
            let is_better = match entry.1.get(*arm_name) {
                None => true,
                Some(&prev_rank) => rank < prev_rank,
            };
            if is_better {
                if let Some(&prev_rank) = entry.1.get(*arm_name) {
                    entry.0 -= 1.0 / (k + prev_rank as f64);
                }
                entry.1.insert((*arm_name).to_string(), rank);
                entry.0 += 1.0 / (k + rank as f64);
            }
        }
    }

    let mut result: Vec<FusedEntry> = acc
        .into_iter()
        .map(|(key, (score, _))| FusedEntry { key, score })
        .collect();
    result.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.key.cmp(&b.key))
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(keys: &[&str]) -> Vec<RankedItem> {
        keys.iter().map(|k| RankedItem::new(*k)).collect()
    }

    fn score(fused: &[FusedEntry], key: &str) -> f64 {
        fused.iter().find(|e| e.key == key).unwrap().score
    }

    // These five cases port `fuseArms (RRF core)` from
    // apps/desktop/src/search/hybrid.test.ts verbatim, so the Rust and
    // TypeScript fusions provably agree on fixture lists.

    #[test]
    fn sums_1_over_k_plus_rank_across_arms_and_ranks_by_fused_score() {
        let lexical = items(&["a", "b", "c"]);
        let semantic = items(&["b", "a", "d"]);
        let fused = fuse_arms(&[("lexical", &lexical), ("semantic", &semantic)], RRF_K);

        assert!((score(&fused, "a") - (1.0 / 60.0 + 1.0 / 61.0)).abs() < 1e-10);
        assert!((score(&fused, "b") - (1.0 / 61.0 + 1.0 / 60.0)).abs() < 1e-10);
        assert_eq!(fused[0].key, "a"); // equal score, 'a' < 'b'
        assert_eq!(fused[1].key, "b");
        assert!((score(&fused, "c") - 1.0 / 62.0).abs() < 1e-10);
        assert!((score(&fused, "d") - 1.0 / 62.0).abs() < 1e-10);
    }

    #[test]
    fn degenerates_to_single_arm_order() {
        let lexical = items(&["x", "y", "z"]);
        let fused = fuse_arms(&[("lexical", &lexical)], RRF_K);
        let keys: Vec<&str> = fused.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["x", "y", "z"]);
    }

    #[test]
    fn empty_dense_arm_is_pure_lexical() {
        let lexical = items(&["p", "q"]);
        let semantic: Vec<RankedItem> = vec![];
        let fused = fuse_arms(&[("lexical", &lexical), ("semantic", &semantic)], RRF_K);
        let keys: Vec<&str> = fused.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["p", "q"]);
    }

    #[test]
    fn keeps_best_rank_when_key_repeats_within_an_arm() {
        let semantic = items(&["a", "b", "a"]);
        let fused = fuse_arms(&[("semantic", &semantic)], RRF_K);
        // 'a' scores from its rank-0 appearance only, not rank-2.
        assert!((score(&fused, "a") - 1.0 / 60.0).abs() < 1e-10);
        assert_eq!(fused[0].key, "a");
    }

    #[test]
    fn deterministic_tie_break_by_key() {
        let lexical = items(&["z"]);
        let semantic = items(&["a"]);
        let fused = fuse_arms(&[("lexical", &lexical), ("semantic", &semantic)], RRF_K);
        let keys: Vec<&str> = fused.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "z"]); // equal score -> key order
    }
}

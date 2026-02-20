use std::collections::BTreeSet;

use super::CoherentFeatureFlags;

pub(crate) trait CoherentFlagDiff {
    fn diff(&self, prev: &CoherentFeatureFlags) -> Vec<String>;
}

impl CoherentFlagDiff for CoherentFeatureFlags {
    fn diff(&self, prev: &CoherentFeatureFlags) -> Vec<String> {
        if self == prev {
            return vec![];
        }

        let mut changes: Vec<String> = vec![];

        let all_names: BTreeSet<String> =
            BTreeSet::from_iter(self.keys().chain(prev.keys()).cloned());
        for key in all_names {
            let current = self.get(&key);
            let previous = prev.get(&key);

            if current == previous {
                continue;
            }

            match (current, previous) {
                (None, None) => continue,
                (None, Some(feature)) => changes.push(format!("-feature:{key}:{feature:?}")),
                (Some(feature), None) => changes.push(format!("+feature:{key}:{feature:?}")),
                (Some(current), Some(previous)) if current == previous => continue,
                (Some(current), Some(previous)) => {
                    changes.push(format!("~feature:{key}:{}", current.diff(previous)))
                }
            }
        }

        changes
    }
}

#[cfg(test)]
mod tests {
    use crate::checkin::Feature;

    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn feat(
        variant: serde_json::Value,
        payload: Option<serde_json::Value>,
    ) -> Arc<Feature<serde_json::Value>> {
        Arc::new(Feature { variant, payload })
    }

    fn flags(pairs: &[(&str, Arc<Feature<serde_json::Value>>)]) -> CoherentFeatureFlags {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn empty() -> CoherentFeatureFlags {
        BTreeMap::new()
    }

    #[test]
    fn both_empty_returns_empty_vec() {
        assert!(empty().diff(&empty()).is_empty());
    }

    #[test]
    fn identical_single_entry_returns_empty_vec() {
        let f = flags(&[("a", feat(json!("on"), None))]);
        assert!(f.diff(&f.clone()).is_empty());
    }

    #[test]
    fn identical_multiple_entries_returns_empty_vec() {
        let f = flags(&[
            ("a", feat(json!("on"), None)),
            ("b", feat(json!("off"), Some(json!(42)))),
        ]);
        assert!(f.diff(&f.clone()).is_empty());
    }
    #[test]
    fn single_addition_produces_correct_entry() {
        let feature = feat(json!("on"), None);
        let prev = empty();
        let curr = flags(&[("flag_a", feature.clone())]);

        assert_eq!(
            curr.diff(&prev),
            vec![String::from(
                r#"+feature:flag_a:Feature { variant: String("on"), payload: None }"#
            )]
        );
    }

    #[test]
    fn two_additions_produce_two_correctly_formatted_entries() {
        let fa = feat(json!("on"), None);
        let fb = feat(json!("off"), None);
        let prev = empty();
        let curr = flags(&[("alpha", fa.clone()), ("beta", fb.clone())]);

        let expected = vec![
            String::from(r#"+feature:alpha:Feature { variant: String("on"), payload: None }"#),
            String::from(r#"+feature:beta:Feature { variant: String("off"), payload: None }"#),
        ];
        assert_eq!(curr.diff(&prev), expected);
    }

    #[test]
    fn single_removal_produces_correct_entry() {
        let feature = feat(json!("on"), None);
        let prev = flags(&[("flag_a", feature.clone())]);
        let curr = empty();

        assert_eq!(
            curr.diff(&prev),
            vec![String::from(
                r#"-feature:flag_a:Feature { variant: String("on"), payload: None }"#
            ),]
        );
    }

    #[test]
    fn variant_change_produces_mutation_entry() {
        let prev_feat = feat(json!("off"), None);
        let curr_feat = feat(json!("on"), None);
        let prev = flags(&[("toggle", prev_feat.clone())]);
        let curr = flags(&[("toggle", curr_feat.clone())]);

        assert_eq!(
            curr.diff(&prev),
            vec![String::from(
                r#"~feature:toggle:variant: String("off") -> String("on")"#
            ),]
        );
    }

    #[test]
    fn add_remove_and_mutate_in_one_diff() {
        let stable_feat = feat(json!("stable"), None);
        let removed_feat = feat(json!("old"), None);
        let prev_mut = feat(json!("v1"), Some(json!(1)));
        let curr_mut = feat(json!("v2"), Some(json!(2)));
        let added_feat = feat(json!("new"), None);

        let prev = flags(&[
            ("stable", stable_feat.clone()),
            ("removed", removed_feat.clone()),
            ("mutated", prev_mut.clone()),
        ]);
        let curr = flags(&[
            ("stable", stable_feat.clone()),
            ("added", added_feat.clone()),
            ("mutated", curr_mut.clone()),
        ]);

        let expected = vec![
            String::from(r#"+feature:added:Feature { variant: String("new"), payload: None }"#),
            String::from(
                r#"~feature:mutated:variant: String("v1") -> String("v2"), payload: Some(Number(1)) -> Some(Number(2))"#,
            ),
            String::from(r#"-feature:removed:Feature { variant: String("old"), payload: None }"#),
        ];

        assert_eq!(curr.diff(&prev), expected);
    }
}

use super::Feature;

impl<T: serde::ser::Serialize + serde::de::DeserializeOwned + std::cmp::PartialEq + std::fmt::Debug>
    Feature<T>
{
    pub fn diff(&self, previous: &Self) -> String {
        if self == previous {
            return "no change".into();
        }

        let mut diff: Vec<String> = vec![];
        if self.variant != previous.variant {
            diff.push(format!(
                "variant: {:?} -> {:?}",
                previous.variant, self.variant
            ));
        }

        if self.payload != previous.payload {
            diff.push(format!(
                "payload: {:?} -> {:?}",
                previous.payload, self.payload
            ));
        }

        diff.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_feature(variant: serde_json::Value, payload: Option<i32>) -> Feature<i32> {
        Feature { variant, payload }
    }

    #[test]
    fn diff_identical_returns_no_change() {
        let f = make_feature(json!("enabled"), Some(42));
        assert_eq!(f.diff(&f.clone()), "no change");
    }

    #[test]
    fn diff_payload_changed_is_reported() {
        let prev = make_feature(json!("on"), Some(1));
        let curr = make_feature(json!("on"), Some(2));
        let result = curr.diff(&prev);
        assert_eq!(result, "payload: Some(1) -> Some(2)");
    }

    #[test]
    fn diff_payload_added_is_reported() {
        let prev = make_feature(json!("on"), None);
        let curr = make_feature(json!("on"), Some(99));
        let result = curr.diff(&prev);
        assert_eq!(result, r#"payload: None -> Some(99)"#);
    }

    #[test]
    fn diff_payload_removed_is_reported() {
        let prev = make_feature(json!("on"), Some(7));
        let curr = make_feature(json!("on"), None);
        let result = curr.diff(&prev);
        assert_eq!(result, r#"payload: Some(7) -> None"#);
    }

    #[test]
    fn diff_variant_only_change() {
        let prev = make_feature(json!("off"), Some(1));
        let curr = make_feature(json!("on"), Some(1));
        let result = curr.diff(&prev);
        assert_eq!(result, r#"variant: String("off") -> String("on")"#);
    }

    #[test]
    fn diff_both_changed() {
        let prev = make_feature(json!("off"), Some(1));
        let curr = make_feature(json!("on"), Some(2));
        let result = curr.diff(&prev);
        assert_eq!(
            result,
            r#"variant: String("off") -> String("on"), payload: Some(1) -> Some(2)"#
        );
    }
}
